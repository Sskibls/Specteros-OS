// Specteros Updated Daemon
// A/B Update Mechanism with Rollback Support

use specteros_updated::UpdateService;
use gk_audit::AuditChain;
use std::path::PathBuf;
use std::env;

fn main() {
    println!("specteros-updated daemon starting...");

    // Determine config directories
    let config_dir = env::var("SPECTEROS_CONFIG")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/etc/specteros"));

    let cache_dir = env::var("SPECTEROS_CACHE")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/var/cache/specteros"));

    // Create directories if they don't exist
    std::fs::create_dir_all(&config_dir).ok();
    std::fs::create_dir_all(&cache_dir).ok();

    // Initialize update service
    match UpdateService::new(config_dir.clone(), cache_dir.clone()) {
        Ok(mut service) => {
            println!("Update service initialized");
            println!("Current slot: {:?}", service.current_slot());

            let mut audit_chain = AuditChain::default();

            // Check for pending update commit (called after boot)
            if let Some(pending) = service.pending_slot() {
                println!("Pending update detected in slot {:?}", pending);
                if let Err(e) = service.commit_update(&mut audit_chain) {
                    eprintln!("Failed to commit update: {}", e);
                    // Auto-rollback on commit failure
                    if let Err(e) = service.rollback(&mut audit_chain) {
                        eprintln!("Rollback failed: {}", e);
                    }
                } else {
                    println!("Update committed successfully");
                }
            }

            // Print status
            let status = service.get_status();
            println!("Update status: {:?}", status.state);

            println!("specteros-updated ready");

            // In production: start IPC server and listen for update commands
            // For now, just exit
        }
        Err(e) => {
            eprintln!("Failed to initialize update service: {}", e);
            std::process::exit(1);
        }
    }
}
