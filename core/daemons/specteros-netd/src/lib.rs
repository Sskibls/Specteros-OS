use std::collections::HashMap;
use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::Path;
use std::process::Command;

use gk_audit::AuditChain;
use gk_ipc::{
    decode_payload, require_role, success_payload, AuthContext, IpcError, IpcMethodHandler,
    IpcRequest, IpcResponse,
};
use gk_persistence::{load_state, save_state, PersistedState};
use gk_privexec::{Capability, PrivilegedExecutor};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum RouteProfile {
    Offline,
    Direct,
    Tor,
    Vpn,
}

impl RouteProfile {
    pub fn as_str(&self) -> &'static str {
        match self {
            RouteProfile::Offline => "Offline",
            RouteProfile::Direct => "Direct",
            RouteProfile::Tor => "Tor",
            RouteProfile::Vpn => "Vpn",
        }
    }
}

impl std::str::FromStr for RouteProfile {
    type Err = NetworkPolicyError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "Offline" => Ok(RouteProfile::Offline),
            "Direct" => Ok(RouteProfile::Direct),
            "Tor" => Ok(RouteProfile::Tor),
            "Vpn" => Ok(RouteProfile::Vpn),
            other => Err(NetworkPolicyError::BackendFailure(format!(
                "unsupported profile: {other}"
            ))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeakCheckReport {
    pub clean: bool,
    pub risk_score: u8,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NetworkPolicyError {
    KillSwitchEnabled,
    NoProfile,
    OfflineProfileBlocked,
    EgressBlocked,
    BackendFailure(String),
    PersistenceFailure(String),
}

impl Display for NetworkPolicyError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            NetworkPolicyError::KillSwitchEnabled => write!(formatter, "kill-switch is active"),
            NetworkPolicyError::NoProfile => write!(formatter, "no route profile assigned"),
            NetworkPolicyError::OfflineProfileBlocked => {
                write!(formatter, "offline profile blocks network operations")
            }
            NetworkPolicyError::EgressBlocked => {
                write!(formatter, "backend reports egress is blocked")
            }
            NetworkPolicyError::BackendFailure(message) => {
                write!(formatter, "backend failure: {message}")
            }
            NetworkPolicyError::PersistenceFailure(message) => {
                write!(formatter, "persistence failure: {message}")
            }
        }
    }
}

impl Error for NetworkPolicyError {}

pub trait LeakChecker {
    fn run_check(&self, shard_name: &str, profile: RouteProfile) -> LeakCheckReport;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct DeterministicLeakChecker;

impl LeakChecker for DeterministicLeakChecker {
    fn run_check(&self, shard_name: &str, profile: RouteProfile) -> LeakCheckReport {
        match profile {
            RouteProfile::Offline => LeakCheckReport {
                clean: true,
                risk_score: 0,
                summary: format!("{shard_name}:offline-no-egress"),
            },
            RouteProfile::Direct => LeakCheckReport {
                clean: false,
                risk_score: 85,
                summary: format!("{shard_name}:direct-egress-detected"),
            },
            RouteProfile::Tor => LeakCheckReport {
                clean: true,
                risk_score: 15,
                summary: format!("{shard_name}:tor-route-isolated"),
            },
            RouteProfile::Vpn => LeakCheckReport {
                clean: true,
                risk_score: 20,
                summary: format!("{shard_name}:vpn-route-isolated"),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NetworkBackendMode {
    Staged,
    Enforcing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkBackendError {
    pub message: String,
}

pub trait CommandExecutor {
    fn run(&self, program: &str, args: &[String]) -> Result<String, NetworkBackendError>;
}

#[derive(Debug, Default, Clone, Copy)]
pub struct ProcessCommandExecutor;

impl CommandExecutor for ProcessCommandExecutor {
    fn run(&self, program: &str, args: &[String]) -> Result<String, NetworkBackendError> {
        let output =
            Command::new(program)
                .args(args)
                .output()
                .map_err(|error| NetworkBackendError {
                    message: format!("failed to execute {program}: {error}"),
                })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(NetworkBackendError {
                message: format!("command {program} failed: {stderr}"),
            });
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

pub struct PrivilegedCommandExecutor {
    executor: PrivilegedExecutor,
}

impl PrivilegedCommandExecutor {
    pub fn new(enforcing: bool) -> Self {
        Self {
            executor: PrivilegedExecutor::new(enforcing),
        }
    }

    pub fn with_capabilities(capabilities: Vec<Capability>, enforcing: bool) -> Self {
        Self {
            executor: PrivilegedExecutor::with_capabilities(capabilities, enforcing),
        }
    }

    pub fn executor(&self) -> &PrivilegedExecutor {
        &self.executor
    }

    pub fn executor_mut(&mut self) -> &mut PrivilegedExecutor {
        &mut self.executor
    }
}

impl CommandExecutor for PrivilegedCommandExecutor {
    fn run(&self, program: &str, args: &[String]) -> Result<String, NetworkBackendError> {
        // Note: This is a read-only implementation for staged mode
        // For actual execution, use the mutable variant via executor_mut
        let mut exec = PrivilegedExecutor::with_capabilities(vec![Capability::NetAdmin], false);

        exec.run(program, args).map_err(|error| NetworkBackendError {
            message: error.to_string(),
        })
    }
}

pub trait NetworkBackend {
    fn apply_profile(
        &mut self,
        shard_name: &str,
        profile: RouteProfile,
    ) -> Result<(), NetworkBackendError>;
    fn set_kill_switch(&mut self, enabled: bool) -> Result<(), NetworkBackendError>;
    fn can_egress(
        &self,
        shard_name: &str,
        profile: RouteProfile,
        kill_switch_enabled: bool,
    ) -> bool;
    fn sync_state(
        &mut self,
        route_profiles: &HashMap<String, RouteProfile>,
        kill_switch_enabled: bool,
    ) -> Result<(), NetworkBackendError>;
}

pub struct NftablesRouteBackend<E: CommandExecutor> {
    mode: NetworkBackendMode,
    executor: E,
    profiles: HashMap<String, RouteProfile>,
    kill_switch_enabled: bool,
    operations: Vec<String>,
}

impl NftablesRouteBackend<ProcessCommandExecutor> {
    pub fn new_staged() -> Self {
        Self::with_executor(NetworkBackendMode::Staged, ProcessCommandExecutor)
    }

    pub fn new_enforcing() -> Self {
        Self::with_executor(NetworkBackendMode::Enforcing, ProcessCommandExecutor)
    }
}

impl<E: CommandExecutor> NftablesRouteBackend<E> {
    pub fn with_executor(mode: NetworkBackendMode, executor: E) -> Self {
        Self {
            mode,
            executor,
            profiles: HashMap::new(),
            kill_switch_enabled: false,
            operations: Vec::new(),
        }
    }

    pub fn operations(&self) -> &[String] {
        &self.operations
    }

    fn execute_command(&mut self, command: &[String]) -> Result<(), NetworkBackendError> {
        if command.is_empty() {
            return Ok(());
        }

        self.operations.push(command.join(" "));
        if self.mode == NetworkBackendMode::Staged {
            return Ok(());
        }

        let (program, args) = command.split_first().ok_or(NetworkBackendError {
            message: "empty command".to_string(),
        })?;

        let _ = self.executor.run(program, args)?;
        Ok(())
    }

    fn profile_commands(profile: RouteProfile) -> Vec<Vec<String>> {
        match profile {
            RouteProfile::Offline => vec![vec![
                "nft".to_string(),
                "add".to_string(),
                "rule".to_string(),
                "inet".to_string(),
                "phantomkernel".to_string(),
                "output".to_string(),
                "drop".to_string(),
            ]],
            RouteProfile::Direct => vec![vec![
                "ip".to_string(),
                "route".to_string(),
                "replace".to_string(),
                "default".to_string(),
                "dev".to_string(),
                "eth0".to_string(),
            ]],
            RouteProfile::Tor => vec![vec![
                "ip".to_string(),
                "route".to_string(),
                "replace".to_string(),
                "default".to_string(),
                "via".to_string(),
                "127.0.0.1".to_string(),
            ]],
            RouteProfile::Vpn => vec![vec![
                "ip".to_string(),
                "route".to_string(),
                "replace".to_string(),
                "default".to_string(),
                "dev".to_string(),
                "tun0".to_string(),
            ]],
        }
    }

    fn kill_switch_commands(enabled: bool) -> Vec<Vec<String>> {
        if enabled {
            vec![vec![
                "nft".to_string(),
                "add".to_string(),
                "rule".to_string(),
                "inet".to_string(),
                "phantomkernel".to_string(),
                "output".to_string(),
                "drop".to_string(),
            ]]
        } else {
            vec![vec![
                "nft".to_string(),
                "flush".to_string(),
                "chain".to_string(),
                "inet".to_string(),
                "phantomkernel".to_string(),
                "output".to_string(),
            ]]
        }
    }
}

impl<E: CommandExecutor> NetworkBackend for NftablesRouteBackend<E> {
    fn apply_profile(
        &mut self,
        shard_name: &str,
        profile: RouteProfile,
    ) -> Result<(), NetworkBackendError> {
        for command in Self::profile_commands(profile) {
            self.execute_command(&command)?;
        }

        self.profiles.insert(shard_name.to_string(), profile);
        Ok(())
    }

    fn set_kill_switch(&mut self, enabled: bool) -> Result<(), NetworkBackendError> {
        for command in Self::kill_switch_commands(enabled) {
            self.execute_command(&command)?;
        }

        self.kill_switch_enabled = enabled;
        Ok(())
    }

    fn can_egress(
        &self,
        shard_name: &str,
        profile: RouteProfile,
        kill_switch_enabled: bool,
    ) -> bool {
        if kill_switch_enabled || self.kill_switch_enabled {
            return false;
        }

        match self.profiles.get(shard_name).copied() {
            Some(RouteProfile::Offline) | None => false,
            Some(configured) => configured == profile && profile != RouteProfile::Offline,
        }
    }

    fn sync_state(
        &mut self,
        route_profiles: &HashMap<String, RouteProfile>,
        kill_switch_enabled: bool,
    ) -> Result<(), NetworkBackendError> {
        self.profiles.clear();
        for (shard_name, profile) in route_profiles {
            self.apply_profile(shard_name, *profile)?;
        }

        self.set_kill_switch(kill_switch_enabled)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkRuntimeState {
    pub kill_switch_enabled: bool,
    pub route_profiles: HashMap<String, RouteProfile>,
}

impl PersistedState for NetworkRuntimeState {
    const STATE_KIND: &'static str = "phantomkernel-netd-runtime";
    const CURRENT_SCHEMA_VERSION: u32 = 1;
}

pub struct NetworkPolicyService<L: LeakChecker, B: NetworkBackend> {
    kill_switch_enabled: bool,
    route_profiles: HashMap<String, RouteProfile>,
    leak_checker: L,
    backend: B,
}

impl<L: LeakChecker, B: NetworkBackend> NetworkPolicyService<L, B> {
    pub fn new(leak_checker: L, backend: B) -> Self {
        Self {
            kill_switch_enabled: false,
            route_profiles: HashMap::new(),
            leak_checker,
            backend,
        }
    }

    pub fn apply_profile(
        &mut self,
        shard_name: &str,
        profile: RouteProfile,
        audit_chain: &mut AuditChain,
    ) -> Result<(), NetworkPolicyError> {
        self.backend
            .apply_profile(shard_name, profile)
            .map_err(|error| {
                self.kill_switch_enabled = true;
                let _ = self.backend.set_kill_switch(true);
                audit_chain.append("netd.backend.failure", error.message.clone());
                NetworkPolicyError::BackendFailure(error.message)
            })?;

        self.route_profiles.insert(shard_name.to_string(), profile);
        audit_chain.append("netd.profile.applied", format!("{shard_name}:{profile:?}"));
        Ok(())
    }

    pub fn set_kill_switch(
        &mut self,
        enabled: bool,
        audit_chain: &mut AuditChain,
    ) -> Result<(), NetworkPolicyError> {
        self.backend.set_kill_switch(enabled).map_err(|error| {
            self.kill_switch_enabled = true;
            audit_chain.append("netd.backend.failure", error.message.clone());
            NetworkPolicyError::BackendFailure(error.message)
        })?;

        self.kill_switch_enabled = enabled;
        audit_chain.append("netd.kill-switch", format!("enabled={enabled}"));
        Ok(())
    }

    pub fn can_route(
        &self,
        shard_name: &str,
        audit_chain: &mut AuditChain,
    ) -> Result<RouteProfile, NetworkPolicyError> {
        if self.kill_switch_enabled {
            audit_chain.append("netd.route.denied", "kill-switch-enabled");
            return Err(NetworkPolicyError::KillSwitchEnabled);
        }

        let profile = self
            .route_profiles
            .get(shard_name)
            .copied()
            .ok_or(NetworkPolicyError::NoProfile)?;

        if profile == RouteProfile::Offline {
            audit_chain.append("netd.route.denied", "offline-profile");
            return Err(NetworkPolicyError::OfflineProfileBlocked);
        }

        if !self
            .backend
            .can_egress(shard_name, profile, self.kill_switch_enabled)
        {
            audit_chain.append("netd.route.denied", "backend-egress-blocked");
            return Err(NetworkPolicyError::EgressBlocked);
        }

        audit_chain.append("netd.route.allowed", format!("{shard_name}:{profile:?}"));
        Ok(profile)
    }

    pub fn run_leak_check(
        &self,
        shard_name: &str,
        audit_chain: &mut AuditChain,
    ) -> LeakCheckReport {
        let report = if self.kill_switch_enabled {
            LeakCheckReport {
                clean: true,
                risk_score: 0,
                summary: format!("{shard_name}:kill-switch-active"),
            }
        } else if let Some(profile) = self.route_profiles.get(shard_name).copied() {
            self.leak_checker.run_check(shard_name, profile)
        } else {
            LeakCheckReport {
                clean: false,
                risk_score: 100,
                summary: format!("{shard_name}:fail-closed-no-profile"),
            }
        };

        audit_chain.append(
            "netd.leak-check",
            format!(
                "{shard_name}:clean={} score={}",
                report.clean, report.risk_score
            ),
        );
        report
    }

    pub fn save_runtime_state(&self, path: &Path) -> Result<(), NetworkPolicyError> {
        save_state(path, &self.runtime_state())
            .map_err(|error| NetworkPolicyError::PersistenceFailure(error.to_string()))
    }

    pub fn load_runtime_state(
        &mut self,
        path: &Path,
        audit_chain: &mut AuditChain,
    ) -> Result<(), NetworkPolicyError> {
        let Some(state) = load_state::<NetworkRuntimeState>(path)
            .map_err(|error| NetworkPolicyError::PersistenceFailure(error.to_string()))?
        else {
            return Ok(());
        };

        self.apply_runtime_state(state, audit_chain)
    }

    pub fn runtime_state(&self) -> NetworkRuntimeState {
        NetworkRuntimeState {
            kill_switch_enabled: self.kill_switch_enabled,
            route_profiles: self.route_profiles.clone(),
        }
    }

    pub fn apply_runtime_state(
        &mut self,
        state: NetworkRuntimeState,
        audit_chain: &mut AuditChain,
    ) -> Result<(), NetworkPolicyError> {
        let route_profiles = state.route_profiles;
        let kill_switch_enabled = state.kill_switch_enabled;

        self.backend
            .sync_state(&route_profiles, kill_switch_enabled)
            .map_err(|error| {
                self.kill_switch_enabled = true;
                let _ = self.backend.set_kill_switch(true);
                audit_chain.append("netd.backend.failure", error.message.clone());
                NetworkPolicyError::BackendFailure(error.message)
            })?;

        self.route_profiles = route_profiles;
        self.kill_switch_enabled = kill_switch_enabled;
        audit_chain.append(
            "netd.state.recovered",
            format!(
                "profiles={} kill_switch={}",
                self.route_profiles.len(),
                self.kill_switch_enabled
            ),
        );
        Ok(())
    }

    pub fn profile_of(&self, shard_name: &str) -> Option<RouteProfile> {
        self.route_profiles.get(shard_name).copied()
    }

    pub fn kill_switch_enabled(&self) -> bool {
        self.kill_switch_enabled
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyRouteProfilePayload {
    pub shard_name: String,
    pub profile: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetKillSwitchPayload {
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeakCheckPayload {
    pub shard_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteStatePayload {
    pub shard_name: String,
}

pub struct NetworkIpcHandler<L: LeakChecker, B: NetworkBackend> {
    pub service: NetworkPolicyService<L, B>,
    pub audit_chain: AuditChain,
}

impl<L: LeakChecker, B: NetworkBackend> NetworkIpcHandler<L, B> {
    pub fn new(service: NetworkPolicyService<L, B>) -> Self {
        Self {
            service,
            audit_chain: AuditChain::default(),
        }
    }
}

impl<L: LeakChecker, B: NetworkBackend> IpcMethodHandler for NetworkIpcHandler<L, B> {
    fn handle(
        &mut self,
        auth_context: &AuthContext,
        request: IpcRequest,
    ) -> Result<IpcResponse, IpcError> {
        match request.method.as_str() {
            "ApplyRouteProfile" => {
                require_role(auth_context, "network-admin")?;
                let payload = decode_payload::<ApplyRouteProfilePayload>(&request)?;
                let profile = payload.profile.parse::<RouteProfile>().map_err(|error| {
                    IpcError::InvalidPayload(format!("failed to parse route profile: {error}"))
                })?;
                self.service
                    .apply_profile(&payload.shard_name, profile, &mut self.audit_chain)
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&serde_json::json!({"applied": true}))
            }
            "SetKillSwitch" => {
                require_role(auth_context, "network-admin")?;
                let payload = decode_payload::<SetKillSwitchPayload>(&request)?;
                self.service
                    .set_kill_switch(payload.enabled, &mut self.audit_chain)
                    .map_err(|error| IpcError::Internal(error.to_string()))?;
                success_payload(&serde_json::json!({"enabled": payload.enabled}))
            }
            "RunLeakCheck" => {
                require_role(auth_context, "network-read")?;
                let payload = decode_payload::<LeakCheckPayload>(&request)?;
                let report = self
                    .service
                    .run_leak_check(&payload.shard_name, &mut self.audit_chain);
                success_payload(&report)
            }
            "GetRouteState" => {
                require_role(auth_context, "network-read")?;
                let payload = decode_payload::<RouteStatePayload>(&request)?;
                let route_state = self.service.profile_of(&payload.shard_name);
                success_payload(&serde_json::json!({
                    "profile": route_state.map(|profile| profile.as_str().to_string()),
                    "kill_switch_enabled": self.service.kill_switch_enabled(),
                }))
            }
            _ => Err(IpcError::UnknownMethod(request.method)),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    struct MockExecutor {
        should_fail: bool,
        calls: RefCell<Vec<String>>,
    }

    impl MockExecutor {
        fn successful() -> Self {
            Self {
                should_fail: false,
                calls: RefCell::new(Vec::new()),
            }
        }

        fn failing() -> Self {
            Self {
                should_fail: true,
                calls: RefCell::new(Vec::new()),
            }
        }
    }

    impl CommandExecutor for MockExecutor {
        fn run(&self, program: &str, args: &[String]) -> Result<String, NetworkBackendError> {
            self.calls
                .borrow_mut()
                .push(format!("{program} {}", args.join(" ")));

            if self.should_fail {
                return Err(NetworkBackendError {
                    message: "mock execution failure".to_string(),
                });
            }

            Ok("ok".to_string())
        }
    }

    #[test]
    fn profile_apply_is_persisted() {
        let backend = NftablesRouteBackend::with_executor(
            NetworkBackendMode::Staged,
            MockExecutor::successful(),
        );
        let mut service = NetworkPolicyService::new(DeterministicLeakChecker, backend);
        let mut audit_chain = AuditChain::default();
        assert!(service
            .apply_profile("work", RouteProfile::Tor, &mut audit_chain)
            .is_ok());

        assert_eq!(service.profile_of("work"), Some(RouteProfile::Tor));
    }

    #[test]
    fn kill_switch_has_priority() {
        let backend = NftablesRouteBackend::with_executor(
            NetworkBackendMode::Staged,
            MockExecutor::successful(),
        );
        let mut service = NetworkPolicyService::new(DeterministicLeakChecker, backend);
        let mut audit_chain = AuditChain::default();

        assert!(service
            .apply_profile("work", RouteProfile::Direct, &mut audit_chain)
            .is_ok());
        assert!(service.set_kill_switch(true, &mut audit_chain).is_ok());

        let route_result = service.can_route("work", &mut audit_chain);
        assert!(matches!(
            route_result,
            Err(NetworkPolicyError::KillSwitchEnabled)
        ));
    }

    #[test]
    fn fail_closed_without_profile() {
        let backend = NftablesRouteBackend::with_executor(
            NetworkBackendMode::Staged,
            MockExecutor::successful(),
        );
        let service = NetworkPolicyService::new(DeterministicLeakChecker, backend);
        let mut audit_chain = AuditChain::default();

        let route_result = service.can_route("anon", &mut audit_chain);
        assert!(matches!(route_result, Err(NetworkPolicyError::NoProfile)));
    }

    #[test]
    fn deterministic_leak_reports_are_stable() {
        let backend = NftablesRouteBackend::with_executor(
            NetworkBackendMode::Staged,
            MockExecutor::successful(),
        );
        let mut service = NetworkPolicyService::new(DeterministicLeakChecker, backend);
        let mut audit_chain = AuditChain::default();

        assert!(service
            .apply_profile("anon", RouteProfile::Direct, &mut audit_chain)
            .is_ok());
        let report_one = service.run_leak_check("anon", &mut audit_chain);
        let report_two = service.run_leak_check("anon", &mut audit_chain);

        assert_eq!(report_one, report_two);
        assert!(!report_one.clean);
    }

    #[test]
    fn backend_failure_forces_fail_closed_kill_switch() {
        let backend = NftablesRouteBackend::with_executor(
            NetworkBackendMode::Enforcing,
            MockExecutor::failing(),
        );
        let mut service = NetworkPolicyService::new(DeterministicLeakChecker, backend);
        let mut audit_chain = AuditChain::default();

        let outcome = service.apply_profile("work", RouteProfile::Tor, &mut audit_chain);
        assert!(matches!(
            outcome,
            Err(NetworkPolicyError::BackendFailure(_))
        ));
        assert!(service.kill_switch_enabled());

        let route_result = service.can_route("work", &mut audit_chain);
        assert!(matches!(
            route_result,
            Err(NetworkPolicyError::KillSwitchEnabled)
        ));
    }

    #[test]
    fn no_egress_when_kill_switch_is_active() {
        let backend = NftablesRouteBackend::with_executor(
            NetworkBackendMode::Staged,
            MockExecutor::successful(),
        );
        let mut service = NetworkPolicyService::new(DeterministicLeakChecker, backend);
        let mut audit_chain = AuditChain::default();

        assert!(service
            .apply_profile("work", RouteProfile::Vpn, &mut audit_chain)
            .is_ok());
        assert!(service.set_kill_switch(true, &mut audit_chain).is_ok());

        let route_result = service.can_route("work", &mut audit_chain);
        assert!(matches!(
            route_result,
            Err(NetworkPolicyError::KillSwitchEnabled)
        ));
        let report = service.run_leak_check("work", &mut audit_chain);
        assert!(report.clean);
    }

    #[test]
    fn runtime_state_recovers_after_crash_artifact() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let path = temp.path().join("netd-state.json");
        let backend = NftablesRouteBackend::with_executor(
            NetworkBackendMode::Staged,
            MockExecutor::successful(),
        );
        let mut service = NetworkPolicyService::new(DeterministicLeakChecker, backend);
        let mut audit_chain = AuditChain::default();

        assert!(service
            .apply_profile("work", RouteProfile::Tor, &mut audit_chain)
            .is_ok());
        assert!(service.set_kill_switch(true, &mut audit_chain).is_ok());
        service
            .save_runtime_state(&path)
            .expect("state should be persisted");

        let crash_tmp = path.with_extension("json.tmp");
        std::fs::rename(&path, &crash_tmp).expect("state should move to tmp for crash simulation");

        let backend = NftablesRouteBackend::with_executor(
            NetworkBackendMode::Staged,
            MockExecutor::successful(),
        );
        let mut recovered = NetworkPolicyService::new(DeterministicLeakChecker, backend);
        recovered
            .load_runtime_state(&path, &mut audit_chain)
            .expect("state should recover from tmp file");

        assert_eq!(recovered.profile_of("work"), Some(RouteProfile::Tor));
        assert!(recovered.kill_switch_enabled());
        assert!(matches!(
            recovered.can_route("work", &mut audit_chain),
            Err(NetworkPolicyError::KillSwitchEnabled)
        ));
    }

    #[test]
    fn ipc_handler_enforces_authz_boundary() {
        let backend = NftablesRouteBackend::with_executor(
            NetworkBackendMode::Staged,
            MockExecutor::successful(),
        );
        let service = NetworkPolicyService::new(DeterministicLeakChecker, backend);
        let mut handler = NetworkIpcHandler::new(service);

        let request = IpcRequest {
            method: "SetKillSwitch".to_string(),
            payload: serde_json::to_string(&SetKillSwitchPayload { enabled: true })
                .expect("payload should encode"),
        };
        let auth_context = AuthContext {
            caller_id: "app://observer".to_string(),
            roles: vec!["network-read".to_string()],
        };

        let response = handler.handle(&auth_context, request);
        assert!(matches!(response, Err(IpcError::Unauthorized { .. })));
    }
}
