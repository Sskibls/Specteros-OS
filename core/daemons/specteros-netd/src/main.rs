use std::path::Path;

use specteros_netd::{
    DeterministicLeakChecker, NetworkIpcHandler, NetworkPolicyService, NftablesRouteBackend,
    RouteProfile,
};
use gk_audit::AuditChain;
use gk_config::{ensure_runtime_layout, validate_runtime_layout, RuntimePaths};

fn main() {
    if let Err(error) = run() {
        eprintln!("specteros-netd failed to start: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let runtime_paths = runtime_paths();
    ensure_runtime_layout(&runtime_paths).map_err(|error| error.to_string())?;
    validate_runtime_layout(&runtime_paths).map_err(|error| error.to_string())?;

    let state_path = runtime_paths.data_dir.join("netd/state.json");
    let backend = NftablesRouteBackend::new_staged();
    let mut network_policy_service = NetworkPolicyService::new(DeterministicLeakChecker, backend);
    let mut audit_chain = AuditChain::default();
    network_policy_service
        .load_runtime_state(&state_path, &mut audit_chain)
        .map_err(|error| error.to_string())?;

    if network_policy_service.profile_of("system").is_none() {
        network_policy_service
            .apply_profile("system", RouteProfile::Offline, &mut audit_chain)
            .map_err(|error| error.to_string())?;
    }

    network_policy_service
        .save_runtime_state(&state_path)
        .map_err(|error| error.to_string())?;

    let _handler = NetworkIpcHandler::new(network_policy_service);
    println!("specteros-netd initialized");
    Ok(())
}

fn runtime_paths() -> RuntimePaths {
    match std::env::var("GK_RUNTIME_ROOT") {
        Ok(root) => RuntimePaths::from_root(Path::new(&root)),
        Err(_) => RuntimePaths::system_defaults(),
    }
}
