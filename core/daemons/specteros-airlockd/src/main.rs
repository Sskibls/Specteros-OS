use std::path::Path;

use specteros_airlockd::{AirlockIpcHandler, AirlockService, PluggableSanitizerChain};
use gk_config::{ensure_runtime_layout, validate_runtime_layout, RuntimePaths};

fn main() {
    if let Err(error) = run() {
        eprintln!("specteros-airlockd failed to start: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let runtime_paths = runtime_paths();
    ensure_runtime_layout(&runtime_paths).map_err(|error| error.to_string())?;
    validate_runtime_layout(&runtime_paths).map_err(|error| error.to_string())?;

    let state_path = runtime_paths.data_dir.join("airlockd/state.json");
    let mut service = AirlockService::new(PluggableSanitizerChain::default_chain());
    service
        .load_runtime_state(&state_path)
        .map_err(|error| error.to_string())?;
    let _handler = AirlockIpcHandler::new(service);
    println!("specteros-airlockd initialized");
    Ok(())
}

fn runtime_paths() -> RuntimePaths {
    match std::env::var("GK_RUNTIME_ROOT") {
        Ok(root) => RuntimePaths::from_root(Path::new(&root)),
        Err(_) => RuntimePaths::system_defaults(),
    }
}
