use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;

use gk_audit::{AuditStore, AuditStoreError, SignedAuditEvent};
use gk_ipc::{
    decode_payload, require_role, success_payload, AuthContext, IpcError, IpcMethodHandler,
    IpcRequest, IpcResponse,
};
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub enum AuditdError {
    Store(AuditStoreError),
}

impl Display for AuditdError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AuditdError::Store(error) => write!(formatter, "audit store error: {error}"),
        }
    }
}

impl Error for AuditdError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifyChainResult {
    pub valid: bool,
    pub reason: Option<String>,
}

pub struct AuditDaemon {
    store: AuditStore,
}

impl AuditDaemon {
    pub fn open(path: &Path) -> Result<Self, AuditdError> {
        let store = AuditStore::open(path).map_err(AuditdError::Store)?;
        let _ = store.recover_truncated_tail().map_err(AuditdError::Store)?;
        Ok(Self { store })
    }

    pub fn append_event(&self, event_type: &str, payload: &str) -> Result<u64, AuditdError> {
        self.store
            .append_event(event_type, payload)
            .map(|event| event.sequence)
            .map_err(AuditdError::Store)
    }

    pub fn verify_chain(&self) -> Result<VerifyChainResult, AuditdError> {
        match self.store.replay_and_verify() {
            Ok(_) => Ok(VerifyChainResult {
                valid: true,
                reason: None,
            }),
            Err(AuditStoreError::Integrity(error)) => Ok(VerifyChainResult {
                valid: false,
                reason: Some(error.to_string()),
            }),
            Err(error) => Err(AuditdError::Store(error)),
        }
    }

    pub fn query_events(&self, from_sequence: u64) -> Result<Vec<SignedAuditEvent>, AuditdError> {
        let replay = self.store.replay_and_verify().map_err(AuditdError::Store)?;
        let lower_bound = from_sequence.max(1);
        Ok(replay
            .into_iter()
            .filter(|event| event.sequence >= lower_bound)
            .collect())
    }

    pub fn recover_truncated_tail(&self) -> Result<usize, AuditdError> {
        self.store
            .recover_truncated_tail()
            .map_err(AuditdError::Store)
    }

    pub fn store_path(&self) -> &Path {
        self.store.path()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEventPayload {
    pub event_type: String,
    pub payload: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppendEventResponse {
    pub sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEventsPayload {
    pub from_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryEventsResponse {
    pub events: Vec<SignedAuditEvent>,
}

pub struct AuditIpcHandler {
    pub daemon: AuditDaemon,
}

impl AuditIpcHandler {
    pub fn new(daemon: AuditDaemon) -> Self {
        Self { daemon }
    }
}

impl IpcMethodHandler for AuditIpcHandler {
    fn handle(
        &mut self,
        auth_context: &AuthContext,
        request: IpcRequest,
    ) -> Result<IpcResponse, IpcError> {
        match request.method.as_str() {
            "AppendEvent" => {
                require_role(auth_context, "audit-write")?;
                let payload = decode_payload::<AppendEventPayload>(&request)?;
                let sequence = self
                    .daemon
                    .append_event(&payload.event_type, &payload.payload)
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&AppendEventResponse { sequence })
            }
            "VerifyChain" => {
                require_role(auth_context, "audit-read")?;
                let result = self
                    .daemon
                    .verify_chain()
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&result)
            }
            "QueryEvents" => {
                require_role(auth_context, "audit-read")?;
                let payload = decode_payload::<QueryEventsPayload>(&request)?;
                let events = self
                    .daemon
                    .query_events(payload.from_sequence)
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&QueryEventsResponse { events })
            }
            _ => Err(IpcError::UnknownMethod(request.method)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn daemon_persists_and_recovers_events() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let path = temp.path().join("auditd/chain.log");

        let daemon = AuditDaemon::open(&path).expect("daemon should initialize");
        let first = daemon
            .append_event("daemon.start", "phantomkernel-auditd")
            .expect("append should succeed");
        let second = daemon
            .append_event("policy.decision", "allow")
            .expect("append should succeed");

        assert_eq!(first, 1);
        assert_eq!(second, 2);

        let restarted = AuditDaemon::open(&path).expect("daemon should restart");
        let report = restarted.verify_chain().expect("verify should execute");
        assert!(report.valid);
        assert!(report.reason.is_none());
        assert_eq!(
            restarted.query_events(1).expect("query should work").len(),
            2
        );
    }

    #[test]
    fn daemon_detects_chain_tampering() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let path = temp.path().join("auditd/chain.log");
        let daemon = AuditDaemon::open(&path).expect("daemon should initialize");

        let _ = daemon
            .append_event("policy.decision", "allow")
            .expect("append should succeed");
        let _ = daemon
            .append_event("shard.start", "work")
            .expect("append should succeed");

        let content = fs::read_to_string(daemon.store_path()).expect("audit file should read");
        let mut lines = content.lines().map(ToString::to_string).collect::<Vec<_>>();
        let mut event: SignedAuditEvent =
            serde_json::from_str(&lines[1]).expect("event should decode");
        event.payload = "tampered".to_string();
        lines[1] = serde_json::to_string(&event).expect("event should encode");

        let mut rewritten = String::new();
        for line in lines {
            rewritten.push_str(&line);
            rewritten.push('\n');
        }
        fs::write(daemon.store_path(), rewritten).expect("tampered file should write");

        let report = daemon.verify_chain().expect("verify should execute");
        assert!(!report.valid);
        assert!(report.reason.is_some());
    }

    #[test]
    fn ipc_handler_enforces_authz_boundary() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let daemon = AuditDaemon::open(&temp.path().join("auditd/chain.log"))
            .expect("daemon should initialize");
        let mut handler = AuditIpcHandler::new(daemon);

        let request = IpcRequest {
            method: "AppendEvent".to_string(),
            payload: serde_json::to_string(&AppendEventPayload {
                event_type: "policy.decision".to_string(),
                payload: "allow".to_string(),
            })
            .expect("payload should encode"),
        };
        let auth_context = AuthContext {
            caller_id: "app://observer".to_string(),
            roles: vec!["audit-read".to_string()],
        };

        let response = handler.handle(&auth_context, request);
        assert!(matches!(response, Err(IpcError::Unauthorized { .. })));
    }

    #[test]
    fn ipc_handler_appends_and_queries_events() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let daemon = AuditDaemon::open(&temp.path().join("auditd/chain.log"))
            .expect("daemon should initialize");
        let mut handler = AuditIpcHandler::new(daemon);
        let write_auth = AuthContext {
            caller_id: "daemon://policyd".to_string(),
            roles: vec!["audit-write".to_string(), "audit-read".to_string()],
        };

        let append_request = IpcRequest {
            method: "AppendEvent".to_string(),
            payload: serde_json::to_string(&AppendEventPayload {
                event_type: "policyd.token.issued".to_string(),
                payload: "tok-1".to_string(),
            })
            .expect("payload should encode"),
        };
        let append_response = handler
            .handle(&write_auth, append_request)
            .expect("append should succeed");
        let append_result: AppendEventResponse =
            serde_json::from_str(&append_response.payload).expect("response should decode");
        assert_eq!(append_result.sequence, 1);

        let verify_request = IpcRequest {
            method: "VerifyChain".to_string(),
            payload: "{}".to_string(),
        };
        let verify_response = handler
            .handle(&write_auth, verify_request)
            .expect("verify should succeed");
        let verify_result: VerifyChainResult =
            serde_json::from_str(&verify_response.payload).expect("response should decode");
        assert!(verify_result.valid);

        let query_request = IpcRequest {
            method: "QueryEvents".to_string(),
            payload: serde_json::to_string(&QueryEventsPayload { from_sequence: 1 })
                .expect("payload should encode"),
        };
        let query_response = handler
            .handle(&write_auth, query_request)
            .expect("query should succeed");
        let query_result: QueryEventsResponse =
            serde_json::from_str(&query_response.payload).expect("response should decode");
        assert_eq!(query_result.events.len(), 1);
        assert_eq!(query_result.events[0].event_type, "policyd.token.issued");
    }
}
