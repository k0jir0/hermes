use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{ConfigError, ConfidentialCompute, GpuClass, HermesConfig};
use crate::security::{audit_security, SecurityError, SecurityPosture};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HermesDeploymentPlan {
    pub gateway_replicas: u16,
    pub validator_replicas: u16,
    pub node_selector: BTreeMap<String, String>,
    pub telemetry_targets: Vec<String>,
    pub required_services: Vec<String>,
    pub security: SecurityPosture,
    pub notes: Vec<String>,
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    Security(#[from] SecurityError),
}

pub struct DeploymentPlanner;

impl DeploymentPlanner {
    pub fn build(config: &HermesConfig) -> Result<HermesDeploymentPlan, RuntimeError> {
        config.validate()?;
        let security = audit_security(&config.security)?;

        let gateway_replicas = match config.cluster.compute.gpu_class {
            GpuClass::B200 | GpuClass::H200 => 3,
            GpuClass::H100 => 2,
        };

        let validator_replicas = config.cluster.compute.gpu_count.max(2);
        let mut node_selector = config.cluster.orchestration.node_selector.clone();
        node_selector
            .entry("hermes.ai/workload".to_string())
            .or_insert_with(|| "validator".to_string());

        if config.cluster.compute.confidential_compute != ConfidentialCompute::Disabled {
            node_selector
                .entry("hermes.ai/confidential".to_string())
                .or_insert_with(|| "required".to_string());
        }

        let mut notes = vec![
            format!(
                "Provision {} validator replicas across bare-metal nodes with {} GPUs each.",
                validator_replicas, config.cluster.compute.gpu_count
            ),
            format!(
                "Expose {} through dedicated static IPs with strict 1:1 port mapping.",
                config.cluster.network.public_endpoint
            ),
            format!(
                "Use {} for vector retrieval and keep Redis plus PostgreSQL on the same low-latency fabric.",
                match config.storage.vector_backend {
                    crate::config::VectorBackend::Surrealdb => "SurrealDB",
                    crate::config::VectorBackend::CloudflareVectorize => "Cloudflare Vectorize",
                }
            ),
        ];

        if config.cluster.network.bandwidth_gbps >= 25 {
            notes.push("Current network profile is suitable for cross-region validator peering.".to_string());
        }

        Ok(HermesDeploymentPlan {
            gateway_replicas,
            validator_replicas,
            node_selector,
            telemetry_targets: config.cluster.orchestration.telemetry_stack.clone(),
            required_services: vec![
                "redis".to_string(),
                "postgres".to_string(),
                "surrealdb".to_string(),
                "prometheus".to_string(),
                "grafana".to_string(),
            ],
            security,
            notes,
        })
    }
}
