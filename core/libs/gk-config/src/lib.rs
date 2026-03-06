use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use schemars::{schema_for, JsonSchema};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SpecterosConfig {
    pub schema_version: u32,
    pub core: CoreConfig,
    pub security: SecurityConfig,
    pub policy: PolicyConfig,
    pub shards: ShardConfig,
    pub network: NetworkConfig,
    pub airlock: AirlockConfig,
    pub audit: AuditConfig,
    pub update: UpdateConfig,
    pub edition: EditionConfig,
}

impl Default for SpecterosConfig {
    fn default() -> Self {
        Self {
            schema_version: 1,
            core: CoreConfig::default(),
            security: SecurityConfig::default(),
            policy: PolicyConfig::default(),
            shards: ShardConfig::default(),
            network: NetworkConfig::default(),
            airlock: AirlockConfig::default(),
            audit: AuditConfig::default(),
            update: UpdateConfig::default(),
            edition: EditionConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct CoreConfig {
    pub data_dir: String,
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            data_dir: "/var/lib/specteros".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct SecurityConfig {
    pub deny_by_default: bool,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            deny_by_default: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct PolicyConfig {
    pub token_ttl_seconds: u64,
}

impl Default for PolicyConfig {
    fn default() -> Self {
        Self {
            token_ttl_seconds: 900,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct ShardConfig {
    pub enabled: bool,
}

impl Default for ShardConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct NetworkConfig {
    pub kill_switch_default: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            kill_switch_default: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AirlockConfig {
    pub require_scan: bool,
}

impl Default for AirlockConfig {
    fn default() -> Self {
        Self { require_scan: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct AuditConfig {
    pub enabled: bool,
}

impl Default for AuditConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct UpdateConfig {
    pub channel: String,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            channel: "stable".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct EditionConfig {
    pub name: String,
    pub debian: Option<DebianEditionConfig>,
    pub fedora: Option<FedoraEditionConfig>,
}

impl Default for EditionConfig {
    fn default() -> Self {
        Self {
            name: "shared".to_string(),
            debian: None,
            fedora: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct DebianEditionConfig {
    pub apt_channel: String,
}

impl Default for DebianEditionConfig {
    fn default() -> Self {
        Self {
            apt_channel: "stable".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
#[serde(default)]
pub struct FedoraEditionConfig {
    pub dnf_channel: String,
}

impl Default for FedoraEditionConfig {
    fn default() -> Self {
        Self {
            dnf_channel: "stable".to_string(),
        }
    }
}

pub fn default_layer_paths() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/usr/lib/specteros/default.toml"),
        PathBuf::from("/usr/lib/specteros/edition.toml"),
        PathBuf::from("/etc/specteros/config.toml"),
    ];

    paths.extend(config_dir_layers("/etc/specteros/config.d"));
    paths.push(PathBuf::from("/run/specteros/override.toml"));

    paths
}

pub fn load_layered(paths: &[PathBuf]) -> Result<SpecterosConfig> {
    let mut merged = toml::Value::Table(toml::map::Map::new());

    for path in paths {
        if !path.exists() {
            continue;
        }

        merge_file(path, &mut merged)?;
    }

    if merged
        .as_table()
        .map(|table| table.is_empty())
        .unwrap_or(true)
    {
        return Ok(SpecterosConfig::default());
    }

    let cfg = merged
        .try_into()
        .context("failed to deserialize merged Specteros config")?;

    Ok(cfg)
}

pub fn export_schema_value() -> Result<serde_json::Value> {
    let schema = schema_for!(SpecterosConfig);
    let value = serde_json::to_value(schema)?;
    Ok(value)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePaths {
    pub config_dir: PathBuf,
    pub data_dir: PathBuf,
    pub log_dir: PathBuf,
}

impl RuntimePaths {
    pub fn system_defaults() -> Self {
        Self {
            config_dir: PathBuf::from("/etc/specteros"),
            data_dir: PathBuf::from("/var/lib/specteros"),
            log_dir: PathBuf::from("/var/log/specteros"),
        }
    }

    pub fn from_root(root: &Path) -> Self {
        Self {
            config_dir: root.join("etc/specteros"),
            data_dir: root.join("var/lib/specteros"),
            log_dir: root.join("var/log/specteros"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeBootstrapReport {
    pub created_paths: Vec<PathBuf>,
    pub validated_paths: Vec<PathBuf>,
}

pub fn ensure_runtime_layout(paths: &RuntimePaths) -> Result<RuntimeBootstrapReport> {
    let mut created_paths = Vec::new();
    let mut validated_paths = Vec::new();

    ensure_dir_with_permissions(&paths.config_dir, 0o750, &mut created_paths)?;
    ensure_dir_with_permissions(&paths.data_dir, 0o750, &mut created_paths)?;
    ensure_dir_with_permissions(&paths.log_dir, 0o750, &mut created_paths)?;

    validate_dir_writable(&paths.config_dir)?;
    validated_paths.push(paths.config_dir.clone());

    validate_dir_writable(&paths.data_dir)?;
    validated_paths.push(paths.data_dir.clone());

    validate_dir_writable(&paths.log_dir)?;
    validated_paths.push(paths.log_dir.clone());

    Ok(RuntimeBootstrapReport {
        created_paths,
        validated_paths,
    })
}

pub fn validate_runtime_layout(paths: &RuntimePaths) -> Result<()> {
    for path in [&paths.config_dir, &paths.data_dir, &paths.log_dir] {
        if !path.exists() {
            anyhow::bail!("required runtime path is missing: {}", path.display());
        }

        if !path.is_dir() {
            anyhow::bail!(
                "required runtime path is not a directory: {}",
                path.display()
            );
        }

        validate_dir_writable(path)?;
    }

    Ok(())
}

fn merge_file(path: &Path, merged: &mut toml::Value) -> Result<()> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("failed reading config layer {}", path.display()))?;
    let layer: toml::Value =
        toml::from_str(&content).with_context(|| format!("invalid TOML in {}", path.display()))?;
    deep_merge(merged, layer);
    Ok(())
}

fn config_dir_layers(dir: &str) -> Vec<PathBuf> {
    let mut entries = match fs::read_dir(dir) {
        Ok(items) => items
            .filter_map(|entry| entry.ok().map(|item| item.path()))
            .filter(|path| {
                path.extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext == "toml")
                    .unwrap_or(false)
            })
            .collect::<Vec<_>>(),
        Err(_) => return Vec::new(),
    };

    entries.sort();
    entries
}

fn deep_merge(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(base_table), toml::Value::Table(overlay_table)) => {
            for (key, overlay_value) in overlay_table {
                match base_table.get_mut(&key) {
                    Some(base_value) => deep_merge(base_value, overlay_value),
                    None => {
                        base_table.insert(key, overlay_value);
                    }
                }
            }
        }
        (base_value, overlay_value) => {
            *base_value = overlay_value;
        }
    }
}

fn ensure_dir_with_permissions(
    path: &Path,
    mode: u32,
    created_paths: &mut Vec<PathBuf>,
) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)
            .with_context(|| format!("failed to create runtime directory {}", path.display()))?;
        created_paths.push(path.to_path_buf());
    }

    #[cfg(unix)]
    {
        fs::set_permissions(path, fs::Permissions::from_mode(mode)).with_context(|| {
            format!(
                "failed to set runtime directory permissions for {}",
                path.display()
            )
        })?;
    }

    Ok(())
}

fn validate_dir_writable(path: &Path) -> Result<()> {
    let validation_file = path.join(".startup-check");
    fs::write(&validation_file, b"ok")
        .with_context(|| format!("failed writing startup check file in {}", path.display()))?;
    fs::remove_file(&validation_file)
        .with_context(|| format!("failed removing startup check file in {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    #[test]
    fn layered_loader_applies_overrides() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let base = temp.path().join("base.toml");
        let override_file = temp.path().join("override.toml");

        fs::write(
            &base,
            r#"
schema_version = 1
[core]
data_dir = "/var/lib/specteros"
[security]
deny_by_default = true
"#,
        )
        .expect("base layer should be written");

        fs::write(
            &override_file,
            r#"
[core]
data_dir = "/tmp/specteros"
"#,
        )
        .expect("override layer should be written");

        let loaded = load_layered(&[base, override_file]).expect("config should load");
        assert_eq!(loaded.core.data_dir, "/tmp/specteros");
        assert!(loaded.security.deny_by_default);
    }

    #[test]
    fn schema_export_contains_properties() {
        let schema = export_schema_value().expect("schema should export");
        assert!(schema.get("definitions").is_some() || schema.get("$defs").is_some());
    }

    #[test]
    fn default_layer_paths_include_runtime_override() {
        let paths = default_layer_paths();
        assert!(paths
            .iter()
            .any(|path| path == &PathBuf::from("/run/specteros/override.toml")));
    }

    #[test]
    fn runtime_layout_is_created_and_validated() {
        let temp = tempfile::tempdir().expect("tempdir should be created");
        let paths = RuntimePaths::from_root(temp.path());

        let report = ensure_runtime_layout(&paths).expect("layout creation should succeed");
        assert!(report.validated_paths.contains(&paths.config_dir));
        assert!(report.validated_paths.contains(&paths.data_dir));
        assert!(report.validated_paths.contains(&paths.log_dir));

        validate_runtime_layout(&paths).expect("layout validation should succeed");
    }
}
