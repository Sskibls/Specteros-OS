use std::path::Path;

use specteros_auditd::{AuditDaemon, AuditIpcHandler};
use gk_config::{ensure_runtime_layout, validate_runtime_layout, RuntimePaths};

fn main() {
    if let Err(error) = run() {
        eprintln!("specteros-auditd failed to start: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let runtime_paths = runtime_paths();
    ensure_runtime_layout(&runtime_paths).map_err(|error| error.to_string())?;
    validate_runtime_layout(&runtime_paths).map_err(|error| error.to_string())?;

    let store_path = runtime_paths.data_dir.join("auditd/chain.log");
    let daemon = AuditDaemon::open(&store_path).map_err(|error| error.to_string())?;
    let _recovered = daemon
        .recover_truncated_tail()
        .map_err(|error| error.to_string())?;
    let _ = daemon
        .append_event("daemon.start", "specteros-auditd")
        .map_err(|error| error.to_string())?;

    let _handler = AuditIpcHandler::new(daemon);
    println!("specteros-auditd initialized");
    Ok(())
}

fn runtime_paths() -> RuntimePaths {
    match std::env::var("GK_RUNTIME_ROOT") {
        Ok(root) => RuntimePaths::from_root(Path::new(&root)),
        Err(_) => RuntimePaths::system_defaults(),
    }
}
