//! `delete_document` — produce a [`ProposedEditKind::DeleteDocument`] awaiting approval.

use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent::{AgentContext, tool::{Tool, ToolOutput}};
use crate::edit::{ProposedEdit, ProposedEditKind};
use crate::error::KayaError;

pub struct DeleteDocument;

#[async_trait]
impl Tool for DeleteDocument {
    fn name(&self) -> &'static str {
        "delete_document"
    }

    fn description(&self) -> &'static str {
        "Propose deleting a document permanently. The document is NOT removed \
         until the user explicitly approves the proposal."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["document_id"],
            "properties": {
                "document_id": {
                    "type": "string",
                    "format": "uuid",
                    "description": "UUID of the document to delete."
                },
                "reason": {
                    "type": "string",
                    "description": "Short explanation of why this document should be deleted."
                }
            }
        })
    }

    async fn invoke(&self, input: Value, ctx: &AgentContext) -> Result<ToolOutput, KayaError> {
        let id_str = input["document_id"]
            .as_str()
            .ok_or_else(|| KayaError::Internal("delete_document: missing 'document_id'".into()))?;
        let document_id: Uuid = id_str.parse().map_err(|_| {
            KayaError::Internal(format!("delete_document: invalid UUID '{id_str}'"))
        })?;

        let reason = input["reason"].as_str().unwrap_or("").to_owned();

        // Verify the document exists before proposing deletion.
        let doc = ctx.storage.get_document(document_id).await?;

        let edit = ProposedEdit {
            id: Uuid::new_v4(),
            kind: ProposedEditKind::DeleteDocument { document_id },
        };
        let edit_id = edit.id;

        Ok(ToolOutput::with_edit(
            json!({
                "proposed_edit_id": edit_id,
                "action": "delete",
                "document_id": document_id,
                "document_title": doc.title,
                "reason": reason,
                "status": "pending_approval",
            }),
            edit,
        ))
    }
}
