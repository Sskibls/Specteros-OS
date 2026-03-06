use anyhow::Result;

pub fn run_smoke_policy_scenario() -> Result<bool> {
    let request = gk_policy::CapabilityRequest {
        subject: "app://harness".to_string(),
        resource: "network".to_string(),
        duration_seconds: 60,
        grant: false,
    };

    let decision = gk_policy::evaluate_request(&request, 100);
    Ok(matches!(decision, gk_policy::PolicyDecision::Deny(_)))
}

pub fn run_smoke_config_scenario() -> Result<gk_config::SpecterosConfig> {
    gk_config::load_layered(&[])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoke_policy_is_deny_by_default() {
        let denied = run_smoke_policy_scenario().expect("scenario should execute");
        assert!(denied);
    }

    #[test]
    fn smoke_config_loads_defaults() {
        let cfg = run_smoke_config_scenario().expect("config should load");
        assert_eq!(cfg.schema_version, 1);
    }
}
