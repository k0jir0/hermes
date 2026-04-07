use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum GpuClass {
    B200,
    H200,
    H100,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConfidentialCompute {
    IntelTdx,
    AmdSev,
    Disabled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ComputeProfile {
    pub gpu_class: GpuClass,
    pub gpu_count: u16,
    pub dedicated_bare_metal: bool,
    pub confidential_compute: ConfidentialCompute,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NetworkProfile {
    pub bandwidth_gbps: u16,
    pub static_ips: bool,
    pub strict_port_mapping: bool,
    pub public_endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LocalStorageProfile {
    pub local_nvme: bool,
    pub nvme_raid_level: String,
    pub scratch_mount: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrchestrationProfile {
    pub kubernetes_distribution: String,
    pub telemetry_stack: Vec<String>,
    pub node_selector: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClusterConfig {
    pub compute: ComputeProfile,
    pub network: NetworkProfile,
    pub storage: LocalStorageProfile,
    pub orchestration: OrchestrationProfile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GatewayConfig {
    pub listen_addr: String,
    pub chutes_base_url: String,
    pub allowed_subnets: Vec<u16>,
    pub request_timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ConsensusSettings {
    pub kappa: f64,
    pub bonds_penalty: f64,
    pub ema_alpha: f64,
    pub validator_emission_ratio: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum VectorBackend {
    Surrealdb,
    CloudflareVectorize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StorageConfig {
    pub vector_backend: VectorBackend,
    pub redis_url: String,
    pub postgres_url: String,
    pub vector_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HotkeySource {
    KubernetesSecret { secret_name: String, key: String },
    HashicorpVault { path: String, field: String },
    File { path: String },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SecuritySettings {
    pub coldkey_policy: String,
    pub hotkey_source: HotkeySource,
    pub disallow_inline_hotkeys: bool,
    pub required_attestation: ConfidentialCompute,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HermesConfig {
    pub cluster: ClusterConfig,
    pub gateway: GatewayConfig,
    pub consensus: ConsensusSettings,
    pub storage: StorageConfig,
    pub security: SecuritySettings,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to parse config: {0}")]
    Parse(#[from] serde_json::Error),
    #[error("configuration validation failed: {0:?}")]
    Invalid(Vec<String>),
}

impl HermesConfig {
    pub fn from_json_str(raw: &str) -> Result<Self, ConfigError> {
        serde_json::from_str(raw).map_err(ConfigError::from)
    }

    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut issues = Vec::new();

        if !self.cluster.compute.dedicated_bare_metal {
            issues.push("cluster.compute.dedicated_bare_metal must be true".to_string());
        }

        if self.cluster.compute.gpu_count == 0 {
            issues.push("cluster.compute.gpu_count must be greater than zero".to_string());
        }

        if self.cluster.network.bandwidth_gbps < 10 {
            issues.push("cluster.network.bandwidth_gbps must be at least 10".to_string());
        }

        if !self.cluster.network.static_ips {
            issues.push("cluster.network.static_ips must be true".to_string());
        }

        if !self.cluster.network.strict_port_mapping {
            issues.push("cluster.network.strict_port_mapping must be true".to_string());
        }

        if !self.cluster.storage.local_nvme {
            issues.push("cluster.storage.local_nvme must be true".to_string());
        }

        if self.cluster.storage.scratch_mount.trim().is_empty() {
            issues.push("cluster.storage.scratch_mount must not be empty".to_string());
        }

        if !self
            .cluster
            .orchestration
            .telemetry_stack
            .iter()
            .any(|item| item.eq_ignore_ascii_case("prometheus"))
        {
            issues.push("cluster.orchestration.telemetry_stack must include prometheus".to_string());
        }

        if !self
            .cluster
            .orchestration
            .telemetry_stack
            .iter()
            .any(|item| item.eq_ignore_ascii_case("grafana"))
        {
            issues.push("cluster.orchestration.telemetry_stack must include grafana".to_string());
        }

        if self.gateway.allowed_subnets.is_empty() {
            issues.push("gateway.allowed_subnets must not be empty".to_string());
        }

        if self.gateway.request_timeout_ms == 0 {
            issues.push("gateway.request_timeout_ms must be greater than zero".to_string());
        }

        if !(0.5..=1.0).contains(&self.consensus.kappa) {
            issues.push("consensus.kappa must be between 0.5 and 1.0".to_string());
        }

        if !(0.0..=1.0).contains(&self.consensus.bonds_penalty) {
            issues.push("consensus.bonds_penalty must be between 0.0 and 1.0".to_string());
        }

        if !(0.0..=1.0).contains(&self.consensus.ema_alpha) {
            issues.push("consensus.ema_alpha must be between 0.0 and 1.0".to_string());
        }

        if !(0.0..=1.0).contains(&self.consensus.validator_emission_ratio) {
            issues.push(
                "consensus.validator_emission_ratio must be between 0.0 and 1.0".to_string(),
            );
        }

        if self.storage.redis_url.trim().is_empty() {
            issues.push("storage.redis_url must not be empty".to_string());
        }

        if self.storage.postgres_url.trim().is_empty() {
            issues.push("storage.postgres_url must not be empty".to_string());
        }

        if self.storage.vector_url.trim().is_empty() {
            issues.push("storage.vector_url must not be empty".to_string());
        }

        if !self.security.disallow_inline_hotkeys {
            issues.push("security.disallow_inline_hotkeys must be true".to_string());
        }

        match &self.security.hotkey_source {
            HotkeySource::KubernetesSecret { secret_name, key } => {
                if secret_name.trim().is_empty() || key.trim().is_empty() {
                    issues.push(
                        "security.hotkey_source KubernetesSecret requires secret_name and key"
                            .to_string(),
                    );
                }
            }
            HotkeySource::HashicorpVault { path, field } => {
                if path.trim().is_empty() || field.trim().is_empty() {
                    issues.push(
                        "security.hotkey_source HashicorpVault requires path and field"
                            .to_string(),
                    );
                }
            }
            HotkeySource::File { path } => {
                if path.trim().is_empty() {
                    issues.push("security.hotkey_source File requires path".to_string());
                }
            }
        }

        if self.security.required_attestation == ConfidentialCompute::Disabled {
            issues.push("security.required_attestation must not be disabled".to_string());
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::Invalid(issues))
        }
    }
}
