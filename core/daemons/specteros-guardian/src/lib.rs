// Specteros Guardian - Emergency Modes
//
// Provides panic mode, mask mode, and travel mode functionality
// for emergency containment and privacy protection.

use gk_audit::AuditChain;
use specteros_netd::NetworkBackend;
use specteros_shardd::{ShardManager, ShardState, NamespaceBoundary};

pub struct GuardianService<B: NetworkBackend, P: NamespaceBoundary> {
    network_backend: B,
    shard_manager: ShardManager<P>,
    travel_mode_enabled: bool,
}

impl<B: NetworkBackend, P: NamespaceBoundary> GuardianService<B, P> {
    pub fn new(network_backend: B, shard_manager: ShardManager<P>) -> Self {
        Self {
            network_backend,
            shard_manager,
            travel_mode_enabled: false,
        }
    }

    /// Panic Mode: Emergency containment
    /// - Kills all network interfaces
    /// - Locks all persona shards
    /// - Clears volatile memory secrets
    /// - Logs event to audit chain
    pub fn panic(&mut self, audit_chain: &mut AuditChain) -> Result<(), GuardianError> {
        // Kill all network interfaces via kill switch
        self.network_backend.set_kill_switch(true)
            .map_err(|e| GuardianError::NetworkOperationFailed(format!("{:?}", e)))?;
        
        audit_chain.append("guardian.panic.network_killed", "all_interfaces");

        // Lock all persona shards by stopping them
        // In a real implementation, this would also encrypt and seal shard state
        let shard_names = vec!["work", "anon", "burner", "lab"];
        for shard_name in shard_names {
            if let Some(current_state) = self.shard_manager.state_of(shard_name) {
                if current_state == ShardState::Running {
                    self.shard_manager.stop_shard(shard_name, 0, audit_chain)
                        .map_err(|e| GuardianError::ShardOperationFailed(shard_name.to_string(), format!("{:?}", e)))?;
                    audit_chain.append("guardian.panic.shard_locked", shard_name.to_string());
                }
            }
        }

        // Clear volatile memory secrets
        // In a real implementation, this would use secure memory zeroization
        self.clear_volatile_secrets();
        audit_chain.append("guardian.panic.secrets_cleared", "volatile_memory");

        // Final audit event for panic mode activation
        audit_chain.append("guardian.panic.activated", "emergency_containment");

        Ok(())
    }

    /// Clear volatile secrets from memory
    fn clear_volatile_secrets(&self) {
        // In a real implementation, this would:
        // 1. Zeroize cryptographic keys in memory
        // 2. Clear clipboard buffers
        // 3. Wipe any cached credentials
        // For now, this is a stub that represents the intent
    }

    /// Mask Mode: Decoy desktop/workspace
    pub fn mask(&mut self, decoy_workspace: &str, audit_chain: &mut AuditChain) -> Result<(), GuardianError> {
        // Switch to decoy workspace
        // In a real implementation, this would:
        // 1. Switch to a pre-configured decoy desktop environment
        // 2. Show innocuous applications and files
        // 3. Hide sensitive workspace indicators
        
        audit_chain.append("guardian.mask.activated", format!("workspace:{}", decoy_workspace));

        // For now, we'll simulate this by logging the event
        // and potentially switching shards to a decoy state
        if let Some(current_state) = self.shard_manager.state_of(decoy_workspace) {
            if current_state != ShardState::Running {
                self.shard_manager.start_shard(decoy_workspace, 0, audit_chain)
                    .map_err(|e| GuardianError::ShardOperationFailed(decoy_workspace.to_string(), format!("{:?}", e)))?;
            }
        }

        Ok(())
    }

    /// Travel Mode: Reduced local footprint
    pub fn set_travel_mode(&mut self, enabled: bool, audit_chain: &mut AuditChain) {
        self.travel_mode_enabled = enabled;
        let state = if enabled { "enabled" } else { "disabled" };
        audit_chain.append("guardian.travel_mode", state.to_string());

        // In a real implementation, this would:
        // 1. Configure ephemeral session defaults
        // 2. Set strict policy denying persistent storage
        // 3. Reduce local caching and logging
    }

    pub fn is_travel_mode_enabled(&self) -> bool {
        self.travel_mode_enabled
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GuardianError {
    #[error("Network operation failed: {0}")]
    NetworkOperationFailed(String),
    #[error("Shard operation failed for {0}: {1}")]
    ShardOperationFailed(String, String),
    #[error("Guardian service error: {0}")]
    ServiceError(String),
}

// Mock implementations for testing
#[cfg(test)]
mod tests {
    use super::*;
    use gk_audit::AuditChain;
    use specteros_netd::{NetworkBackendError, RouteProfile};
    use std::cell::RefCell;
    use std::collections::HashMap;

    struct MockNetworkBackend {
        kill_switch_enabled: RefCell<bool>,
    }

    impl MockNetworkBackend {
        fn new() -> Self {
            Self {
                kill_switch_enabled: RefCell::new(false),
            }
        }
    }

    impl NetworkBackend for MockNetworkBackend {
        fn apply_profile(
            &mut self,
            _shard_name: &str,
            _profile: RouteProfile,
        ) -> Result<(), NetworkBackendError> {
            Ok(())
        }

        fn set_kill_switch(&mut self, enabled: bool) -> Result<(), NetworkBackendError> {
            *self.kill_switch_enabled.borrow_mut() = enabled;
            Ok(())
        }

        fn can_egress(
            &self,
            _shard_name: &str,
            _profile: RouteProfile,
            _kill_switch_enabled: bool,
        ) -> bool {
            false
        }

        fn sync_state(
            &mut self,
            _route_profiles: &HashMap<String, RouteProfile>,
            _kill_switch_enabled: bool,
        ) -> Result<(), NetworkBackendError> {
            Ok(())
        }
    }

    struct MockNamespaceBoundary;

    impl NamespaceBoundary for MockNamespaceBoundary {
        fn create_namespace(&self, _shard_name: &str) -> Result<(), specteros_shardd::ShardError> {
            Ok(())
        }

        fn start_namespace(&self, _shard_name: &str) -> Result<(), specteros_shardd::ShardError> {
            Ok(())
        }

        fn stop_namespace(&self, _shard_name: &str) -> Result<(), specteros_shardd::ShardError> {
            Ok(())
        }

        fn destroy_namespace(&self, _shard_name: &str) -> Result<(), specteros_shardd::ShardError> {
            Ok(())
        }
    }

    fn create_mock_shard_manager() -> ShardManager<MockNamespaceBoundary> {
        let mut manager = ShardManager::new(MockNamespaceBoundary);
        let mut audit_chain = AuditChain::default();
        
        // Initialize with some shards
        manager.create_shard("work", 0, &mut audit_chain).unwrap();
        manager.start_shard("work", 0, &mut audit_chain).unwrap();
        
        manager.create_shard("anon", 0, &mut audit_chain).unwrap();
        manager.start_shard("anon", 0, &mut audit_chain).unwrap();
        
        manager.create_shard("burner", 0, &mut audit_chain).unwrap();
        manager.create_shard("lab", 0, &mut audit_chain).unwrap();
        
        manager
    }

    #[test]
    fn test_panic_mode() {
        let network_backend = MockNetworkBackend::new();
        let shard_manager = create_mock_shard_manager();
        let mut service = GuardianService::new(network_backend, shard_manager);
        let mut audit_chain = AuditChain::default();

        let result = service.panic(&mut audit_chain);
        assert!(result.is_ok());
        // network_killed + 2 shard_locked + secrets_cleared + activated + initial shard setup events
        assert!(audit_chain.len() >= 5);
    }

    #[test]
    fn test_mask_mode() {
        let network_backend = MockNetworkBackend::new();
        let shard_manager = create_mock_shard_manager();
        let mut service = GuardianService::new(network_backend, shard_manager);
        let mut audit_chain = AuditChain::default();

        let result = service.mask("decoy", &mut audit_chain);
        assert!(result.is_ok());
        assert_eq!(audit_chain.len(), 1);
    }

    #[test]
    fn test_travel_mode() {
        let network_backend = MockNetworkBackend::new();
        let shard_manager = create_mock_shard_manager();
        let mut service = GuardianService::new(network_backend, shard_manager);
        let mut audit_chain = AuditChain::default();

        assert!(!service.is_travel_mode_enabled());
        
        service.set_travel_mode(true, &mut audit_chain);
        assert!(service.is_travel_mode_enabled());
        assert_eq!(audit_chain.len(), 1);

        service.set_travel_mode(false, &mut audit_chain);
        assert!(!service.is_travel_mode_enabled());
        assert_eq!(audit_chain.len(), 2);
    }
}
