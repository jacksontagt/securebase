use sqlx::PgPool;

pub async fn run_migrations(pool: &PgPool, schema: &str) -> Result<(), sqlx::migrate::MigrateError> {
    match schema {
        "acl" => sqlx::migrate!("../../migrations/acl").run(pool).await,
        "auth" => sqlx::migrate!("../../migrations/auth").run(pool).await,
        "app" => sqlx::migrate!("../../migrations/app").run(pool).await,
        _ => panic!("unknown schema: {schema}"),
    }
}
