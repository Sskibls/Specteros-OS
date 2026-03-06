use std::path::Path;

use specteros_policyd::{CapabilityRequest, CapabilityRule, PolicyIpcHandler, PolicyService};
use gk_audit::AuditChain;
use gk_config::{ensure_runtime_layout, validate_runtime_layout, RuntimePaths};

fn main() {
    if let Err(error) = run() {
        eprintln!("specteros-policyd failed to start: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut policy_service = PolicyService::new("bootstrap-signing-key");
    let mut audit_chain = AuditChain::default();
    let runtime_paths = runtime_paths();
    ensure_runtime_layout(&runtime_paths).map_err(|error| error.to_string())?;
    validate_runtime_layout(&runtime_paths).map_err(|error| error.to_string())?;

    let state_path = runtime_paths.data_dir.join("policyd/state.json");
    policy_service
        .load_runtime_state(&state_path)
        .map_err(|error| error.to_string())?;

    let _ = policy_service.allow_rule(CapabilityRule::new(
        "daemon://bootstrap",
        "system",
        "healthcheck",
        "read",
    ));

    let bootstrap_request = CapabilityRequest {
        subject: "daemon://bootstrap".to_string(),
        shard: "system".to_string(),
        resource: "healthcheck".to_string(),
        action: "read".to_string(),
        ttl_seconds: 10,
    };

    let _ = policy_service
        .issue_token(&bootstrap_request, 0, &mut audit_chain)
        .map_err(|error| error.to_string())?;
    policy_service
        .save_runtime_state(&state_path)
        .map_err(|error| error.to_string())?;

    let _handler = PolicyIpcHandler::new(policy_service);
    println!("specteros-policyd initialized");
    Ok(())
}

fn runtime_paths() -> RuntimePaths {
    match std::env::var("GK_RUNTIME_ROOT") {
        Ok(root) => RuntimePaths::from_root(Path::new(&root)),
        Err(_) => RuntimePaths::system_defaults(),
    }
}
