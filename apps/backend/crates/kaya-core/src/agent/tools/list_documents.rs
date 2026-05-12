//! `list_documents` — return metadata for all documents (no body text).

use async_trait::async_trait;
use serde_json::{json, Value};

use crate::agent::{AgentContext, tool::{Tool, ToolOutput}};
use crate::error::KayaError;

pub struct ListDocuments;

#[async_trait]
impl Tool for ListDocuments {
    fn name(&self) -> &'static str {
        "list_documents"
    }

    fn description(&self) -> &'static str {
        "List all documents in the knowledge base. \
         Returns metadata only (id, title, tags, last_reviewed) — \
         use read_document to fetch the full body."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {}
        })
    }

    async fn invoke(&self, _input: Value, ctx: &AgentContext) -> Result<ToolOutput, KayaError> {
        let docs = ctx.storage.list_documents().await?;
        let items: Vec<Value> = docs
            .into_iter()
            .map(|d| json!({
                "id": d.id,
                "title": d.title,
                "owner": d.owner,
                "last_reviewed": d.last_reviewed,
                "tags": d.tags,
            }))
            .collect();

        Ok(ToolOutput::value(json!({ "documents": items })))
    }
}
