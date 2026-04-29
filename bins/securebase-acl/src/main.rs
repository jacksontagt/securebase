fn main() {
    let config = acl_api::Config::from_env().unwrap_or_else(|e| {
        eprintln!("config error: {e}");
        std::process::exit(1);
    });
    let _schema = acl_api::serve(config).unwrap_or_else(|e| {
        eprintln!("startup failed:\n{e}");
        std::process::exit(1);
    });

    eprintln!("ACL service ready");
}
