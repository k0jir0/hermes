use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::config::{ConfidentialCompute, HotkeySource, SecuritySettings};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SecurityPosture {
    pub coldkey_policy: String,
    pub hotkey_delivery: String,
    pub required_attestation: String,
    pub controls: Vec<String>,
}

#[derive(Debug, Error)]
pub enum SecurityError {
    #[error("security posture is invalid: {0}")]
    Invalid(String),
}

pub fn audit_security(settings: &SecuritySettings) -> Result<SecurityPosture, SecurityError> {
    if !settings.disallow_inline_hotkeys {
        return Err(SecurityError::Invalid(
            "inline hotkeys are not allowed".to_string(),
        ));
    }

    let hotkey_delivery = match &settings.hotkey_source {
        HotkeySource::KubernetesSecret { secret_name, key } => {
            if secret_name.trim().is_empty() || key.trim().is_empty() {
                return Err(SecurityError::Invalid(
                    "kubernetes secret hotkey delivery requires secret_name and key".to_string(),
                ));
            }
            format!("kubernetes_secret:{secret_name}:{key}")
        }
        HotkeySource::HashicorpVault { path, field } => {
            if path.trim().is_empty() || field.trim().is_empty() {
                return Err(SecurityError::Invalid(
                    "hashicorp vault hotkey delivery requires path and field".to_string(),
                ));
            }
            format!("hashicorp_vault:{path}:{field}")
        }
        HotkeySource::File { path } => {
            if path.trim().is_empty() {
                return Err(SecurityError::Invalid(
                    "file hotkey delivery requires a non-empty path".to_string(),
                ));
            }
            format!("file:{path}")
        }
    };

    if settings.required_attestation == ConfidentialCompute::Disabled {
        return Err(SecurityError::Invalid(
            "confidential compute must remain enabled for production Hermes deployments"
                .to_string(),
        ));
    }

    let required_attestation = match settings.required_attestation {
        ConfidentialCompute::IntelTdx => "intel_tdx",
        ConfidentialCompute::AmdSev => "amd_sev",
        ConfidentialCompute::Disabled => "disabled",
    }
    .to_string();

    Ok(SecurityPosture {
        coldkey_policy: settings.coldkey_policy.clone(),
        hotkey_delivery,
        required_attestation,
        controls: vec![
            "Keep coldkeys offline on hardware-backed storage.".to_string(),
            "Inject hotkeys at runtime through Vault or Kubernetes secrets.".to_string(),
            "Schedule validator workloads only on confidential-compute-capable nodes.".to_string(),
        ],
    })
}
