use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;

use gk_audit::AuditChain;
use gk_crypto::{CryptoError, KeyRecord, KeyRing, SignatureEnvelope};
use gk_ipc::{
    decode_payload, require_role, success_payload, AuthContext, IpcError, IpcMethodHandler,
    IpcRequest, IpcResponse,
};
use gk_persistence::{load_state, save_state, PersistedState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CapabilityRule {
    pub subject: String,
    pub shard: String,
    pub resource: String,
    pub action: String,
}

impl CapabilityRule {
    pub fn new(
        subject: impl Into<String>,
        shard: impl Into<String>,
        resource: impl Into<String>,
        action: impl Into<String>,
    ) -> Self {
        Self {
            subject: subject.into(),
            shard: shard.into(),
            resource: resource.into(),
            action: action.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRequest {
    pub subject: String,
    pub shard: String,
    pub resource: String,
    pub action: String,
    pub ttl_seconds: u64,
}

impl CapabilityRequest {
    pub fn rule_key(&self) -> CapabilityRule {
        CapabilityRule::new(
            self.subject.clone(),
            self.shard.clone(),
            self.resource.clone(),
            self.action.clone(),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityToken {
    pub token_id: String,
    pub subject: String,
    pub shard: String,
    pub resource: String,
    pub action: String,
    pub issued_at_epoch_s: u64,
    pub expires_at_epoch_s: u64,
    pub signature: SignatureEnvelope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyError {
    DenyByDefault,
    ExpiredToken,
    InvalidSignature,
    SigningKeyExpired,
    SigningKeyRevoked,
    ShardMismatch { expected: String, actual: String },
    RevokedToken,
    UnknownToken,
    CryptoFailure(String),
    PersistenceFailure(String),
}

impl Display for PolicyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyError::DenyByDefault => write!(formatter, "request denied by default policy"),
            PolicyError::ExpiredToken => write!(formatter, "token has expired"),
            PolicyError::InvalidSignature => write!(formatter, "token signature is invalid"),
            PolicyError::SigningKeyExpired => {
                write!(formatter, "signing key for token has expired")
            }
            PolicyError::SigningKeyRevoked => {
                write!(formatter, "signing key for token has been revoked")
            }
            PolicyError::ShardMismatch { expected, actual } => write!(
                formatter,
                "token shard mismatch: expected '{expected}', actual '{actual}'"
            ),
            PolicyError::RevokedToken => write!(formatter, "token has been revoked"),
            PolicyError::UnknownToken => write!(formatter, "token was not issued by this service"),
            PolicyError::CryptoFailure(message) => write!(formatter, "crypto failure: {message}"),
            PolicyError::PersistenceFailure(message) => {
                write!(formatter, "persistence failure: {message}")
            }
        }
    }
}

impl Error for PolicyError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyRuntimeState {
    pub allow_rules: HashSet<CapabilityRule>,
    pub issued_tokens: HashMap<String, CapabilityToken>,
    pub revoked_tokens: HashSet<String>,
    pub next_token_nonce: u64,
}

impl PersistedState for PolicyRuntimeState {
    const STATE_KIND: &'static str = "specteros-policyd-runtime";
    const CURRENT_SCHEMA_VERSION: u32 = 1;
}

#[derive(Debug, Clone)]
pub struct PolicyService {
    key_ring: KeyRing,
    allow_rules: HashSet<CapabilityRule>,
    issued_tokens: HashMap<String, CapabilityToken>,
    revoked_tokens: HashSet<String>,
    next_token_nonce: u64,
}

impl PolicyService {
    pub fn new(signing_secret: impl Into<String>) -> Self {
        Self::with_key_ring(KeyRing::new("key-primary", signing_secret))
    }

    pub fn with_key_ring(key_ring: KeyRing) -> Self {
        Self {
            key_ring,
            allow_rules: HashSet::new(),
            issued_tokens: HashMap::new(),
            revoked_tokens: HashSet::new(),
            next_token_nonce: 1,
        }
    }

    pub fn allow_rule(&mut self, rule: CapabilityRule) -> bool {
        self.allow_rules.insert(rule)
    }

    pub fn rotate_signing_key(
        &mut self,
        new_key: KeyRecord,
        activate: bool,
        audit_chain: &mut AuditChain,
    ) {
        let key_id = new_key.key_id.clone();
        self.key_ring.rotate(new_key, activate);
        audit_chain.append(
            "policyd.key.rotated",
            format!("key_id={key_id} active={activate}"),
        );
    }

    pub fn revoke_signing_key(&mut self, key_id: &str, audit_chain: &mut AuditChain) -> bool {
        let revoked = self.key_ring.revoke_key(key_id);
        if revoked {
            audit_chain.append("policyd.key.revoked", key_id.to_string());
        }
        revoked
    }

    pub fn issue_token(
        &mut self,
        request: &CapabilityRequest,
        now_epoch_s: u64,
        audit_chain: &mut AuditChain,
    ) -> Result<CapabilityToken, PolicyError> {
        if !self.allow_rules.contains(&request.rule_key()) {
            let payload = format!(
                "subject={} shard={} resource={} action={}",
                request.subject, request.shard, request.resource, request.action
            );
            audit_chain.append("policyd.request.denied", payload);
            return Err(PolicyError::DenyByDefault);
        }

        let token_id = format!("tok-{:016x}", self.next_token_nonce);
        self.next_token_nonce = self.next_token_nonce.saturating_add(1);
        let expires_at_epoch_s = now_epoch_s.saturating_add(request.ttl_seconds);

        let mut token = CapabilityToken {
            token_id: token_id.clone(),
            subject: request.subject.clone(),
            shard: request.shard.clone(),
            resource: request.resource.clone(),
            action: request.action.clone(),
            issued_at_epoch_s: now_epoch_s,
            expires_at_epoch_s,
            signature: SignatureEnvelope {
                key_id: String::new(),
                algorithm: String::new(),
                value_hex: String::new(),
            },
        };

        token.signature = self
            .key_ring
            .sign(&self.signature_payload(&token), now_epoch_s)
            .map_err(map_crypto_error)?;

        self.issued_tokens.insert(token_id.clone(), token.clone());
        audit_chain.append("policyd.token.issued", token_id);

        Ok(token)
    }

    pub fn validate_token(
        &self,
        token: &CapabilityToken,
        expected_shard: &str,
        now_epoch_s: u64,
        audit_chain: &mut AuditChain,
    ) -> Result<(), PolicyError> {
        if self.revoked_tokens.contains(&token.token_id) {
            audit_chain.append("policyd.token.rejected", "revoked");
            return Err(PolicyError::RevokedToken);
        }

        if token.shard != expected_shard {
            audit_chain.append("policyd.token.rejected", "shard-mismatch");
            return Err(PolicyError::ShardMismatch {
                expected: expected_shard.to_string(),
                actual: token.shard.clone(),
            });
        }

        if now_epoch_s >= token.expires_at_epoch_s {
            audit_chain.append("policyd.token.rejected", "expired");
            return Err(PolicyError::ExpiredToken);
        }

        let Some(issued) = self.issued_tokens.get(&token.token_id) else {
            audit_chain.append("policyd.token.rejected", "unknown-token");
            return Err(PolicyError::UnknownToken);
        };

        if token != issued {
            audit_chain.append("policyd.token.rejected", "tamper-detected");
            return Err(PolicyError::InvalidSignature);
        }

        self.key_ring
            .verify(
                &self.signature_payload(token),
                &token.signature,
                now_epoch_s,
            )
            .map_err(|error| {
                audit_chain.append("policyd.token.rejected", "invalid-signature");
                map_crypto_error(error)
            })?;

        audit_chain.append("policyd.token.validated", token.token_id.clone());
        Ok(())
    }

    pub fn revoke_token(&mut self, token_id: &str, audit_chain: &mut AuditChain) -> bool {
        if !self.issued_tokens.contains_key(token_id) {
            audit_chain.append("policyd.token.revoke-missing", token_id.to_string());
            return false;
        }

        let inserted = self.revoked_tokens.insert(token_id.to_string());
        if inserted {
            audit_chain.append("policyd.token.revoked", token_id.to_string());
        }
        inserted
    }

    pub fn save_runtime_state(&self, path: &Path) -> Result<(), PolicyError> {
        save_state(path, &self.runtime_state())
            .map_err(|error| PolicyError::PersistenceFailure(error.to_string()))
    }

    pub fn load_runtime_state(&mut self, path: &Path) -> Result<(), PolicyError> {
        let Some(state) = load_state::<PolicyRuntimeState>(path)
            .map_err(|error| PolicyError::PersistenceFailure(error.to_string()))?
        else {
            return Ok(());
        };

        self.apply_runtime_state(state);
        Ok(())
    }

    pub fn runtime_state(&self) -> PolicyRuntimeState {
        PolicyRuntimeState {
            allow_rules: self.allow_rules.clone(),
            issued_tokens: self.issued_tokens.clone(),
            revoked_tokens: self.revoked_tokens.clone(),
            next_token_nonce: self.next_token_nonce,
        }
    }

    pub fn apply_runtime_state(&mut self, state: PolicyRuntimeState) {
        self.allow_rules = state.allow_rules;
        self.issued_tokens = state.issued_tokens;
        self.revoked_tokens = state.revoked_tokens;
        self.next_token_nonce = state.next_token_nonce;
    }

    fn signature_payload(&self, token: &CapabilityToken) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}",
            token.token_id,
            token.subject,
            token.shard,
            token.resource,
            token.action,
            token.issued_at_epoch_s,
            token.expires_at_epoch_s,
        )
    }
}

fn map_crypto_error(error: CryptoError) -> PolicyError {
    match error {
        CryptoError::KeyExpired(_) => PolicyError::SigningKeyExpired,
        CryptoError::KeyRevoked(_) => PolicyError::SigningKeyRevoked,
        CryptoError::InvalidSignature => PolicyError::InvalidSignature,
        CryptoError::KeyNotFound(_) | CryptoError::InvalidSignatureEncoding => {
            PolicyError::InvalidSignature
        }
        other => PolicyError::CryptoFailure(other.to_string()),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateCapabilityPayload {
    pub request: CapabilityRequest,
    pub now_epoch_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateCapabilityPayload {
    pub token: CapabilityToken,
    pub expected_shard: String,
    pub now_epoch_s: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResult {
    pub valid: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevokeCapabilityPayload {
    pub token_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateSigningKeyPayload {
    pub key_id: String,
    pub secret: String,
    pub not_after_epoch_s: Option<u64>,
    pub activate: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RotateSigningKeyResponse {
    pub active_key_id: String,
}

#[derive(Debug)]
pub struct PolicyIpcHandler {
    pub service: PolicyService,
    pub audit_chain: AuditChain,
}

impl PolicyIpcHandler {
    pub fn new(service: PolicyService) -> Self {
        Self {
            service,
            audit_chain: AuditChain::default(),
        }
    }
}

impl IpcMethodHandler for PolicyIpcHandler {
    fn handle(
        &mut self,
        auth_context: &AuthContext,
        request: IpcRequest,
    ) -> Result<IpcResponse, IpcError> {
        match request.method.as_str() {
            "EvaluateCapability" => {
                require_role(auth_context, "policy-client")?;
                let payload = decode_payload::<EvaluateCapabilityPayload>(&request)?;
                let token = self
                    .service
                    .issue_token(&payload.request, payload.now_epoch_s, &mut self.audit_chain)
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&token)
            }
            "ValidateCapability" => {
                require_role(auth_context, "policy-client")?;
                let payload = decode_payload::<ValidateCapabilityPayload>(&request)?;
                let outcome = self.service.validate_token(
                    &payload.token,
                    &payload.expected_shard,
                    payload.now_epoch_s,
                    &mut self.audit_chain,
                );

                let response = match outcome {
                    Ok(()) => ValidationResult {
                        valid: true,
                        reason: None,
                    },
                    Err(error) => ValidationResult {
                        valid: false,
                        reason: Some(error.to_string()),
                    },
                };
                success_payload(&response)
            }
            "RevokeCapability" => {
                require_role(auth_context, "policy-admin")?;
                let payload = decode_payload::<RevokeCapabilityPayload>(&request)?;
                let revoked = self
                    .service
                    .revoke_token(&payload.token_id, &mut self.audit_chain);
                success_payload(&serde_json::json!({"revoked": revoked}))
            }
            "RotateSigningKey" => {
                require_role(auth_context, "policy-admin")?;
                let payload = decode_payload::<RotateSigningKeyPayload>(&request)?;
                self.service.rotate_signing_key(
                    KeyRecord::new(payload.key_id, payload.secret, payload.not_after_epoch_s),
                    payload.activate,
                    &mut self.audit_chain,
                );

                let response = RotateSigningKeyResponse {
                    active_key_id: self.service.key_ring.active_key_id().to_string(),
                };
                success_payload(&response)
            }
            _ => Err(IpcError::UnknownMethod(request.method)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allow_network_rule(service: &mut PolicyService) {
        let inserted = service.allow_rule(CapabilityRule::new(
            "app://mail",
            "work",
            "network",
            "connect",
        ));
        assert!(inserted);
    }

    fn issue_test_token(
        service: &mut PolicyService,
        audit_chain: &mut AuditChain,
    ) -> CapabilityToken {
        allow_network_rule(service);
        let request = CapabilityRequest {
            subject: "app://mail".to_string(),
            shard: "work".to_string(),
            resource: "network".to_string(),
            action: "connect".to_string(),
            ttl_seconds: 30,
        };

        match service.issue_token(&request, 10, audit_chain) {
            Ok(token) => token,
            Err(error) => panic!("failed to issue token in test: {error}"),
        }
    }

    #[test]
    fn deny_by_default_without_allow_rule() {
        let mut service = PolicyService::new("test-key");
        let mut audit_chain = AuditChain::default();
        let request = CapabilityRequest {
            subject: "app://mail".to_string(),
            shard: "work".to_string(),
            resource: "network".to_string(),
            action: "connect".to_string(),
            ttl_seconds: 30,
        };

        let outcome = service.issue_token(&request, 100, &mut audit_chain);
        assert!(matches!(outcome, Err(PolicyError::DenyByDefault)));
    }

    #[test]
    fn expiry_is_enforced() {
        let mut service = PolicyService::new("test-key");
        let mut audit_chain = AuditChain::default();
        let token = issue_test_token(&mut service, &mut audit_chain);

        let outcome =
            service.validate_token(&token, "work", token.expires_at_epoch_s, &mut audit_chain);
        assert!(matches!(outcome, Err(PolicyError::ExpiredToken)));
    }

    #[test]
    fn tamper_detection_rejects_modified_tokens() {
        let mut service = PolicyService::new("test-key");
        let mut audit_chain = AuditChain::default();
        let mut token = issue_test_token(&mut service, &mut audit_chain);
        token.resource = "filesystem".to_string();

        let outcome = service.validate_token(&token, "work", 20, &mut audit_chain);
        assert!(matches!(outcome, Err(PolicyError::InvalidSignature)));
    }

    #[test]
    fn shard_mismatch_is_rejected() {
        let mut service = PolicyService::new("test-key");
        let mut audit_chain = AuditChain::default();
        let token = issue_test_token(&mut service, &mut audit_chain);

        let outcome = service.validate_token(&token, "anon", 20, &mut audit_chain);
        assert!(matches!(outcome, Err(PolicyError::ShardMismatch { .. })));
    }

    #[test]
    fn revoked_tokens_are_rejected() {
        let mut service = PolicyService::new("test-key");
        let mut audit_chain = AuditChain::default();
        let token = issue_test_token(&mut service, &mut audit_chain);

        let revoked = service.revoke_token(&token.token_id, &mut audit_chain);
        assert!(revoked);

        let outcome = service.validate_token(&token, "work", 20, &mut audit_chain);
        assert!(matches!(outcome, Err(PolicyError::RevokedToken)));
    }

    #[test]
    fn signing_key_rollover_keeps_existing_tokens_valid() {
        let mut service = PolicyService::new("test-key-old");
        let mut audit_chain = AuditChain::default();
        let token = issue_test_token(&mut service, &mut audit_chain);

        service.rotate_signing_key(
            KeyRecord::new("key-next", "test-key-new", None),
            true,
            &mut audit_chain,
        );

        let validation = service.validate_token(&token, "work", 20, &mut audit_chain);
        assert!(validation.is_ok());
    }

    #[test]
    fn runtime_state_survives_crash_recovery() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let path = temp.path().join("policyd-state.json");
        let mut service = PolicyService::new("test-key");
        let mut audit_chain = AuditChain::default();
        let token = issue_test_token(&mut service, &mut audit_chain);
        let _ = service.revoke_token(&token.token_id, &mut audit_chain);
        service
            .save_runtime_state(&path)
            .expect("state should save to disk");

        let crash_tmp = path.with_extension("json.tmp");
        std::fs::rename(&path, &crash_tmp).expect("state should move to tmp for crash simulation");

        let mut recovered = PolicyService::new("test-key");
        recovered
            .load_runtime_state(&path)
            .expect("state should recover from tmp");

        let validation = recovered.validate_token(&token, "work", 20, &mut audit_chain);
        assert!(matches!(validation, Err(PolicyError::RevokedToken)));
    }

    #[test]
    fn ipc_handler_enforces_authz_boundary() {
        let mut service = PolicyService::new("test-key");
        allow_network_rule(&mut service);
        let mut handler = PolicyIpcHandler::new(service);
        let request = IpcRequest {
            method: "RevokeCapability".to_string(),
            payload: serde_json::to_string(&RevokeCapabilityPayload {
                token_id: "tok-1".to_string(),
            })
            .expect("payload should encode"),
        };

        let auth_context = AuthContext {
            caller_id: "app://client".to_string(),
            roles: vec!["policy-client".to_string()],
        };
        let response = handler.handle(&auth_context, request);
        assert!(matches!(response, Err(IpcError::Unauthorized { .. })));
    }

    #[test]
    fn ipc_handler_routes_issue_and_validate() {
        let mut service = PolicyService::new("test-key");
        allow_network_rule(&mut service);
        let mut handler = PolicyIpcHandler::new(service);

        let issue_request = IpcRequest {
            method: "EvaluateCapability".to_string(),
            payload: serde_json::to_string(&EvaluateCapabilityPayload {
                request: CapabilityRequest {
                    subject: "app://mail".to_string(),
                    shard: "work".to_string(),
                    resource: "network".to_string(),
                    action: "connect".to_string(),
                    ttl_seconds: 30,
                },
                now_epoch_s: 10,
            })
            .expect("payload should encode"),
        };

        let auth_context = AuthContext {
            caller_id: "app://client".to_string(),
            roles: vec!["policy-client".to_string()],
        };
        let issue_response = handler
            .handle(&auth_context, issue_request)
            .expect("issue should succeed");
        let token: CapabilityToken =
            serde_json::from_str(&issue_response.payload).expect("token should decode");

        let validate_request = IpcRequest {
            method: "ValidateCapability".to_string(),
            payload: serde_json::to_string(&ValidateCapabilityPayload {
                token,
                expected_shard: "work".to_string(),
                now_epoch_s: 11,
            })
            .expect("payload should encode"),
        };
        let validate_response = handler
            .handle(&auth_context, validate_request)
            .expect("validate should respond");
        let result: ValidationResult =
            serde_json::from_str(&validate_response.payload).expect("response should decode");
        assert!(result.valid);
    }
}
