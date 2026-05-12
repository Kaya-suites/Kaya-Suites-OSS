//! `read_document` — return a document's full Markdown body and frontmatter.

use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent::{AgentContext, tool::{Tool, ToolOutput}};
use crate::error::KayaError;

pub struct ReadDocument;

#[async_trait]
impl Tool for ReadDocument {
    fn name(&self) -> &'static str {
        "read_document"
    }

    fn description(&self) -> &'static str {
        "Read the full content and metadata of a single document by its UUID."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["document_id"],
            "properties": {
                "document_id": {
                    "type": "string",
                    "format": "uuid",
                    "description": "The UUID of the document to read."
                }
            }
        })
    }

    async fn invoke(&self, input: Value, ctx: &AgentContext) -> Result<ToolOutput, KayaError> {
        let id_str = input["document_id"]
            .as_str()
            .ok_or_else(|| KayaError::Internal("read_document: missing 'document_id'".into()))?;
        let id: Uuid = id_str
            .parse()
            .map_err(|_| KayaError::Internal(format!("read_document: invalid UUID '{id_str}'")))?;

        let doc = ctx.storage.get_document(id).await?;

        Ok(ToolOutput::value(json!({
            "id": doc.id,
            "title": doc.title,
            "owner": doc.owner,
            "last_reviewed": doc.last_reviewed,
            "tags": doc.tags,
            "related_docs": doc.related_docs,
            "body": doc.body,
        })))
    }
}
