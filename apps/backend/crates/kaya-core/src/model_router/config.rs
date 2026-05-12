//! Routing-table configuration loaded from `kaya.yaml`.
//!
//! # Schema
//!
//! ```yaml
//! routing:
//!   <operation>:           # snake_case OperationType variant
//!     provider: <name>     # key in the `providers` map
//!     model: <model-id>    # provider-specific model identifier
//!
//! providers:
//!   <name>:
//!     api_key_env: <VAR>   # environment variable holding the API key
//! ```
//!
//! Every [`OperationType`] variant **must** appear in `routing`.
//! Every `provider` value referenced in `routing` **must** appear in
//! `providers`. Validation runs at load time; an invalid config returns a
//! descriptive [`ConfigError`].

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use thiserror::Error;

use super::OperationType;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file: {0}")]
    Io(#[from] std::io::Error),
    #[error("failed to parse YAML: {0}")]
    Parse(#[from] serde_yaml::Error),
    #[error("unknown provider '{0}' referenced in routing table")]
    UnknownProvider(String),
    #[error("missing routing entry for operation {0:?}")]
    MissingRoute(OperationType),
    #[error("env var '{0}' not set (required for provider '{1}')")]
    MissingApiKey(String, String),
}

/// Top-level deserialization target for `kaya.yaml`.
#[derive(Debug, Clone, Deserialize)]
pub struct RoutingConfig {
    pub routing: HashMap<OperationType, RouteEntry>,
    pub providers: HashMap<String, ProviderConfig>,
}

/// Maps a single operation to a provider + model.
#[derive(Debug, Clone, Deserialize)]
pub struct RouteEntry {
    pub provider: String,
    pub model: String,
}

/// Per-provider configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderConfig {
    /// Name of the environment variable holding the API key.
    pub api_key_env: String,
}

impl RoutingConfig {
    /// Load and validate from a YAML file on disk.
    pub fn from_yaml_file(path: &Path) -> Result<Self, ConfigError> {
        let content = std::fs::read_to_string(path)?;
        Self::from_yaml_str(&content)
    }

    /// Parse and validate from a YAML string (useful in tests).
    pub fn from_yaml_str(s: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_yaml::from_str(s)?;
        config.validate()?;
        Ok(config)
    }

    /// Resolve the API key for `provider_name` by reading its env var.
    pub fn resolve_api_key(&self, provider_name: &str) -> Result<String, ConfigError> {
        let cfg = self
            .providers
            .get(provider_name)
            .ok_or_else(|| ConfigError::UnknownProvider(provider_name.to_owned()))?;
        std::env::var(&cfg.api_key_env).map_err(|_| {
            ConfigError::MissingApiKey(cfg.api_key_env.clone(), provider_name.to_owned())
        })
    }

    fn validate(&self) -> Result<(), ConfigError> {
        // Every operation must have a route.
        let required = [
            OperationType::RetrievalClassification,
            OperationType::DocumentGeneration,
            OperationType::EditProposal,
            OperationType::StaleDetection,
            OperationType::Embedding,
        ];
        for op in &required {
            if !self.routing.contains_key(op) {
                return Err(ConfigError::MissingRoute(op.clone()));
            }
        }
        // Every provider referenced in routing must be declared.
        for entry in self.routing.values() {
            if !self.providers.contains_key(&entry.provider) {
                return Err(ConfigError::UnknownProvider(entry.provider.clone()));
            }
        }
        Ok(())
    }
}
