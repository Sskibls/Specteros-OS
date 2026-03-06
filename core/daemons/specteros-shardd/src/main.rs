use std::path::Path;

use specteros_shardd::{LinuxNamespaceStub, ShardIpcHandler, ShardManager};
use gk_audit::AuditChain;
use gk_config::{ensure_runtime_layout, validate_runtime_layout, RuntimePaths};

fn main() {
    if let Err(error) = run() {
        eprintln!("specteros-shardd failed to start: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut shard_manager = ShardManager::new(LinuxNamespaceStub);
    let mut audit_chain = AuditChain::default();
    let runtime_paths = runtime_paths();
    ensure_runtime_layout(&runtime_paths).map_err(|error| error.to_string())?;
    validate_runtime_layout(&runtime_paths).map_err(|error| error.to_string())?;

    let state_path = runtime_paths.data_dir.join("shardd/state.json");
    shard_manager
        .load_runtime_state(&state_path)
        .map_err(|error| error.to_string())?;

    let _ = shard_manager.create_shard("bootstrap", 0, &mut audit_chain);
    shard_manager
        .save_runtime_state(&state_path)
        .map_err(|error| error.to_string())?;

    let _handler = ShardIpcHandler::new(shard_manager);
    println!("specteros-shardd initialized");
    Ok(())
}

fn runtime_paths() -> RuntimePaths {
    match std::env::var("GK_RUNTIME_ROOT") {
        Ok(root) => RuntimePaths::from_root(Path::new(&root)),
        Err(_) => RuntimePaths::system_defaults(),
    }
}
