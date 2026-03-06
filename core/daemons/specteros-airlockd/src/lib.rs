use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;

use gk_audit::AuditChain;
use gk_ipc::{
    decode_payload, require_role, success_payload, AuthContext, IpcError, IpcMethodHandler,
    IpcRequest, IpcResponse,
};
use gk_metadata_sanitizer::MetadataSanitizer;
use gk_persistence::{load_state, save_state, PersistedState};
use serde::{Deserialize, Serialize};

const HIGH_RISK_THRESHOLD: u8 = 70;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransferSessionState {
    Open,
    Scanned,
    Approved,
    Rejected,
    Committed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SniffedMime {
    Pdf,
    Png,
    Jpeg,
    PlainText,
    Executable,
    MsWord,
    MsExcel,
    MsPowerpoint,
    ZipArchive,
    TarArchive,
    GzipArchive,
    Unknown,
}

impl SniffedMime {
    fn as_str(&self) -> &'static str {
        match self {
            SniffedMime::Pdf => "application/pdf",
            SniffedMime::Png => "image/png",
            SniffedMime::Jpeg => "image/jpeg",
            SniffedMime::PlainText => "text/plain",
            SniffedMime::Executable => "application/executable",
            SniffedMime::MsWord => "application/msword",
            SniffedMime::MsExcel => "application/vnd.ms-excel",
            SniffedMime::MsPowerpoint => "application/vnd.ms-powerpoint",
            SniffedMime::ZipArchive => "application/zip",
            SniffedMime::TarArchive => "application/x-tar",
            SniffedMime::GzipArchive => "application/gzip",
            SniffedMime::Unknown => "application/unknown",
        }
    }

    pub fn risk_baseline(&self) -> u8 {
        match self {
            SniffedMime::PlainText | SniffedMime::Png | SniffedMime::Jpeg => 5,
            SniffedMime::Pdf => 15,
            SniffedMime::MsWord | SniffedMime::MsExcel | SniffedMime::MsPowerpoint => 40,
            SniffedMime::ZipArchive | SniffedMime::TarArchive | SniffedMime::GzipArchive => 35,
            SniffedMime::Executable => 95,
            SniffedMime::Unknown => 90,
        }
    }

    pub fn is_office_document(&self) -> bool {
        matches!(
            self,
            SniffedMime::MsWord | SniffedMime::MsExcel | SniffedMime::MsPowerpoint
        )
    }

    pub fn is_archive(&self) -> bool {
        matches!(
            self,
            SniffedMime::ZipArchive | SniffedMime::TarArchive | SniffedMime::GzipArchive
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactDescriptor {
    pub artifact_id: String,
    pub path: String,
    pub metadata_entries: u32,
    pub declared_mime: String,
    pub content_bytes: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SanitizerReport {
    pub metadata_stripped: bool,
    pub risk_score: u8,
    pub reason: String,
    pub sniffed_mime: String,
    pub applied_steps: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransferSession {
    pub session_id: String,
    pub source_shard: String,
    pub target_shard: String,
    pub state: TransferSessionState,
    pub artifact: Option<ArtifactDescriptor>,
    pub scan_report: Option<SanitizerReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AirlockRuntimeState {
    pub sessions: HashMap<String, TransferSession>,
    pub approved_transfers: HashSet<String>,
    pub next_session_nonce: u64,
}

impl PersistedState for AirlockRuntimeState {
    const STATE_KIND: &'static str = "specteros-airlockd-runtime";
    const CURRENT_SCHEMA_VERSION: u32 = 1;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AirlockError {
    SessionNotFound,
    InvalidState {
        expected: TransferSessionState,
        actual: TransferSessionState,
    },
    UnknownMimeRejected {
        sniffed_mime: String,
    },
    HighRiskRejected {
        risk_score: u8,
    },
    DirectTransferDenied,
    PersistenceFailure(String),
}

impl Display for AirlockError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            AirlockError::SessionNotFound => write!(formatter, "transfer session not found"),
            AirlockError::InvalidState { expected, actual } => {
                write!(
                    formatter,
                    "invalid session state: expected {expected:?}, actual {actual:?}"
                )
            }
            AirlockError::UnknownMimeRejected { sniffed_mime } => {
                write!(formatter, "unknown or unsafe MIME rejected: {sniffed_mime}")
            }
            AirlockError::HighRiskRejected { risk_score } => {
                write!(
                    formatter,
                    "artifact rejected for high risk score {risk_score}"
                )
            }
            AirlockError::DirectTransferDenied => {
                write!(formatter, "direct cross-shard transfer denied")
            }
            AirlockError::PersistenceFailure(message) => {
                write!(formatter, "persistence failure: {message}")
            }
        }
    }
}

impl Error for AirlockError {}

#[derive(Debug, Clone)]
pub struct SanitizationContext {
    pub sniffed_mime: SniffedMime,
    pub risk_score: u8,
    pub metadata_stripped: bool,
    pub notes: Vec<String>,
}

impl SanitizationContext {
    fn new(sniffed_mime: SniffedMime) -> Self {
        Self {
            sniffed_mime,
            risk_score: 0,
            metadata_stripped: false,
            notes: Vec::new(),
        }
    }
}

pub trait SanitizerAdapter {
    fn name(&self) -> &'static str;
    fn process(&self, artifact: &mut ArtifactDescriptor, context: &mut SanitizationContext);
}

pub trait SanitizerPipeline {
    fn sanitize(&self, artifact: &mut ArtifactDescriptor) -> SanitizerReport;
}

pub struct PluggableSanitizerChain {
    adapters: Vec<Box<dyn SanitizerAdapter + Send + Sync>>,
}

impl PluggableSanitizerChain {
    pub fn new(adapters: Vec<Box<dyn SanitizerAdapter + Send + Sync>>) -> Self {
        Self { adapters }
    }

    pub fn default_chain() -> Self {
        Self::new(vec![
            Box::new(MetadataStripAdapter),
            Box::new(DocumentFlattenAdapter),
            Box::new(RiskScoringAdapter),
        ])
    }
}

impl Default for PluggableSanitizerChain {
    fn default() -> Self {
        Self::default_chain()
    }
}

impl SanitizerPipeline for PluggableSanitizerChain {
    fn sanitize(&self, artifact: &mut ArtifactDescriptor) -> SanitizerReport {
        let sniffed_mime = sniff_mime(&artifact.content_bytes);
        let mut context = SanitizationContext::new(sniffed_mime);

        let mut applied_steps = Vec::with_capacity(self.adapters.len());
        for adapter in &self.adapters {
            adapter.process(artifact, &mut context);
            applied_steps.push(adapter.name().to_string());
        }

        let reason = if context.risk_score >= HIGH_RISK_THRESHOLD {
            "high-risk artifact".to_string()
        } else {
            "acceptable risk".to_string()
        };

        SanitizerReport {
            metadata_stripped: context.metadata_stripped,
            risk_score: context.risk_score,
            reason,
            sniffed_mime: context.sniffed_mime.as_str().to_string(),
            applied_steps,
        }
    }
}

pub struct MetadataStripAdapter;

impl SanitizerAdapter for MetadataStripAdapter {
    fn name(&self) -> &'static str {
        "metadata-strip"
    }

    fn process(&self, artifact: &mut ArtifactDescriptor, context: &mut SanitizationContext) {
        let before_len = artifact.content_bytes.len();
        artifact.content_bytes = strip_metadata_markers(&artifact.content_bytes);
        let after_len = artifact.content_bytes.len();

        if artifact.metadata_entries > 0 || before_len != after_len {
            context.metadata_stripped = true;
            artifact.metadata_entries = 0;
            context.notes.push("metadata stripped".to_string());
        }
    }
}

/// Adapter that uses the gk-metadata-sanitizer library for comprehensive metadata removal
pub struct LibraryMetadataSanitizer {
    sanitizer: MetadataSanitizer,
}

impl LibraryMetadataSanitizer {
    pub fn new() -> Self {
        Self {
            sanitizer: MetadataSanitizer::new(),
        }
    }
}

impl Default for LibraryMetadataSanitizer {
    fn default() -> Self {
        Self::new()
    }
}

impl SanitizerAdapter for LibraryMetadataSanitizer {
    fn name(&self) -> &'static str {
        "library-metadata-sanitizer"
    }

    fn process(&self, artifact: &mut ArtifactDescriptor, context: &mut SanitizationContext) {
        let mime_type = context.sniffed_mime.as_str();
        let result = self.sanitizer.sanitize(&artifact.content_bytes, mime_type);
        
        if result.metadata_removed {
            context.metadata_stripped = true;
            context.notes.extend(result.operations);
        }
        
        if !result.warnings.is_empty() {
            context.notes.extend(result.warnings);
        }
        
        artifact.content_bytes = Vec::new(); // In production, would use sanitized output
        artifact.metadata_entries = 0;
    }
}

pub struct DocumentFlattenAdapter;

impl SanitizerAdapter for DocumentFlattenAdapter {
    fn name(&self) -> &'static str {
        "document-flatten"
    }

    fn process(&self, artifact: &mut ArtifactDescriptor, context: &mut SanitizationContext) {
        let before = artifact.content_bytes.clone();
        artifact.content_bytes = flatten_document_payload(&artifact.content_bytes);

        if before != artifact.content_bytes {
            context.notes.push("document flattened".to_string());
        }
    }
}

pub struct RiskScoringAdapter;

impl SanitizerAdapter for RiskScoringAdapter {
    fn name(&self) -> &'static str {
        "risk-scoring"
    }

    fn process(&self, artifact: &mut ArtifactDescriptor, context: &mut SanitizationContext) {
        let mut risk_score = context.risk_score;
        
        // Add baseline risk for the MIME type
        risk_score = risk_score.saturating_add(context.sniffed_mime.risk_baseline());
        
        // Add risk for metadata entries
        risk_score = risk_score.saturating_add((artifact.metadata_entries.min(20) as u8) * 2);

        // Additional risk for metadata stripping detected
        if context.metadata_stripped {
            risk_score = risk_score.saturating_add(10);
        }

        // Risk for MIME mismatch
        if artifact.declared_mime != context.sniffed_mime.as_str() {
            risk_score = risk_score.saturating_add(35);
            context
                .notes
                .push("declared/sniffed MIME mismatch".to_string());
        }

        // Additional risk for Office macros
        if context.sniffed_mime.is_office_document()
            && contains_macro_markers(&artifact.content_bytes) {
            risk_score = risk_score.saturating_add(40);
            context.notes.push("potential macro detected".to_string());
        }

        // Additional risk for nested archives
        if context.sniffed_mime.is_archive()
            && is_nested_archive(&artifact.content_bytes) {
            risk_score = risk_score.saturating_add(20);
            context.notes.push("nested archive detected".to_string());
        }

        context.risk_score = risk_score.min(100);
    }
}

pub struct OfficeMacroStripAdapter;

impl SanitizerAdapter for OfficeMacroStripAdapter {
    fn name(&self) -> &'static str {
        "office-macro-strip"
    }

    fn process(&self, artifact: &mut ArtifactDescriptor, context: &mut SanitizationContext) {
        if !context.sniffed_mime.is_office_document() {
            return;
        }

        let before = artifact.content_bytes.clone();
        artifact.content_bytes = strip_macro_markers(&artifact.content_bytes);

        if artifact.content_bytes != before {
            context.notes.push("Office macro markers stripped".to_string());
            context.metadata_stripped = true;
        }
    }
}

pub struct ArchiveInspectAdapter;

impl SanitizerAdapter for ArchiveInspectAdapter {
    fn name(&self) -> &'static str {
        "archive-inspect"
    }

    fn process(&self, artifact: &mut ArtifactDescriptor, context: &mut SanitizationContext) {
        if !context.sniffed_mime.is_archive() {
            return;
        }

        // Check for nested archives
        if is_nested_archive(&artifact.content_bytes) {
            context.notes.push("nested archive detected".to_string());
        }

        // Check for suspicious file patterns in archive headers
        if contains_suspicious_paths(&artifact.content_bytes) {
            context.notes.push("suspicious path patterns detected".to_string());
            context.risk_score = context.risk_score.saturating_add(25);
        }
    }
}

pub struct XmlSanitizeAdapter;

impl SanitizerAdapter for XmlSanitizeAdapter {
    fn name(&self) -> &'static str {
        "xml-sanitize"
    }

    fn process(&self, artifact: &mut ArtifactDescriptor, context: &mut SanitizationContext) {
        // Office documents (docx, xlsx, pptx) are ZIP containing XML
        if !context.sniffed_mime.is_office_document() {
            return;
        }

        let before = artifact.content_bytes.clone();
        artifact.content_bytes = strip_xml_external_refs(&artifact.content_bytes);

        if artifact.content_bytes != before {
            context.notes.push("XML external references stripped".to_string());
        }
    }
}

pub struct AirlockService<S: SanitizerPipeline> {
    sanitizer: S,
    sessions: HashMap<String, TransferSession>,
    approved_transfers: HashSet<String>,
    next_session_nonce: u64,
}

impl<S: SanitizerPipeline> AirlockService<S> {
    pub fn new(sanitizer: S) -> Self {
        Self {
            sanitizer,
            sessions: HashMap::new(),
            approved_transfers: HashSet::new(),
            next_session_nonce: 1,
        }
    }

    pub fn open_session(
        &mut self,
        source_shard: &str,
        target_shard: &str,
        audit_chain: &mut AuditChain,
    ) -> String {
        let session_id = format!("airlock-{:016x}", self.next_session_nonce);
        self.next_session_nonce = self.next_session_nonce.saturating_add(1);

        self.sessions.insert(
            session_id.clone(),
            TransferSession {
                session_id: session_id.clone(),
                source_shard: source_shard.to_string(),
                target_shard: target_shard.to_string(),
                state: TransferSessionState::Open,
                artifact: None,
                scan_report: None,
            },
        );

        audit_chain.append(
            "airlockd.session.opened",
            format!("{session_id}:{source_shard}->{target_shard}"),
        );
        session_id
    }

    pub fn scan_session(
        &mut self,
        session_id: &str,
        mut artifact: ArtifactDescriptor,
        audit_chain: &mut AuditChain,
    ) -> Result<SanitizerReport, AirlockError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(AirlockError::SessionNotFound)?;

        if session.state != TransferSessionState::Open {
            return Err(AirlockError::InvalidState {
                expected: TransferSessionState::Open,
                actual: session.state,
            });
        }

        let sniffed = sniff_mime(&artifact.content_bytes);
        if matches!(sniffed, SniffedMime::Unknown | SniffedMime::Executable) {
            session.state = TransferSessionState::Rejected;
            audit_chain.append(
                "airlockd.session.rejected",
                format!("{session_id}:unknown-mime:{}", sniffed.as_str()),
            );
            return Err(AirlockError::UnknownMimeRejected {
                sniffed_mime: sniffed.as_str().to_string(),
            });
        }

        let report = self.sanitizer.sanitize(&mut artifact);
        session.artifact = Some(artifact);
        session.scan_report = Some(report.clone());

        if report.risk_score >= HIGH_RISK_THRESHOLD {
            session.state = TransferSessionState::Rejected;
            audit_chain.append("airlockd.session.rejected", session_id.to_string());
            return Err(AirlockError::HighRiskRejected {
                risk_score: report.risk_score,
            });
        }

        session.state = TransferSessionState::Scanned;
        audit_chain.append("airlockd.session.scanned", session_id.to_string());
        Ok(report)
    }

    pub fn approve_session(
        &mut self,
        session_id: &str,
        audit_chain: &mut AuditChain,
    ) -> Result<(), AirlockError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(AirlockError::SessionNotFound)?;

        if session.state != TransferSessionState::Scanned {
            return Err(AirlockError::InvalidState {
                expected: TransferSessionState::Scanned,
                actual: session.state,
            });
        }

        session.state = TransferSessionState::Approved;
        audit_chain.append("airlockd.session.approved", session_id.to_string());
        Ok(())
    }

    pub fn reject_session(
        &mut self,
        session_id: &str,
        reason: &str,
        audit_chain: &mut AuditChain,
    ) -> Result<(), AirlockError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(AirlockError::SessionNotFound)?;

        if matches!(
            session.state,
            TransferSessionState::Committed | TransferSessionState::Rejected
        ) {
            return Err(AirlockError::InvalidState {
                expected: TransferSessionState::Open,
                actual: session.state,
            });
        }

        session.state = TransferSessionState::Rejected;
        audit_chain.append(
            "airlockd.session.rejected",
            format!("{session_id}:{reason}"),
        );
        Ok(())
    }

    pub fn commit_session(
        &mut self,
        session_id: &str,
        audit_chain: &mut AuditChain,
    ) -> Result<(), AirlockError> {
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(AirlockError::SessionNotFound)?;

        if session.state != TransferSessionState::Approved {
            return Err(AirlockError::InvalidState {
                expected: TransferSessionState::Approved,
                actual: session.state,
            });
        }

        let Some(artifact) = session.artifact.as_ref() else {
            return Err(AirlockError::InvalidState {
                expected: TransferSessionState::Scanned,
                actual: session.state,
            });
        };

        let transfer_key = Self::transfer_key(
            &session.source_shard,
            &session.target_shard,
            artifact.artifact_id.as_str(),
        );
        self.approved_transfers.insert(transfer_key);
        session.state = TransferSessionState::Committed;
        audit_chain.append("airlockd.session.committed", session_id.to_string());
        Ok(())
    }

    pub fn request_direct_transfer(
        &self,
        source_shard: &str,
        target_shard: &str,
        artifact_id: &str,
        audit_chain: &mut AuditChain,
    ) -> Result<(), AirlockError> {
        let transfer_key = Self::transfer_key(source_shard, target_shard, artifact_id);
        if self.approved_transfers.contains(&transfer_key) {
            audit_chain.append(
                "airlockd.transfer.allowed",
                format!("{source_shard}->{target_shard}:{artifact_id}"),
            );
            return Ok(());
        }

        audit_chain.append(
            "airlockd.transfer.denied",
            format!("{source_shard}->{target_shard}:{artifact_id}"),
        );
        Err(AirlockError::DirectTransferDenied)
    }

    pub fn save_runtime_state(&self, path: &Path) -> Result<(), AirlockError> {
        save_state(path, &self.runtime_state())
            .map_err(|error| AirlockError::PersistenceFailure(error.to_string()))
    }

    pub fn load_runtime_state(&mut self, path: &Path) -> Result<(), AirlockError> {
        let Some(state) = load_state::<AirlockRuntimeState>(path)
            .map_err(|error| AirlockError::PersistenceFailure(error.to_string()))?
        else {
            return Ok(());
        };

        self.apply_runtime_state(state);
        Ok(())
    }

    pub fn runtime_state(&self) -> AirlockRuntimeState {
        AirlockRuntimeState {
            sessions: self.sessions.clone(),
            approved_transfers: self.approved_transfers.clone(),
            next_session_nonce: self.next_session_nonce,
        }
    }

    pub fn apply_runtime_state(&mut self, state: AirlockRuntimeState) {
        self.sessions = state.sessions;
        self.approved_transfers = state.approved_transfers;
        self.next_session_nonce = state.next_session_nonce;
    }

    pub fn session_state(&self, session_id: &str) -> Option<TransferSessionState> {
        self.sessions.get(session_id).map(|session| session.state)
    }

    fn transfer_key(source_shard: &str, target_shard: &str, artifact_id: &str) -> String {
        format!("{source_shard}->{target_shard}:{artifact_id}")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTransferSessionPayload {
    pub source_shard: String,
    pub target_shard: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenTransferSessionResponse {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanTransferPayload {
    pub session_id: String,
    pub artifact: ArtifactDescriptor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionPayload {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RejectTransferPayload {
    pub session_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectTransferPayload {
    pub source_shard: String,
    pub target_shard: String,
    pub artifact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStateResponse {
    pub state: Option<String>,
}

pub struct AirlockIpcHandler<S: SanitizerPipeline> {
    pub service: AirlockService<S>,
    pub audit_chain: AuditChain,
}

impl<S: SanitizerPipeline> AirlockIpcHandler<S> {
    pub fn new(service: AirlockService<S>) -> Self {
        Self {
            service,
            audit_chain: AuditChain::default(),
        }
    }
}

impl<S: SanitizerPipeline> IpcMethodHandler for AirlockIpcHandler<S> {
    fn handle(
        &mut self,
        auth_context: &AuthContext,
        request: IpcRequest,
    ) -> Result<IpcResponse, IpcError> {
        match request.method.as_str() {
            "OpenTransferSession" => {
                require_role(auth_context, "airlock-submit")?;
                let payload = decode_payload::<OpenTransferSessionPayload>(&request)?;
                let session_id = self.service.open_session(
                    &payload.source_shard,
                    &payload.target_shard,
                    &mut self.audit_chain,
                );
                success_payload(&OpenTransferSessionResponse { session_id })
            }
            "ScanArtifact" => {
                require_role(auth_context, "airlock-scan")?;
                let payload = decode_payload::<ScanTransferPayload>(&request)?;
                let report = self
                    .service
                    .scan_session(&payload.session_id, payload.artifact, &mut self.audit_chain)
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&report)
            }
            "ApproveTransfer" => {
                require_role(auth_context, "airlock-approve")?;
                let payload = decode_payload::<SessionPayload>(&request)?;
                self.service
                    .approve_session(&payload.session_id, &mut self.audit_chain)
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&serde_json::json!({"approved": true}))
            }
            "RejectTransfer" => {
                require_role(auth_context, "airlock-approve")?;
                let payload = decode_payload::<RejectTransferPayload>(&request)?;
                self.service
                    .reject_session(&payload.session_id, &payload.reason, &mut self.audit_chain)
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&serde_json::json!({"rejected": true}))
            }
            "CommitTransfer" => {
                require_role(auth_context, "airlock-approve")?;
                let payload = decode_payload::<SessionPayload>(&request)?;
                self.service
                    .commit_session(&payload.session_id, &mut self.audit_chain)
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&serde_json::json!({"committed": true}))
            }
            "RequestDirectTransfer" => {
                require_role(auth_context, "airlock-submit")?;
                let payload = decode_payload::<DirectTransferPayload>(&request)?;
                self.service
                    .request_direct_transfer(
                        &payload.source_shard,
                        &payload.target_shard,
                        &payload.artifact_id,
                        &mut self.audit_chain,
                    )
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&serde_json::json!({"allowed": true}))
            }
            "GetSessionState" => {
                require_role(auth_context, "airlock-read")?;
                let payload = decode_payload::<SessionPayload>(&request)?;
                let state = self
                    .service
                    .session_state(&payload.session_id)
                    .map(|state| match state {
                        TransferSessionState::Open => "Open",
                        TransferSessionState::Scanned => "Scanned",
                        TransferSessionState::Approved => "Approved",
                        TransferSessionState::Rejected => "Rejected",
                        TransferSessionState::Committed => "Committed",
                    })
                    .map(ToString::to_string);
                success_payload(&SessionStateResponse { state })
            }
            _ => Err(IpcError::UnknownMethod(request.method)),
        }
    }
}

fn strip_metadata_markers(bytes: &[u8]) -> Vec<u8> {
    if bytes.is_empty() {
        return Vec::new();
    }

    // Handle JPEG EXIF removal properly
    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return strip_jpeg_exif(bytes);
    }

    // Handle PNG metadata removal
    if bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47]) {
        return strip_png_chunks(bytes);
    }

    // Generic EXIF/text metadata removal for other formats
    let marker = b"EXIF";
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if index + marker.len() <= bytes.len() && &bytes[index..index + marker.len()] == marker {
            // Skip EXIF marker and associated data
            index += marker.len();
            // Skip length prefix if present (2 bytes)
            if index + 2 <= bytes.len() {
                let exif_len = u16::from_be_bytes([bytes[index], bytes[index + 1]]) as usize;
                index += 2 + exif_len;
            }
            continue;
        }

        output.push(bytes[index]);
        index += 1;
    }

    output
}

/// Strip EXIF APP1 segment from JPEG
fn strip_jpeg_exif(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    // Copy SOI marker
    if bytes.len() >= 2 && bytes[0] == 0xff && bytes[1] == 0xd8 {
        output.extend_from_slice(&bytes[0..2]);
        index = 2;
    }

    // Process segments
    while index + 1 < bytes.len() {
        if bytes[index] != 0xff {
            // Not a marker, copy rest
            output.extend_from_slice(&bytes[index..]);
            break;
        }

        let marker = bytes[index + 1];
        
        // Handle standalone markers (no length field): SOI, EOI, etc.
        if marker == 0xd8 || marker == 0xd9 || marker == 0x01 {
            output.extend_from_slice(&bytes[index..index + 2]);
            index += 2;
            continue;
        }

        // Skip EXIF APP1 (0xe1) and other metadata APPn segments (0xe2-0xef)
        if marker == 0xe1 || (0xe2..=0xef).contains(&marker) {
            // Skip this segment
            if index + 4 <= bytes.len() {
                let seg_len = u16::from_be_bytes([bytes[index + 2], bytes[index + 3]]) as usize;
                index += 2 + seg_len;
                continue;
            }
        }

        // Copy this segment
        if index + 4 <= bytes.len() {
            let seg_len = u16::from_be_bytes([bytes[index + 2], bytes[index + 3]]) as usize;
            let seg_end = (index + 2 + seg_len).min(bytes.len());
            output.extend_from_slice(&bytes[index..seg_end]);
            index = seg_end;
        } else {
            output.extend_from_slice(&bytes[index..]);
            break;
        }
    }

    output
}

/// Strip ancillary chunks from PNG (keeping IHDR, IDAT, IEND)
fn strip_png_chunks(bytes: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    // Copy PNG signature
    if bytes.len() >= 8 && bytes.starts_with(&[0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a]) {
        output.extend_from_slice(&bytes[0..8]);
        index = 8;
    }

    // Process chunks
    while index + 8 <= bytes.len() {
        let length = u32::from_be_bytes([bytes[index], bytes[index + 1], bytes[index + 2], bytes[index + 3]]) as usize;
        let chunk_type = &bytes[index + 4..index + 8];
        
        // Keep critical chunks: IHDR, IDAT, IEND, PLTE
        let keep_chunk = chunk_type == b"IHDR" || chunk_type == b"IDAT" 
            || chunk_type == b"IEND" || chunk_type == b"PLTE";

        let chunk_end = index + 12 + length; // 4 len + 4 type + data + 4 crc
        if chunk_end > bytes.len() {
            break;
        }

        if keep_chunk {
            output.extend_from_slice(&bytes[index..chunk_end]);
        } else {
            // Skip metadata chunks (tEXt, zTXt, iTXt, eXIf, etc.)
        }

        index = chunk_end;
    }

    // Copy any remaining data
    if index < bytes.len() {
        output.extend_from_slice(&bytes[index..]);
    }

    output
}

fn contains_macro_markers(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    let markers: &[&[u8]] = &[b"VBA", b"Macro", b"macro", b"Module1", b"ThisDocument"];
    markers.iter().any(|marker| {
        bytes.windows(marker.len()).any(|window| window == *marker)
    })
}

fn is_nested_archive(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    // Check for multiple archive signatures
    let archive_markers = [
        b"PK".as_slice(),  // ZIP
        b"\x1f\x8b".as_slice(),  // GZIP
        b"ustar".as_slice(),  // TAR
    ];

    let mut found_count = 0;
    for marker in &archive_markers {
        if bytes.windows(marker.len()).any(|window| window == *marker) {
            found_count += 1;
        }
    }

    // Multiple archive signatures indicate nesting
    found_count > 1 || bytes.windows(2).filter(|w| w == b"PK").count() > 1
}

fn strip_macro_markers(bytes: &[u8]) -> Vec<u8> {
    if bytes.is_empty() {
        return Vec::new();
    }

    let mut output = bytes.to_vec();
    for marker in [b"VBA".as_slice(), b"Macro".as_slice(), b"macro".as_slice()] {
        output = strip_pattern(&output, marker);
    }
    output
}

fn contains_suspicious_paths(bytes: &[u8]) -> bool {
    if bytes.is_empty() {
        return false;
    }

    let suspicious_patterns: &[&[u8]] = &[
        b"/etc/passwd",
        b"/etc/shadow",
        b"/root/",
        b"../",
        b"..\\",
        b"\\Windows\\System32",
        b"cmd.exe",
        b"powershell",
    ];

    suspicious_patterns.iter().any(|pattern: &&[u8]| {
        bytes.windows(pattern.len()).any(|window| window == *pattern)
    })
}

fn strip_xml_external_refs(bytes: &[u8]) -> Vec<u8> {
    if bytes.is_empty() {
        return Vec::new();
    }

    let mut output = bytes.to_vec();
    for pattern in [
        b"<!ENTITY".as_slice(),
        b"SYSTEM".as_slice(),
        b"PUBLIC".as_slice(),
        b"file://".as_slice(),
        b"http://".as_slice(),
        b"https://".as_slice(),
    ] {
        output = strip_pattern(&output, pattern);
    }
    output
}

fn flatten_document_payload(bytes: &[u8]) -> Vec<u8> {
    if bytes.is_empty() {
        return Vec::new();
    }

    let mut output = bytes.to_vec();
    for marker in [
        b"/JavaScript".as_slice(),
        b"/JS".as_slice(),
        b"/OpenAction".as_slice(),
    ] {
        output = strip_pattern(&output, marker);
    }

    output
}

fn strip_pattern(bytes: &[u8], pattern: &[u8]) -> Vec<u8> {
    if pattern.is_empty() || bytes.is_empty() {
        return bytes.to_vec();
    }

    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if index + pattern.len() <= bytes.len() && &bytes[index..index + pattern.len()] == pattern {
            index += pattern.len();
            continue;
        }

        output.push(bytes[index]);
        index += 1;
    }

    output
}

fn sniff_mime(bytes: &[u8]) -> SniffedMime {
    if bytes.starts_with(b"%PDF-") {
        return SniffedMime::Pdf;
    }

    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return SniffedMime::Png;
    }

    if bytes.starts_with(&[0xff, 0xd8, 0xff]) {
        return SniffedMime::Jpeg;
    }

    if bytes.starts_with(b"\x7fELF") {
        return SniffedMime::Executable;
    }

    if bytes.starts_with(b"PK\x03\x04") {
        return SniffedMime::ZipArchive;
    }

    if bytes.is_empty() {
        return SniffedMime::Unknown;
    }

    let printable = bytes
        .iter()
        .filter(|byte| matches!(byte, b'\n' | b'\r' | b'\t' | 32..=126))
        .count();
    if printable * 100 / bytes.len() >= 90 {
        return SniffedMime::PlainText;
    }

    SniffedMime::Unknown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pdf_artifact(artifact_id: &str) -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_id: artifact_id.to_string(),
            path: format!("/tmp/{artifact_id}.pdf"),
            metadata_entries: 2,
            declared_mime: "application/pdf".to_string(),
            content_bytes: b"%PDF-1.7\nEXIFpayload".to_vec(),
        }
    }

    fn unknown_artifact() -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_id: "artifact-unknown".to_string(),
            path: "/tmp/blob.bin".to_string(),
            metadata_entries: 1,
            declared_mime: "application/octet-stream".to_string(),
            content_bytes: vec![0x00, 0x01, 0x02, 0x03, 0x04],
        }
    }

    fn high_risk_artifact() -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_id: "artifact-elf".to_string(),
            path: "/tmp/dropper".to_string(),
            metadata_entries: 5,
            declared_mime: "application/pdf".to_string(),
            content_bytes: b"\x7fELFbinary".to_vec(),
        }
    }

    fn mismatch_high_risk_artifact() -> ArtifactDescriptor {
        ArtifactDescriptor {
            artifact_id: "artifact-risk".to_string(),
            path: "/tmp/risk.zip".to_string(),
            metadata_entries: 20,
            // Declare as PDF but content is a ZIP archive (mismatch + archive baseline)
            declared_mime: "application/pdf".to_string(),
            // ZIP header that will be recognized
            content_bytes: b"PK\x03\x04\x14\x00\x00\x00\x08\x00test.zip content data here".to_vec(),
        }
    }

    #[test]
    fn bypass_is_denied_without_approval() {
        let service = AirlockService::new(PluggableSanitizerChain::default_chain());
        let mut audit_chain = AuditChain::default();

        let result =
            service.request_direct_transfer("work", "anon", "artifact-1", &mut audit_chain);
        assert!(matches!(result, Err(AirlockError::DirectTransferDenied)));
    }

    #[test]
    fn unknown_mime_is_rejected_by_default() {
        let mut service = AirlockService::new(PluggableSanitizerChain::default_chain());
        let mut audit_chain = AuditChain::default();
        let session_id = service.open_session("work", "anon", &mut audit_chain);

        let scan_result = service.scan_session(&session_id, unknown_artifact(), &mut audit_chain);
        assert!(matches!(
            scan_result,
            Err(AirlockError::UnknownMimeRejected { .. })
        ));
        assert_eq!(
            service.session_state(&session_id),
            Some(TransferSessionState::Rejected)
        );
    }

    #[test]
    fn high_risk_artifacts_are_rejected() {
        let mut service = AirlockService::new(PluggableSanitizerChain::default_chain());
        let mut audit_chain = AuditChain::default();
        let session_id = service.open_session("work", "anon", &mut audit_chain);

        let scan_result = service.scan_session(&session_id, high_risk_artifact(), &mut audit_chain);
        assert!(matches!(
            scan_result,
            Err(AirlockError::UnknownMimeRejected { .. })
        ));
    }

    #[test]
    fn high_risk_score_artifacts_are_rejected() {
        let mut service = AirlockService::new(PluggableSanitizerChain::default_chain());
        let mut audit_chain = AuditChain::default();
        let session_id = service.open_session("work", "anon", &mut audit_chain);

        let scan_result =
            service.scan_session(&session_id, mismatch_high_risk_artifact(), &mut audit_chain);
        eprintln!("scan_result: {:?}", scan_result);
        assert!(matches!(
            scan_result,
            Err(AirlockError::HighRiskRejected { .. })
        ));
    }

    #[test]
    fn approved_commit_allows_transfer() {
        let mut service = AirlockService::new(PluggableSanitizerChain::default_chain());
        let mut audit_chain = AuditChain::default();
        let session_id = service.open_session("work", "anon", &mut audit_chain);

        let report = service
            .scan_session(&session_id, pdf_artifact("artifact-1"), &mut audit_chain)
            .expect("pdf artifact should pass scan");
        assert_eq!(report.sniffed_mime, "application/pdf");
        assert!(report.metadata_stripped);
        assert!(report
            .applied_steps
            .contains(&"document-flatten".to_string()));

        assert!(service
            .approve_session(&session_id, &mut audit_chain)
            .is_ok());
        assert!(service
            .commit_session(&session_id, &mut audit_chain)
            .is_ok());

        let transfer_result =
            service.request_direct_transfer("work", "anon", "artifact-1", &mut audit_chain);
        assert!(transfer_result.is_ok());
    }

    #[test]
    fn ipc_handler_enforces_authz_boundary() {
        let service = AirlockService::new(PluggableSanitizerChain::default_chain());
        let mut handler = AirlockIpcHandler::new(service);

        let request = IpcRequest {
            method: "ApproveTransfer".to_string(),
            payload: serde_json::to_string(&SessionPayload {
                session_id: "airlock-1".to_string(),
            })
            .expect("payload should encode"),
        };
        let auth_context = AuthContext {
            caller_id: "app://client".to_string(),
            roles: vec!["airlock-read".to_string()],
        };

        let response = handler.handle(&auth_context, request);
        assert!(matches!(response, Err(IpcError::Unauthorized { .. })));
    }

    #[test]
    fn ipc_handler_routes_approved_transfer_flow() {
        let service = AirlockService::new(PluggableSanitizerChain::default_chain());
        let mut handler = AirlockIpcHandler::new(service);
        let auth_context = AuthContext {
            caller_id: "daemon://workflow".to_string(),
            roles: vec![
                "airlock-submit".to_string(),
                "airlock-scan".to_string(),
                "airlock-approve".to_string(),
                "airlock-read".to_string(),
            ],
        };

        let open_request = IpcRequest {
            method: "OpenTransferSession".to_string(),
            payload: serde_json::to_string(&OpenTransferSessionPayload {
                source_shard: "work".to_string(),
                target_shard: "anon".to_string(),
            })
            .expect("payload should encode"),
        };
        let open_response = handler
            .handle(&auth_context, open_request)
            .expect("open should succeed");
        let open_result: OpenTransferSessionResponse =
            serde_json::from_str(&open_response.payload).expect("response should decode");

        let scan_request = IpcRequest {
            method: "ScanArtifact".to_string(),
            payload: serde_json::to_string(&ScanTransferPayload {
                session_id: open_result.session_id.clone(),
                artifact: pdf_artifact("artifact-ipc"),
            })
            .expect("payload should encode"),
        };
        let scan_response = handler
            .handle(&auth_context, scan_request)
            .expect("scan should succeed");
        let scan_report: SanitizerReport =
            serde_json::from_str(&scan_response.payload).expect("report should decode");
        assert!(scan_report.risk_score < HIGH_RISK_THRESHOLD);

        let approve_request = IpcRequest {
            method: "ApproveTransfer".to_string(),
            payload: serde_json::to_string(&SessionPayload {
                session_id: open_result.session_id.clone(),
            })
            .expect("payload should encode"),
        };
        assert!(handler.handle(&auth_context, approve_request).is_ok());

        let commit_request = IpcRequest {
            method: "CommitTransfer".to_string(),
            payload: serde_json::to_string(&SessionPayload {
                session_id: open_result.session_id.clone(),
            })
            .expect("payload should encode"),
        };
        assert!(handler.handle(&auth_context, commit_request).is_ok());

        let direct_transfer_request = IpcRequest {
            method: "RequestDirectTransfer".to_string(),
            payload: serde_json::to_string(&DirectTransferPayload {
                source_shard: "work".to_string(),
                target_shard: "anon".to_string(),
                artifact_id: "artifact-ipc".to_string(),
            })
            .expect("payload should encode"),
        };
        let transfer_response = handler
            .handle(&auth_context, direct_transfer_request)
            .expect("approved direct transfer should succeed");
        assert!(transfer_response.ok);
    }

    #[test]
    fn runtime_state_recovers_after_crash_artifact() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let path = temp.path().join("airlock-state.json");
        let mut service = AirlockService::new(PluggableSanitizerChain::default_chain());
        let mut audit_chain = AuditChain::default();

        let session_id = service.open_session("work", "anon", &mut audit_chain);
        let _ = service.scan_session(&session_id, pdf_artifact("artifact-2"), &mut audit_chain);
        let _ = service.approve_session(&session_id, &mut audit_chain);
        let _ = service.commit_session(&session_id, &mut audit_chain);
        service
            .save_runtime_state(&path)
            .expect("runtime state should save");

        let crash_tmp = path.with_extension("json.tmp");
        std::fs::rename(&path, &crash_tmp).expect("state should move to tmp for crash simulation");

        let mut recovered = AirlockService::new(PluggableSanitizerChain::default_chain());
        recovered
            .load_runtime_state(&path)
            .expect("state should recover from tmp");

        let transfer_result =
            recovered.request_direct_transfer("work", "anon", "artifact-2", &mut audit_chain);
        assert!(transfer_result.is_ok());
    }

    #[test]
    fn strip_jpeg_exif_removes_app1_segment() {
        // JPEG with EXIF APP1 segment (0xff 0xe1)
        // Segment length includes the 2 length bytes themselves
        let jpeg_with_exif = vec![
            0xff, 0xd8, // SOI
            0xff, 0xe1, 0x00, 0x08, // APP1 marker, length 8 (includes length bytes)
            b'E', b'X', b'I', b'F', 0x00, 0x00, // EXIF data
            0xff, 0xdb, 0x00, 0x04, // DQT segment (should keep)
            0x00, 0x00,
            0xff, 0xd9, // EOI
        ];

        let stripped = strip_jpeg_exif(&jpeg_with_exif);
        
        // Should not contain APP1 segment
        assert!(!stripped.windows(2).any(|w| w == [0xff, 0xe1]));
        // Should still have SOI and EOI
        assert!(stripped.starts_with(&[0xff, 0xd8]));
        assert!(stripped.ends_with(&[0xff, 0xd9]));
        // Should be shorter than original (EXIF was removed)
        assert!(stripped.len() < jpeg_with_exif.len());
    }

    #[test]
    fn strip_png_chunks_removes_metadata() {
        // PNG with tEXt metadata chunk
        let png_with_text = vec![
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
            // IHDR chunk (keep)
            0x00, 0x00, 0x00, 0x0d, // length 13
            b'I', b'H', b'D', b'R',
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x02, 0x00, 0x00, 0x00,
            0x90, 0x77, 0x53, 0xde, // CRC
            // tEXt chunk (remove)
            0x00, 0x00, 0x00, 0x04, // length 4
            b't', b'E', b'X', b't',
            0x00, 0x00, 0x00, 0x00, // data
            0x00, 0x00, 0x00, 0x00, // CRC
            // IEND chunk (keep)
            0x00, 0x00, 0x00, 0x00,
            b'I', b'E', b'N', b'D',
            0xae, 0x42, 0x60, 0x82,
        ];

        let stripped = strip_png_chunks(&png_with_text);
        
        // Should not contain tEXt chunk type
        assert!(!stripped.windows(4).any(|w| w == b"tEXt"));
        // Should still have IHDR and IEND
        assert!(stripped.windows(4).any(|w| w == b"IHDR"));
        assert!(stripped.windows(4).any(|w| w == b"IEND"));
    }

    #[test]
    fn strip_metadata_markers_handles_generic_exif() {
        let data_with_exif = b"headerEXIF\x00\x04data";
        let stripped = strip_metadata_markers(data_with_exif);
        
        // EXIF marker should be removed
        assert!(!stripped.windows(4).any(|w| w == b"EXIF"));
    }

    #[test]
    fn sanitizer_pipeline_strips_image_metadata() {
        let mut chain = PluggableSanitizerChain::default_chain();
        
        // JPEG with EXIF
        let mut artifact = ArtifactDescriptor {
            artifact_id: "test-jpeg".to_string(),
            path: "/tmp/test.jpg".to_string(),
            metadata_entries: 5,
            declared_mime: "image/jpeg".to_string(),
            content_bytes: vec![
                0xff, 0xd8, 0xff, 0xe1, 0x00, 0x04, b'E', b'X', b'I', b'F',
                0xff, 0xd9,
            ],
        };

        let report = chain.sanitize(&mut artifact);
        
        assert!(report.metadata_stripped);
        // EXIF segment should be removed
        assert!(!artifact.content_bytes.windows(2).any(|w| w == [0xff, 0xe1]));
    }
}
