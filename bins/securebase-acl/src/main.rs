#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = acl_api::Config::from_env()?;
    acl_api::serve(cfg).await
}
