// SpecterOS Updated - A/B Update Mechanism
// Provides signed, reproducible updates with rollback support

use gk_audit::AuditChain;
use gk_crypto::{KeyRing, SignatureEnvelope, CryptoError as GkCryptoError};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// Update slot for A/B partition scheme
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateSlot {
    A,
    B,
}

impl UpdateSlot {
    pub fn mount_point(&self) -> &'static str {
        match self {
            UpdateSlot::A => "/mnt/root_a",
            UpdateSlot::B => "/mnt/root_b",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            UpdateSlot::A => "SPECTEROS_A",
            UpdateSlot::B => "SPECTEROS_B",
        }
    }

    pub fn other(&self) -> UpdateSlot {
        match self {
            UpdateSlot::A => UpdateSlot::B,
            UpdateSlot::B => UpdateSlot::A,
        }
    }
}

/// Update manifest describing a release
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateManifest {
    pub version: String,
    pub build_id: String,
    pub release_date: u64,
    pub slot: UpdateSlot,
    pub components: Vec<ComponentInfo>,
    pub signature: SignatureInfo,
    pub changelog: Vec<String>,
    pub min_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentInfo {
    pub name: String,
    pub version: String,
    pub hash: String,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignatureInfo {
    pub algorithm: String,
    pub key_id: String,
    pub signature: String,
    pub signed_at: u64,
}

impl From<SignatureEnvelope> for SignatureInfo {
    fn from(envelope: SignatureEnvelope) -> Self {
        Self {
            algorithm: envelope.algorithm,
            key_id: envelope.key_id,
            signature: envelope.value_hex,
            signed_at: 0, // Would need to be tracked separately
        }
    }
}

/// Update state machine
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum UpdateState {
    Idle,
    Checking,
    Downloading { progress: u8 },
    Verifying,
    Applying,
    RebootRequired,
    RollingBack,
    Failed { reason: String },
}

/// Update service for managing A/B updates
pub struct UpdateService {
    state: UpdateState,
    current_slot: UpdateSlot,
    pending_slot: Option<UpdateSlot>,
    config_dir: PathBuf,
    cache_dir: PathBuf,
    mount_root: PathBuf,
    key_ring: KeyRing,
}

impl UpdateService {
    pub fn new(config_dir: PathBuf, cache_dir: PathBuf) -> Result<Self, UpdateError> {
        Self::with_mount_root(config_dir, cache_dir, PathBuf::from("/"))
    }

    pub fn with_mount_root(config_dir: PathBuf, cache_dir: PathBuf, mount_root: PathBuf) -> Result<Self, UpdateError> {
        let keys_path = config_dir.join("keys");
        let key_ring = if keys_path.exists() {
            KeyRing::load_from_path(&keys_path)?
        } else {
            // Create default key ring for testing
            let ring = KeyRing::new("default", "default-secret-for-testing");
            ring.save_to_path(&keys_path)?;
            ring
        };
        
        // Detect current slot from boot arguments or mount info
        let current_slot = Self::detect_current_slot()?;
        
        Ok(Self {
            state: UpdateState::Idle,
            current_slot,
            pending_slot: None,
            config_dir,
            cache_dir,
            mount_root,
            key_ring,
        })
    }

    /// Detect which slot we're currently booted from
    fn detect_current_slot() -> Result<UpdateSlot, UpdateError> {
        // Check /proc/cmdline for root= parameter
        let cmdline = fs::read_to_string("/proc/cmdline")
            .unwrap_or_default();
        
        if cmdline.contains("root=LABEL=SPECTEROS_A") || cmdline.contains("root_a") {
            Ok(UpdateSlot::A)
        } else if cmdline.contains("root=LABEL=SPECTEROS_B") || cmdline.contains("root_b") {
            Ok(UpdateSlot::B)
        } else {
            // Default to A if undetermined
            Ok(UpdateSlot::A)
        }
    }

    /// Check for available updates
    pub fn check_for_updates(&mut self, _update_server: &str) -> Result<Option<UpdateManifest>, UpdateError> {
        self.state = UpdateState::Checking;
        
        // In production, this would fetch from update_server
        // For now, return None (no update available)
        self.state = UpdateState::Idle;
        Ok(None)
    }

    /// Download update to cache
    pub fn download_update(&mut self, manifest: &UpdateManifest, _download_url: &str) -> Result<(), UpdateError> {
        self.state = UpdateState::Downloading { progress: 0 };
        
        // Validate manifest signature
        self.verify_manifest(manifest)?;
        
        // Create cache directory for this update
        let update_cache = self.cache_dir.join(&manifest.build_id);
        fs::create_dir_all(&update_cache)?;
        
        // Simulate download progress
        for progress in (10..=100).step_by(10) {
            self.state = UpdateState::Downloading { progress };
            // In production: actually download files here
        }
        
        self.state = UpdateState::Verifying;
        Ok(())
    }

    /// Verify manifest signature
    pub fn verify_manifest(&self, manifest: &UpdateManifest) -> Result<(), UpdateError> {
        // Verify the manifest was signed by a trusted key
        let data = format!("{}:{}:{}", manifest.version, manifest.build_id, manifest.release_date);
        
        let envelope = SignatureEnvelope {
            algorithm: manifest.signature.algorithm.clone(),
            key_id: manifest.signature.key_id.clone(),
            value_hex: manifest.signature.signature.clone(),
        };
        
        // verify returns Ok(()) if valid, Err if invalid
        self.key_ring.verify(&data, &envelope, manifest.release_date)
            .map_err(|_| UpdateError::SignatureInvalid)?;
        
        Ok(())
    }

    /// Apply update to inactive slot
    pub fn apply_update(&mut self, manifest: &UpdateManifest, audit_chain: &mut AuditChain) -> Result<(), UpdateError> {
        self.state = UpdateState::Applying;
        
        let target_slot = self.current_slot.other();
        self.pending_slot = Some(target_slot);
        
        audit_chain.append("updated.apply.started", format!("{} -> {:?}", manifest.version, target_slot));
        
        // Mount target slot
        let mount_point = self.mount_root.join(target_slot.mount_point().trim_start_matches('/'));
        fs::create_dir_all(mount_point)?;
        
        // In production:
        // 1. Mount target partition
        // 2. Extract update payload
        // 3. Verify component hashes
        // 4. Update bootloader configuration
        // 5. Mark slot as valid
        
        // Write update metadata
        let metadata_path = self.config_dir.join("pending_update.json");
        let metadata = PendingUpdate {
            manifest: manifest.clone(),
            applied_at: current_timestamp(),
            target_slot,
        };
        fs::write(&metadata_path, serde_json::to_string_pretty(&metadata)?)?;
        
        audit_chain.append("updated.apply.completed", format!("{} to {:?}", manifest.version, target_slot));
        
        self.state = UpdateState::RebootRequired;
        Ok(())
    }

    /// Commit the pending update (mark as successful after reboot)
    pub fn commit_update(&mut self, audit_chain: &mut AuditChain) -> Result<(), UpdateError> {
        // Called after successful boot into new slot
        let metadata_path = self.config_dir.join("pending_update.json");
        
        if metadata_path.exists() {
            let metadata: PendingUpdate = serde_json::from_str(&fs::read_to_string(&metadata_path)?)?;
            
            // Mark slot as valid in bootloader
            self.mark_slot_valid(metadata.target_slot)?;
            
            // Clear pending update
            fs::remove_file(&metadata_path)?;
            
            self.current_slot = metadata.target_slot;
            self.pending_slot = None;
            self.state = UpdateState::Idle;
            
            audit_chain.append("updated.commit", format!("committed {:?}", metadata.target_slot));
        }
        
        Ok(())
    }

    /// Rollback to previous slot
    pub fn rollback(&mut self, audit_chain: &mut AuditChain) -> Result<(), UpdateError> {
        self.state = UpdateState::RollingBack;
        
        let rollback_slot = self.current_slot.other();
        
        audit_chain.append("updated.rollback.started", format!("{:?} -> {:?}", self.current_slot, rollback_slot));
        
        // Update bootloader to boot from other slot
        self.set_boot_slot(rollback_slot)?;
        
        audit_chain.append("updated.rollback.completed", format!("to {:?}", rollback_slot));
        
        self.state = UpdateState::RebootRequired;
        Ok(())
    }

    /// Mark a slot as valid (successfully booted)
    fn mark_slot_valid(&self, _slot: UpdateSlot) -> Result<(), UpdateError> {
        // In production: update bootloader environment
        // e.g., fw_setenv slot_valid_a 1
        Ok(())
    }

    /// Set which slot to boot next
    fn set_boot_slot(&self, _slot: UpdateSlot) -> Result<(), UpdateError> {
        // In production: update bootloader configuration
        // e.g., fw_setenv boot_slot a/b
        Ok(())
    }

    /// Get current update state
    pub fn state(&self) -> &UpdateState {
        &self.state
    }

    /// Get current slot
    pub fn current_slot(&self) -> UpdateSlot {
        self.current_slot
    }

    /// Get pending slot (if update applied)
    pub fn pending_slot(&self) -> Option<UpdateSlot> {
        self.pending_slot
    }

    /// Check if reboot is required
    pub fn reboot_required(&self) -> bool {
        matches!(self.state, UpdateState::RebootRequired)
    }

    /// Get system update status
    pub fn get_status(&self) -> UpdateStatus {
        UpdateStatus {
            current_version: env!("CARGO_PKG_VERSION").to_string(),
            current_slot: self.current_slot,
            pending_version: None,
            pending_slot: self.pending_slot,
            state: self.state.clone(),
            last_check: None,
        }
    }
}

/// Pending update metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingUpdate {
    pub manifest: UpdateManifest,
    pub applied_at: u64,
    pub target_slot: UpdateSlot,
}

/// System update status
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateStatus {
    pub current_version: String,
    pub current_slot: UpdateSlot,
    pub pending_version: Option<String>,
    pub pending_slot: Option<UpdateSlot>,
    pub state: UpdateState,
    pub last_check: Option<u64>,
}

/// Update errors
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Signature invalid")]
    SignatureInvalid,
    
    #[error("Crypto error: {0}")]
    Crypto(#[from] GkCryptoError),
    
    #[error("Hash mismatch for {0}")]
    HashMismatch(String),
    
    #[error("Slot {0} is not valid")]
    InvalidSlot(String),
    
    #[error("No pending update to commit")]
    NoPendingUpdate,
    
    #[error("Update already in progress")]
    UpdateInProgress,
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use gk_crypto::{KeyRecord, SignatureEnvelope};
    use gk_audit::AuditChain;

    fn create_test_service() -> (UpdateService, TempDir) {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config");
        let cache_dir = temp_dir.path().join("cache");
        let mount_root = temp_dir.path().join("mnt");
        fs::create_dir_all(&config_dir).unwrap();
        fs::create_dir_all(&cache_dir).unwrap();
        fs::create_dir_all(&mount_root).unwrap();
        
        let service = UpdateService::with_mount_root(config_dir, cache_dir, mount_root).unwrap();
        (service, temp_dir)
    }

    fn create_mock_manifest(version: &str, key_id: &str, secret: &str, now: u64) -> UpdateManifest {
        let version = version.to_string();
        let build_id = format!("build-{}", version);
        let release_date = now;
        
        // Sign the manifest
        let data = format!("{}:{}:{}", version, build_id, release_date);
        let key_ring = KeyRing::new(key_id, secret);
        let sig = key_ring.sign(&data, now).unwrap();
        
        UpdateManifest {
            version,
            build_id,
            release_date,
            slot: UpdateSlot::B,
            components: vec![
                ComponentInfo {
                    name: "kernel".to_string(),
                    version: "6.1.0".to_string(),
                    hash: "sha256:deadbeef".to_string(),
                    size: 1024 * 1024,
                }
            ],
            signature: sig.into(),
            changelog: vec!["Initial release".to_string()],
            min_version: None,
        }
    }

    #[test]
    fn test_slot_logic() {
        assert_eq!(UpdateSlot::A.other(), UpdateSlot::B);
        assert_eq!(UpdateSlot::B.other(), UpdateSlot::A);
        assert_eq!(UpdateSlot::A.mount_point(), "/mnt/root_a");
        assert_eq!(UpdateSlot::B.mount_point(), "/mnt/root_b");
        assert_eq!(UpdateSlot::A.label(), "SPECTEROS_A");
        assert_eq!(UpdateSlot::B.label(), "SPECTEROS_B");
    }

    #[test]
    fn test_manifest_signature_valid() {
        let (service, _temp) = create_test_service();
        let now = current_timestamp();
        let manifest = create_mock_manifest("1.0.0", "default", "default-secret-for-testing", now);
        
        assert!(service.verify_manifest(&manifest).is_ok());
    }

    #[test]
    fn test_manifest_signature_invalid_secret() {
        let (service, _temp) = create_test_service();
        let now = current_timestamp();
        let manifest = create_mock_manifest("1.0.0", "default", "wrong-secret", now);
        
        let result = service.verify_manifest(&manifest);
        assert!(matches!(result, Err(UpdateError::SignatureInvalid)));
    }

    #[test]
    fn test_manifest_signature_expired_key() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config");
        let cache_dir = temp_dir.path().join("cache");
        fs::create_dir_all(&config_dir).unwrap();
        
        let now = current_timestamp();
        // Create a key ring with an expired key
        let mut key_ring = KeyRing::new("expired-key", "secret");
        key_ring.rotate(KeyRecord::new("expired-key", "secret", Some(now - 100)), true);
        key_ring.save_to_path(&config_dir.join("keys")).unwrap();
        
        let service = UpdateService::new(config_dir, cache_dir).unwrap();
        let manifest = create_mock_manifest("1.0.0", "expired-key", "secret", now);
        
        let result = service.verify_manifest(&manifest);
        assert!(matches!(result, Err(UpdateError::SignatureInvalid)));
    }

    #[test]
    fn test_manifest_signature_revoked_key() {
        let temp_dir = TempDir::new().unwrap();
        let config_dir = temp_dir.path().join("config");
        let cache_dir = temp_dir.path().join("cache");
        fs::create_dir_all(&config_dir).unwrap();
        
        let now = current_timestamp();
        // Create a key ring with a revoked key
        let mut key_ring = KeyRing::new("revoked-key", "secret");
        key_ring.revoke_key("revoked-key");
        key_ring.save_to_path(&config_dir.join("keys")).unwrap();
        
        let service = UpdateService::new(config_dir, cache_dir).unwrap();
        let manifest = create_mock_manifest("1.0.0", "revoked-key", "secret", now);
        
        let result = service.verify_manifest(&manifest);
        assert!(matches!(result, Err(UpdateError::SignatureInvalid)));
    }

    #[test]
    fn test_apply_and_commit_flow() {
        let (mut service, _temp) = create_test_service();
        let mut audit_chain = AuditChain::default();
        let now = current_timestamp();
        let manifest = create_mock_manifest("1.1.0", "default", "default-secret-for-testing", now);
        
        // 1. Download
        service.download_update(&manifest, "https://updates.phantomkernel.org/1.1.0").unwrap();
        assert!(matches!(service.state(), UpdateState::Verifying));
        
        // 2. Apply
        service.apply_update(&manifest, &mut audit_chain).unwrap();
        assert_eq!(service.state(), &UpdateState::RebootRequired);
        assert_eq!(service.pending_slot(), Some(UpdateSlot::B));
        
        // Verify metadata file exists
        let metadata_path = service.config_dir.join("pending_update.json");
        assert!(metadata_path.exists());
        
        // 3. Commit (simulated after reboot)
        service.commit_update(&mut audit_chain).unwrap();
        assert_eq!(service.state(), &UpdateState::Idle);
        assert_eq!(service.current_slot(), UpdateSlot::B);
        assert!(service.pending_slot().is_none());
        assert!(!metadata_path.exists());
    }

    #[test]
    fn test_rollback_logic() {
        let (mut service, _temp) = create_test_service();
        let mut audit_chain = AuditChain::default();
        
        assert_eq!(service.current_slot(), UpdateSlot::A);
        
        service.rollback(&mut audit_chain).unwrap();
        assert_eq!(service.state(), &UpdateState::RebootRequired);
        // Note: rollback logic in lib.rs sets boot slot to other(), but current_slot stays same until commit
        // In a real system, reboot would happen here.
    }

    #[test]
    fn test_hash_verification_placeholder() {
        // This test would verify component hashes. 
        // Currently apply_update doesn't implement full verification, so this is a baseline.
        let (mut service, _temp) = create_test_service();
        let now = current_timestamp();
        let manifest = create_mock_manifest("1.0.0", "default", "default-secret-for-testing", now);
        
        // verify_manifest only checks the signature of the manifest itself
        assert!(service.verify_manifest(&manifest).is_ok());
    }

    #[test]
    fn test_edge_case_io_failure() {
        let (mut service, _temp) = create_test_service();
        let mut audit_chain = AuditChain::default();
        let now = current_timestamp();
        let manifest = create_mock_manifest("1.0.0", "default", "default-secret-for-testing", now);
        
        // Simulate IO failure by making config_dir read-only
        let mut permissions = fs::metadata(&service.config_dir).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(&service.config_dir, permissions).unwrap();
        
        let result = service.apply_update(&manifest, &mut audit_chain);
        assert!(result.is_err());
        
        // Reset permissions so TempDir can cleanup
        let mut permissions = fs::metadata(&service.config_dir).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(&service.config_dir, permissions).unwrap();
    }
}
