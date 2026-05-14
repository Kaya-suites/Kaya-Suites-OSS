// Copyright 2024 Kaya Suites. All rights reserved. — BSL 1.1
//!
//! Token cost calculation from `config/pricing.yaml`.
//!
//! Unknown models fall back to the Opus rate so spend is never under-counted.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use crate::error::MeteringError;

#[derive(Debug, Clone, Deserialize)]
pub struct ModelPricing {
    pub input_per_million: f64,
    pub output_per_million: f64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PricingConfig {
    pub models: HashMap<String, ModelPricing>,
}

const FALLBACK_INPUT_PER_MILLION: f64 = 15.0;
const FALLBACK_OUTPUT_PER_MILLION: f64 = 75.0;

impl PricingConfig {
    pub fn from_yaml_str(s: &str) -> Result<Self, MeteringError> {
        serde_yaml::from_str(s)
            .map_err(|e| MeteringError::Config(format!("pricing YAML parse error: {e}")))
    }

    pub fn from_yaml_file(path: &Path) -> Result<Self, MeteringError> {
        let s = std::fs::read_to_string(path)
            .map_err(|e| MeteringError::Config(format!("could not read {}: {e}", path.display())))?;
        Self::from_yaml_str(&s)
    }

    /// Compute the USD cost for a single LLM call.
    ///
    /// Falls back to Opus rates for unknown models to avoid under-counting.
    pub fn compute_cost(&self, model: &str, input_tokens: u32, output_tokens: u32) -> f64 {
        let (ipm, opm) = self
            .models
            .get(model)
            .map(|p| (p.input_per_million, p.output_per_million))
            .unwrap_or_else(|| {
                tracing::warn!(model = %model, "unknown model in pricing config — using Opus fallback");
                (FALLBACK_INPUT_PER_MILLION, FALLBACK_OUTPUT_PER_MILLION)
            });
        (input_tokens as f64 / 1_000_000.0) * ipm
            + (output_tokens as f64 / 1_000_000.0) * opm
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const YAML: &str = r#"
models:
  claude-opus-4-6:
    input_per_million: 15.00
    output_per_million: 75.00
  gpt-4o-mini:
    input_per_million: 0.15
    output_per_million: 0.60
  text-embedding-3-small:
    input_per_million: 0.02
    output_per_million: 0.00
"#;

    #[test]
    fn opus_cost() {
        let cfg = PricingConfig::from_yaml_str(YAML).unwrap();
        let cost = cfg.compute_cost("claude-opus-4-6", 2_000, 800);
        // 0.002 * 15 + 0.0008 * 75 = 0.030 + 0.060 = 0.090
        assert!((cost - 0.090).abs() < 1e-9);
    }

    #[test]
    fn embedding_is_input_only() {
        let cfg = PricingConfig::from_yaml_str(YAML).unwrap();
        let cost = cfg.compute_cost("text-embedding-3-small", 1_000, 0);
        assert!((cost - 0.00002).abs() < 1e-10);
    }

    #[test]
    fn unknown_model_falls_back_to_opus() {
        let cfg = PricingConfig::from_yaml_str(YAML).unwrap();
        let cost_unknown = cfg.compute_cost("gpt-99-turbo-ultra", 1_000, 0);
        let cost_opus = cfg.compute_cost("claude-opus-4-6", 1_000, 0);
        assert!((cost_unknown - cost_opus).abs() < 1e-9);
    }
}
