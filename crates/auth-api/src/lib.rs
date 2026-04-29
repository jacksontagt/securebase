use auth_core::AuthTokenVerifier;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use reqwest::Client;
use securebase_proto::auth::{
    AuthErrorBody, AuthResponse, LoginRequest, RefreshRequest, SignupRequest,
};
use serde::Deserialize;

#[derive(Clone)]
pub struct GoTrueConfig {
    client: Client,
    endpoint: String,
    verifier: AuthTokenVerifier,
}

pub async fn serve(addr: &str, gotrue_url: &str, secret: &[u8]) -> anyhow::Result<()> {
    let state = GoTrueConfig {
        client: Client::new(),
        endpoint: gotrue_url.trim_end_matches('/').to_string(),
        verifier: AuthTokenVerifier::new(secret),
    };

    let app = Router::new()
        .route("/signup", post(signup))
        .route("/login", post(login))
        .route("/refresh", post(refresh))
        .route("/logout", post(logout))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("auth-api listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
struct GoTrueSession {
    access_token: String,
    refresh_token: String,
    token_type: String,
    expires_in: i64,
}

async fn signup(
    State(state): State<GoTrueConfig>,
    Json(req): Json<SignupRequest>,
) -> Response {
    forward_session(&state, "/signup", &req).await
}

async fn login(
    State(state): State<GoTrueConfig>,
    Json(req): Json<LoginRequest>,
) -> Response {
    forward_session(&state, "/token?grant_type=password", &req).await
}

async fn refresh(
    State(state): State<GoTrueConfig>,
    Json(req): Json<RefreshRequest>,
) -> Response {
    forward_session(&state, "/token?grant_type=refresh_token", &req).await
}

async fn forward_session<T: serde::Serialize>(
    gotrue_config: &GoTrueConfig,
    gotrue_path: &str,
    body: &T,
) -> Response {
    let url = format!("{}{}", gotrue_config.endpoint, gotrue_path);
    let resp = match gotrue_config.client.post(&url).json(body).send().await {
        Ok(r) => r,
        Err(e) => {
            eprintln!("gotrue request failed: {e}");
            return send_error(StatusCode::BAD_GATEWAY, "auth upstream unreachable", None);
        }
    };

    let status = resp.status();
    let body_bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(e) => {
            eprintln!("gotrue body read failed: {e}");
            return send_error(StatusCode::BAD_GATEWAY, "auth upstream read failed", None);
        }
    };

    if !status.is_success() {
        let (msg, code) = from_gotrue_error(&body_bytes);
        let mapped = match status.as_u16() {
            401 => StatusCode::UNAUTHORIZED,
            s if s < 500 => StatusCode::BAD_REQUEST,
            _ => StatusCode::BAD_GATEWAY,
        };
        return send_error(mapped, &msg, code);
    }

    let session: GoTrueSession = match serde_json::from_slice(&body_bytes) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("could not parse gotrue session: {e}");
            return send_error(
                StatusCode::BAD_GATEWAY,
                "unexpected upstream response",
                None,
            );
        }
    };

    let verified_claims = match gotrue_config.verifier.verify(&session.access_token) {
        Ok(claims) => claims,
        Err(e) => {
            eprintln!("could not verify token: {e}");
            return send_error(
                StatusCode::BAD_GATEWAY,
                "issued token failed local verification",
                None,
            );
        }
    };

    Json(AuthResponse {
        access_token: session.access_token,
        refresh_token: session.refresh_token,
        token_type: session.token_type,
        expires_in: session.expires_in,
        claims: verified_claims,
    })
    .into_response()
}

async fn logout(State(state): State<GoTrueConfig>, headers: HeaderMap) -> Response {
    let auth = match headers.get(header::AUTHORIZATION) {
        Some(h) => h.clone(),
        None => {
            return send_error(
                StatusCode::UNAUTHORIZED,
                "missing Authorization header",
                None,
            );
        }
    };
    let url = format!("{}/logout", state.endpoint);
    match state
        .client
        .post(&url)
        .header(header::AUTHORIZATION, auth)
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => StatusCode::NO_CONTENT.into_response(),
        Ok(r) => {
            let body = r.bytes().await.unwrap_or_default();
            let (msg, code) = from_gotrue_error(&body);
            send_error(StatusCode::BAD_REQUEST, &msg, code)
        }
        Err(e) => {
            eprintln!("logout upstream failed: {e}");
            send_error(StatusCode::BAD_GATEWAY, "auth upstream unreachable", None)
        }
    }
}

fn from_gotrue_error(body: &[u8]) -> (String, Option<String>) {
    if let Ok(v) = serde_json::from_slice::<serde_json::Value>(body) {
        let msg = v
            .get("error_description")
            .and_then(|x| x.as_str())
            .or_else(|| v.get("msg").and_then(|x| x.as_str()))
            .or_else(|| v.get("error").and_then(|x| x.as_str()))
            .unwrap_or("auth error")
            .to_string();
        let code = v
            .get("error_code")
            .and_then(|x| x.as_str())
            .map(String::from);
        return (msg, code);
    }
    ("auth error".to_string(), None)
}

fn send_error(status: StatusCode, msg: &str, code: Option<String>) -> Response {
    let body = AuthErrorBody {
        error: msg.to_string(),
        code,
    };
    (status, Json(body)).into_response()
}
