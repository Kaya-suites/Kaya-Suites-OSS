//! `find_stale_references` — scan documents mentioning a hint entity.

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::agent::{AgentContext, tool::{Tool, ToolOutput}};
use crate::error::KayaError;

pub struct FindStaleReferences;

#[async_trait]
impl Tool for FindStaleReferences {
    fn name(&self) -> &'static str {
        "find_stale_references"
    }

    fn description(&self) -> &'static str {
        "Find documents that may contain stale references to a given entity, \
         concept, or topic. Returns candidates with a reason string explaining \
         why they might need review."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["hint"],
            "properties": {
                "hint": {
                    "type": "string",
                    "description": "The entity, term, or phrase to look for."
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum candidates to return (default 10).",
                    "default": 10
                }
            }
        })
    }

    async fn invoke(&self, input: Value, ctx: &AgentContext) -> Result<ToolOutput, KayaError> {
        let hint = input["hint"]
            .as_str()
            .ok_or_else(|| KayaError::Internal("find_stale_references: missing 'hint'".into()))?
            .to_owned();
        let limit = input["limit"].as_u64().unwrap_or(10) as usize;
        let hint_lower = hint.to_lowercase();

        let docs = ctx.storage.list_documents().await?;

        let mut candidates: Vec<Value> = docs
            .into_iter()
            .filter_map(|doc| {
                let in_title = doc.title.to_lowercase().contains(&hint_lower);
                let in_body = doc.body.to_lowercase().contains(&hint_lower);
                if !(in_title || in_body) {
                    return None;
                }
                let reason = if in_title && in_body {
                    format!("Mentions '{hint}' in title and body — may need review.")
                } else if in_title {
                    format!("Title references '{hint}' — check if still accurate.")
                } else {
                    // Count occurrences to gauge staleness risk.
                    let occurrences = doc
                        .body
                        .to_lowercase()
                        .matches(hint_lower.as_str())
                        .count();
                    format!("Body mentions '{hint}' {occurrences} time(s) — verify currency.")
                };
                Some(json!({
                    "id": doc.id,
                    "title": doc.title,
                    "last_reviewed": doc.last_reviewed,
                    "reason": reason,
                }))
            })
            .take(limit)
            .collect();

        // Sort: documents never reviewed first (most likely stale).
        candidates.sort_by_key(|c| c["last_reviewed"].is_null());

        Ok(ToolOutput::value(json!({ "candidates": candidates })))
    }
}
