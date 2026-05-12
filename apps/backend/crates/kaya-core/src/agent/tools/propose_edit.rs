//! `propose_edit` — produce a paragraph-level [`ProposedEdit::Modify`].

use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent::{AgentContext, tool::{Tool, ToolOutput}};
use crate::diff::compute_paragraph_diff;
use crate::edit::{ProposedEdit, ProposedEditKind};
use crate::error::KayaError;

pub struct ProposeEdit;

#[async_trait]
impl Tool for ProposeEdit {
    fn name(&self) -> &'static str {
        "propose_edit"
    }

    fn description(&self) -> &'static str {
        "Propose an edit to an existing document. The change is NOT applied \
         until the user explicitly approves the proposal. The diff is rendered \
         in the UI for review."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["document_id", "new_body"],
            "properties": {
                "document_id": {
                    "type": "string",
                    "format": "uuid",
                    "description": "UUID of the document to edit."
                },
                "new_body": {
                    "type": "string",
                    "description": "Full replacement Markdown body."
                },
                "reason": {
                    "type": "string",
                    "description": "Short explanation of why this change is being proposed."
                }
            }
        })
    }

    async fn invoke(&self, input: Value, ctx: &AgentContext) -> Result<ToolOutput, KayaError> {
        let id_str = input["document_id"]
            .as_str()
            .ok_or_else(|| KayaError::Internal("propose_edit: missing 'document_id'".into()))?;
        let document_id: Uuid = id_str.parse().map_err(|_| {
            KayaError::Internal(format!("propose_edit: invalid UUID '{id_str}'"))
        })?;

        let new_body = input["new_body"]
            .as_str()
            .ok_or_else(|| KayaError::Internal("propose_edit: missing 'new_body'".into()))?
            .to_owned();

        let reason = input["reason"].as_str().unwrap_or("").to_owned();

        // Read the current document to compute the diff.
        let current = ctx.storage.get_document(document_id).await?;
        let diff = compute_paragraph_diff(&current.body, &new_body);

        let edit = ProposedEdit {
            id: Uuid::new_v4(),
            kind: ProposedEditKind::Modify {
                document_id,
                diff: diff.clone(),
                new_body,
            },
        };
        let edit_id = edit.id;

        Ok(ToolOutput::with_edit(
            json!({
                "proposed_edit_id": edit_id,
                "action": "modify",
                "document_id": document_id,
                "reason": reason,
                "changes": diff.changes.len(),
                "status": "pending_approval",
            }),
            edit,
        ))
    }
}
