fn main() {
    let config = acl_api::Config::from_env().unwrap_or_else(|e| {
        eprintln!("config error: {e}");
        std::process::exit(1);
    });
    let _schema = acl_api::serve(config).unwrap_or_else(|e| {
        eprintln!("startup failed:\n{e}");
        std::process::exit(1);
    });
    // HTTP server wired in task 2.5 (acl-api::serve will block here).
    eprintln!("ACL service ready");
}
