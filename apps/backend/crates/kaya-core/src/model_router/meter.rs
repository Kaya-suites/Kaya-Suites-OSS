//! In-memory token-usage aggregator.
//!
//! Every LLM call returns a [`TokenUsage`] record. Callers (typically
//! [`super::ModelRouter`]) pass it to [`Meter::record`]. Prompt 9 will wire
//! cloud billing into this via the BSL `kaya-metering` crate.

use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use super::OperationType;

/// Token-usage record for a single LLM call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenUsage {
    pub input_tokens: u32,
    pub output_tokens: u32,
    /// The model string returned by the provider (may differ from requested model).
    pub model: String,
    pub operation: OperationType,
}

/// In-memory aggregator for token usage across all LLM calls.
///
/// Cheap to clone — the inner `Arc<Mutex<_>>` is shared.
#[derive(Debug, Clone, Default)]
pub struct Meter {
    records: Arc<Mutex<Vec<TokenUsage>>>,
}

impl Meter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a usage record.
    pub fn record(&self, usage: TokenUsage) {
        self.records.lock().expect("meter lock poisoned").push(usage);
    }

    pub fn total_input_tokens(&self) -> u32 {
        self.records
            .lock()
            .expect("meter lock poisoned")
            .iter()
            .map(|u| u.input_tokens)
            .sum()
    }

    pub fn total_output_tokens(&self) -> u32 {
        self.records
            .lock()
            .expect("meter lock poisoned")
            .iter()
            .map(|u| u.output_tokens)
            .sum()
    }

    /// Clone the current records for inspection.
    pub fn snapshot(&self) -> Vec<TokenUsage> {
        self.records.lock().expect("meter lock poisoned").clone()
    }

    pub fn reset(&self) {
        self.records.lock().expect("meter lock poisoned").clear();
    }
}
