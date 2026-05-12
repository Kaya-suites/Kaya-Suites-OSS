//! `create_document` — produce a [`ProposedEdit::Create`] awaiting approval.

use async_trait::async_trait;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::agent::{AgentContext, tool::{Tool, ToolOutput}};
use crate::edit::{ProposedEdit, ProposedEditKind};
use crate::error::KayaError;

pub struct CreateDocument;

#[async_trait]
impl Tool for CreateDocument {
    fn name(&self) -> &'static str {
        "create_document"
    }

    fn description(&self) -> &'static str {
        "Propose creating a new document. The document is NOT saved until the \
         user explicitly approves the proposal."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "required": ["title", "body"],
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Document title."
                },
                "body": {
                    "type": "string",
                    "description": "Full Markdown body of the new document."
                }
            }
        })
    }

    async fn invoke(&self, input: Value, _ctx: &AgentContext) -> Result<ToolOutput, KayaError> {
        let title = input["title"]
            .as_str()
            .ok_or_else(|| KayaError::Internal("create_document: missing 'title'".into()))?
            .to_owned();
        let body = input["body"]
            .as_str()
            .ok_or_else(|| KayaError::Internal("create_document: missing 'body'".into()))?
            .to_owned();

        let edit = ProposedEdit {
            id: Uuid::new_v4(),
            kind: ProposedEditKind::Create {
                title: title.clone(),
                body,
            },
        };
        let edit_id = edit.id;

        Ok(ToolOutput::with_edit(
            json!({
                "proposed_edit_id": edit_id,
                "action": "create",
                "title": title,
                "status": "pending_approval",
            }),
            edit,
        ))
    }
}
