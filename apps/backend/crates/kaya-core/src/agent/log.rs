//! Tool invocation log — every tool call writes a record here.
//!
//! Records are queryable for the transparency view (FR-14). Cloud mode will
//! persist them to Postgres in Prompt 9.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use serde_json::Value;
use uuid::Uuid;

/// A single recorded tool invocation.
#[derive(Debug, Clone)]
pub struct ToolInvocation {
    /// Unique ID for this invocation record.
    pub id: Uuid,
    /// Groups all tool calls within one agent turn.
    pub turn_id: Uuid,
    pub tool_name: String,
    pub input: Value,
    /// `Ok` = successful result JSON; `Err` = error message string.
    pub output: Result<Value, String>,
    pub latency_ms: u64,
    pub started_at: DateTime<Utc>,
}

/// In-memory log of tool invocations for the current session.
///
/// Cheap to clone (inner `Arc`). Pass an `Arc<InvocationLog>` to
/// [`super::AgentLoop::run`]; inspect it after the stream is consumed.
#[derive(Debug, Clone, Default)]
pub struct InvocationLog {
    records: Arc<Mutex<Vec<ToolInvocation>>>,
}

impl InvocationLog {
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a record. Called by the agent loop; not normally called by
    /// application code.
    pub fn record(&self, inv: ToolInvocation) {
        self.records.lock().expect("log lock poisoned").push(inv);
    }

    /// Clone all records for inspection.
    pub fn entries(&self) -> Vec<ToolInvocation> {
        self.records.lock().expect("log lock poisoned").clone()
    }

    /// Number of recorded invocations.
    pub fn len(&self) -> usize {
        self.records.lock().expect("log lock poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}
