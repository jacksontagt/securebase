mod error;
mod handlers;

use std::sync::Arc;

use acl_model::{parse_schema, Schema, SchemaError};
use acl_store::PostgresTupleStore;
use auth_core::{require_auth, AuthTokenVerifier};
use axum::{routing::post, Router};
use sqlx::postgres::PgPoolOptions;

pub struct Config {
    pub addr: String,
    pub schema_path: String,
    pub database_url: String,
    pub jwt_secret: String,
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(Self {
            addr: std::env::var("ACL_API_HOST").unwrap_or_else(|_| "0.0.0.0:8081".into()),
            schema_path: std::env::var("SCHEMA_PATH")
                .map_err(|_| anyhow::anyhow!("SCHEMA_PATH not set"))?,
            database_url: std::env::var("DATABASE_URL")
                .map_err(|_| anyhow::anyhow!("DATABASE_URL not set"))?,
            jwt_secret: std::env::var("JWT_SECRET")
                .map_err(|_| anyhow::anyhow!("JWT_SECRET not set"))?,
        })
    }
}

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) schema: Arc<Schema>,
    pub(crate) store: Arc<PostgresTupleStore>,
}

pub fn load_schema(path: &str) -> Result<Arc<Schema>, Vec<SchemaError>> {
    let text = std::fs::read_to_string(path).map_err(|e| {
        vec![SchemaError::Parse {
            message: format!("failed to read schema file '{path}': {e}"),
            span: 0..0,
        }]
    })?;
    parse_schema(&text).map(Arc::new)
}

pub async fn serve(cfg: Config) -> anyhow::Result<()> {
    let schema = load_schema(&cfg.schema_path).map_err(|errs| {
        anyhow::anyhow!(
            "schema load failed:\n{}",
            errs.iter()
                .map(|e| format!("{e:?}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    })?;
    println!(
        "schema loaded from '{}': {} namespace(s)",
        cfg.schema_path,
        schema.namespace_count()
    );

    let pool = PgPoolOptions::new().connect(&cfg.database_url).await?;
    securebase_db::run_migrations(&pool, "acl").await?;
    let store = Arc::new(PostgresTupleStore::new(pool));

    let state = AppState { schema, store };
    let verifier = AuthTokenVerifier::new(cfg.jwt_secret.as_bytes());

    let app = Router::new()
        .route("/check", post(handlers::check))
        .route("/write", post(handlers::write))
        .layer(axum::middleware::from_fn_with_state(verifier, require_auth))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(&cfg.addr).await?;
    println!("acl-api listening on {}", cfg.addr);
    axum::serve(listener, app).await?;
    Ok(())
}
