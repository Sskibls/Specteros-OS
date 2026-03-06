use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;

use gk_audit::AuditChain;
use gk_ipc::{
    decode_payload, require_role, success_payload, AuthContext, IpcError, IpcMethodHandler,
    IpcRequest, IpcResponse,
};
use gk_persistence::{load_state, save_state, PersistedState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShardState {
    Created,
    Running,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardTransitionRecord {
    pub shard_name: String,
    pub from: Option<ShardState>,
    pub to: Option<ShardState>,
    pub at_epoch_s: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ShardRuntimeState {
    pub shard_states: HashMap<String, ShardState>,
    pub transitions: Vec<ShardTransitionRecord>,
}

impl PersistedState for ShardRuntimeState {
    const STATE_KIND: &'static str = "phantomkernel-shardd-runtime";
    const CURRENT_SCHEMA_VERSION: u32 = 1;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShardError {
    AlreadyExists,
    NotFound,
    InvalidTransition {
        from: Option<ShardState>,
        attempted: &'static str,
    },
    PlatformFailure(String),
    PersistenceFailure(String),
}

impl Display for ShardError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ShardError::AlreadyExists => write!(formatter, "shard already exists"),
            ShardError::NotFound => write!(formatter, "shard not found"),
            ShardError::InvalidTransition { from, attempted } => {
                write!(
                    formatter,
                    "invalid transition from {from:?} via {attempted}"
                )
            }
            ShardError::PlatformFailure(message) => {
                write!(formatter, "platform boundary failed: {message}")
            }
            ShardError::PersistenceFailure(message) => {
                write!(formatter, "persistence failure: {message}")
            }
        }
    }
}

impl Error for ShardError {}

pub trait NamespaceBoundary {
    fn create_namespace(&self, shard_name: &str) -> Result<(), ShardError>;
    fn start_namespace(&self, shard_name: &str) -> Result<(), ShardError>;
    fn stop_namespace(&self, shard_name: &str) -> Result<(), ShardError>;
    fn destroy_namespace(&self, shard_name: &str) -> Result<(), ShardError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct LinuxNamespaceStub;

impl NamespaceBoundary for LinuxNamespaceStub {
    fn create_namespace(&self, _shard_name: &str) -> Result<(), ShardError> {
        Ok(())
    }

    fn start_namespace(&self, _shard_name: &str) -> Result<(), ShardError> {
        Ok(())
    }

    fn stop_namespace(&self, _shard_name: &str) -> Result<(), ShardError> {
        Ok(())
    }

    fn destroy_namespace(&self, _shard_name: &str) -> Result<(), ShardError> {
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ShardManager<P: NamespaceBoundary> {
    platform: P,
    shard_states: HashMap<String, ShardState>,
    transitions: Vec<ShardTransitionRecord>,
}

impl<P: NamespaceBoundary> ShardManager<P> {
    pub fn new(platform: P) -> Self {
        Self {
            platform,
            shard_states: HashMap::new(),
            transitions: Vec::new(),
        }
    }

    pub fn create_shard(
        &mut self,
        shard_name: &str,
        now_epoch_s: u64,
        audit_chain: &mut AuditChain,
    ) -> Result<(), ShardError> {
        if self.shard_states.contains_key(shard_name) {
            return Err(ShardError::AlreadyExists);
        }

        self.platform.create_namespace(shard_name)?;
        self.shard_states
            .insert(shard_name.to_string(), ShardState::Created);
        self.record_transition(shard_name, None, Some(ShardState::Created), now_epoch_s);
        audit_chain.append("shardd.transition", format!("{shard_name}:None->Created"));
        Ok(())
    }

    pub fn start_shard(
        &mut self,
        shard_name: &str,
        now_epoch_s: u64,
        audit_chain: &mut AuditChain,
    ) -> Result<(), ShardError> {
        let current = self
            .shard_states
            .get(shard_name)
            .copied()
            .ok_or(ShardError::NotFound)?;

        match current {
            ShardState::Created | ShardState::Stopped => {
                self.platform.start_namespace(shard_name)?;
                self.shard_states
                    .insert(shard_name.to_string(), ShardState::Running);
                self.record_transition(
                    shard_name,
                    Some(current),
                    Some(ShardState::Running),
                    now_epoch_s,
                );
                audit_chain.append(
                    "shardd.transition",
                    format!("{shard_name}:{current:?}->Running"),
                );
                Ok(())
            }
            ShardState::Running => Err(ShardError::InvalidTransition {
                from: Some(current),
                attempted: "start",
            }),
        }
    }

    pub fn stop_shard(
        &mut self,
        shard_name: &str,
        now_epoch_s: u64,
        audit_chain: &mut AuditChain,
    ) -> Result<(), ShardError> {
        let current = self
            .shard_states
            .get(shard_name)
            .copied()
            .ok_or(ShardError::NotFound)?;

        if current != ShardState::Running {
            return Err(ShardError::InvalidTransition {
                from: Some(current),
                attempted: "stop",
            });
        }

        self.platform.stop_namespace(shard_name)?;
        self.shard_states
            .insert(shard_name.to_string(), ShardState::Stopped);
        self.record_transition(
            shard_name,
            Some(current),
            Some(ShardState::Stopped),
            now_epoch_s,
        );
        audit_chain.append(
            "shardd.transition",
            format!("{shard_name}:{current:?}->Stopped"),
        );
        Ok(())
    }

    pub fn destroy_shard(
        &mut self,
        shard_name: &str,
        now_epoch_s: u64,
        audit_chain: &mut AuditChain,
    ) -> Result<(), ShardError> {
        let current = self
            .shard_states
            .get(shard_name)
            .copied()
            .ok_or(ShardError::NotFound)?;

        if current != ShardState::Stopped {
            return Err(ShardError::InvalidTransition {
                from: Some(current),
                attempted: "destroy",
            });
        }

        self.platform.destroy_namespace(shard_name)?;
        self.shard_states.remove(shard_name);
        self.record_transition(shard_name, Some(current), None, now_epoch_s);
        audit_chain.append(
            "shardd.transition",
            format!("{shard_name}:{current:?}->None"),
        );
        Ok(())
    }

    pub fn state_of(&self, shard_name: &str) -> Option<ShardState> {
        self.shard_states.get(shard_name).copied()
    }

    pub fn transitions(&self) -> &[ShardTransitionRecord] {
        &self.transitions
    }

    pub fn save_runtime_state(&self, path: &Path) -> Result<(), ShardError> {
        save_state(path, &self.runtime_state())
            .map_err(|error| ShardError::PersistenceFailure(error.to_string()))
    }

    pub fn load_runtime_state(&mut self, path: &Path) -> Result<(), ShardError> {
        let Some(state) = load_state::<ShardRuntimeState>(path)
            .map_err(|error| ShardError::PersistenceFailure(error.to_string()))?
        else {
            return Ok(());
        };

        self.apply_runtime_state(state);
        Ok(())
    }

    pub fn runtime_state(&self) -> ShardRuntimeState {
        ShardRuntimeState {
            shard_states: self.shard_states.clone(),
            transitions: self.transitions.clone(),
        }
    }

    pub fn apply_runtime_state(&mut self, state: ShardRuntimeState) {
        self.shard_states = state.shard_states;
        self.transitions = state.transitions;
    }

    fn record_transition(
        &mut self,
        shard_name: &str,
        from: Option<ShardState>,
        to: Option<ShardState>,
        at_epoch_s: u64,
    ) {
        self.transitions.push(ShardTransitionRecord {
            shard_name: shard_name.to_string(),
            from,
            to,
            at_epoch_s,
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardLifecyclePayload {
    pub shard_name: String,
    pub now_epoch_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardStatePayload {
    pub shard_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShardStateResponse {
    pub state: Option<String>,
}

#[derive(Debug)]
pub struct ShardIpcHandler<P: NamespaceBoundary> {
    pub manager: ShardManager<P>,
    pub audit_chain: AuditChain,
}

impl<P: NamespaceBoundary> ShardIpcHandler<P> {
    pub fn new(manager: ShardManager<P>) -> Self {
        Self {
            manager,
            audit_chain: AuditChain::default(),
        }
    }
}

impl<P: NamespaceBoundary> IpcMethodHandler for ShardIpcHandler<P> {
    fn handle(
        &mut self,
        auth_context: &AuthContext,
        request: IpcRequest,
    ) -> Result<IpcResponse, IpcError> {
        match request.method.as_str() {
            "CreateShard" => {
                require_role(auth_context, "shard-admin")?;
                let payload = decode_payload::<ShardLifecyclePayload>(&request)?;
                self.manager
                    .create_shard(
                        &payload.shard_name,
                        payload.now_epoch_s,
                        &mut self.audit_chain,
                    )
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&serde_json::json!({"created": true}))
            }
            "StartShard" => {
                require_role(auth_context, "shard-admin")?;
                let payload = decode_payload::<ShardLifecyclePayload>(&request)?;
                self.manager
                    .start_shard(
                        &payload.shard_name,
                        payload.now_epoch_s,
                        &mut self.audit_chain,
                    )
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&serde_json::json!({"started": true}))
            }
            "StopShard" => {
                require_role(auth_context, "shard-admin")?;
                let payload = decode_payload::<ShardLifecyclePayload>(&request)?;
                self.manager
                    .stop_shard(
                        &payload.shard_name,
                        payload.now_epoch_s,
                        &mut self.audit_chain,
                    )
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&serde_json::json!({"stopped": true}))
            }
            "DestroyShard" => {
                require_role(auth_context, "shard-admin")?;
                let payload = decode_payload::<ShardLifecyclePayload>(&request)?;
                self.manager
                    .destroy_shard(
                        &payload.shard_name,
                        payload.now_epoch_s,
                        &mut self.audit_chain,
                    )
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&serde_json::json!({"destroyed": true}))
            }
            "GetShardState" => {
                require_role(auth_context, "shard-read")?;
                let payload = decode_payload::<ShardStatePayload>(&request)?;
                let state = self.manager.state_of(&payload.shard_name).map(|state| {
                    match state {
                        ShardState::Created => "Created",
                        ShardState::Running => "Running",
                        ShardState::Stopped => "Stopped",
                    }
                    .to_string()
                });

                success_payload(&ShardStateResponse { state })
            }
            _ => Err(IpcError::UnknownMethod(request.method)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    #[derive(Default)]
    struct RecordingPlatform {
        calls: RefCell<Vec<String>>,
    }

    impl RecordingPlatform {
        fn calls(&self) -> Vec<String> {
            self.calls.borrow().clone()
        }
    }

    impl NamespaceBoundary for RecordingPlatform {
        fn create_namespace(&self, shard_name: &str) -> Result<(), ShardError> {
            self.calls.borrow_mut().push(format!("create:{shard_name}"));
            Ok(())
        }

        fn start_namespace(&self, shard_name: &str) -> Result<(), ShardError> {
            self.calls.borrow_mut().push(format!("start:{shard_name}"));
            Ok(())
        }

        fn stop_namespace(&self, shard_name: &str) -> Result<(), ShardError> {
            self.calls.borrow_mut().push(format!("stop:{shard_name}"));
            Ok(())
        }

        fn destroy_namespace(&self, shard_name: &str) -> Result<(), ShardError> {
            self.calls
                .borrow_mut()
                .push(format!("destroy:{shard_name}"));
            Ok(())
        }
    }

    #[test]
    fn lifecycle_create_start_stop_destroy_succeeds() {
        let platform = RecordingPlatform::default();
        let mut manager = ShardManager::new(platform);
        let mut audit_chain = AuditChain::default();

        assert!(manager.create_shard("work", 1, &mut audit_chain).is_ok());
        assert!(manager.start_shard("work", 2, &mut audit_chain).is_ok());
        assert!(manager.stop_shard("work", 3, &mut audit_chain).is_ok());
        assert!(manager.destroy_shard("work", 4, &mut audit_chain).is_ok());
        assert!(manager.state_of("work").is_none());
        assert_eq!(manager.transitions().len(), 4);
    }

    #[test]
    fn invalid_transition_is_rejected() {
        let platform = RecordingPlatform::default();
        let mut manager = ShardManager::new(platform);
        let mut audit_chain = AuditChain::default();

        assert!(matches!(
            manager.start_shard("work", 1, &mut audit_chain),
            Err(ShardError::NotFound)
        ));

        assert!(manager.create_shard("work", 2, &mut audit_chain).is_ok());
        assert!(matches!(
            manager.destroy_shard("work", 3, &mut audit_chain),
            Err(ShardError::InvalidTransition { .. })
        ));
    }

    #[test]
    fn namespace_boundary_calls_are_recorded() {
        let platform = RecordingPlatform::default();
        let mut manager = ShardManager::new(platform);
        let mut audit_chain = AuditChain::default();

        assert!(manager.create_shard("anon", 10, &mut audit_chain).is_ok());
        assert!(manager.start_shard("anon", 11, &mut audit_chain).is_ok());
        assert!(manager.stop_shard("anon", 12, &mut audit_chain).is_ok());
        assert!(manager.destroy_shard("anon", 13, &mut audit_chain).is_ok());

        let calls = manager.platform.calls();
        assert_eq!(
            calls,
            vec!["create:anon", "start:anon", "stop:anon", "destroy:anon"]
        );
    }

    #[test]
    fn runtime_state_recovers_after_crash_artifact() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let path = temp.path().join("shardd-state.json");

        let platform = RecordingPlatform::default();
        let mut manager = ShardManager::new(platform);
        let mut audit_chain = AuditChain::default();

        assert!(manager.create_shard("work", 1, &mut audit_chain).is_ok());
        assert!(manager.start_shard("work", 2, &mut audit_chain).is_ok());
        manager
            .save_runtime_state(&path)
            .expect("state should be persisted");

        let crash_tmp = path.with_extension("json.tmp");
        std::fs::rename(&path, &crash_tmp).expect("state should move to tmp for crash simulation");

        let platform = RecordingPlatform::default();
        let mut recovered = ShardManager::new(platform);
        recovered
            .load_runtime_state(&path)
            .expect("state should recover from tmp file");

        assert_eq!(recovered.state_of("work"), Some(ShardState::Running));
        assert_eq!(recovered.transitions().len(), 2);
    }

    #[test]
    fn ipc_handler_enforces_authz_boundary() {
        let platform = RecordingPlatform::default();
        let manager = ShardManager::new(platform);
        let mut handler = ShardIpcHandler::new(manager);

        let request = IpcRequest {
            method: "CreateShard".to_string(),
            payload: serde_json::to_string(&ShardLifecyclePayload {
                shard_name: "work".to_string(),
                now_epoch_s: 1,
            })
            .expect("payload should encode"),
        };
        let auth_context = AuthContext {
            caller_id: "app://client".to_string(),
            roles: vec!["shard-read".to_string()],
        };

        let response = handler.handle(&auth_context, request);
        assert!(matches!(response, Err(IpcError::Unauthorized { .. })));
    }

    #[test]
    fn ipc_handler_routes_lifecycle_calls() {
        let platform = RecordingPlatform::default();
        let manager = ShardManager::new(platform);
        let mut handler = ShardIpcHandler::new(manager);
        let auth_context = AuthContext {
            caller_id: "daemon://controller".to_string(),
            roles: vec!["shard-admin".to_string(), "shard-read".to_string()],
        };

        let create_request = IpcRequest {
            method: "CreateShard".to_string(),
            payload: serde_json::to_string(&ShardLifecyclePayload {
                shard_name: "work".to_string(),
                now_epoch_s: 1,
            })
            .expect("payload should encode"),
        };
        assert!(handler.handle(&auth_context, create_request).is_ok());

        let state_request = IpcRequest {
            method: "GetShardState".to_string(),
            payload: serde_json::to_string(&ShardStatePayload {
                shard_name: "work".to_string(),
            })
            .expect("payload should encode"),
        };
        let response = handler
            .handle(&auth_context, state_request)
            .expect("state query should succeed");
        let state: ShardStateResponse =
            serde_json::from_str(&response.payload).expect("state response should decode");
        assert_eq!(state.state.as_deref(), Some("Created"));
    }
}
