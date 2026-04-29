#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let host = std::env::var("AUTH_API_HOST").unwrap_or_else(|_| "0.0.0.0:8080".into());
    let gotrue = std::env::var("GOTRUE_URL")?;
    let jwt_secret = std::env::var("JWT_SECRET")?;
    auth_api::serve(&host, &gotrue, jwt_secret.as_bytes()).await
}
