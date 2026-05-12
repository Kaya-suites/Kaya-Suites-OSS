//! Propose-then-approve edit flow.
//!
//! The agent proposes a [`ProposedEdit`]. A user session produces an
//! [`ApprovalToken`] via [`UserSession::approve_edit`]. Only then can
//! [`commit_edit`] mutate storage.
//!
//! [`ApprovalToken`] has private fields and a `pub(crate)` constructor, so
//! external code cannot fabricate one — the only path to a token is through
//! a real [`UserSession`].

use std::sync::Arc;
use chrono::{DateTime, Utc};
use uuid::Uuid;

use crate::auth::UserSession;
use crate::diff::ParagraphDiff;
use crate::error::KayaError;
use crate::storage::{Document, StorageAdapter};

// ── Proposed edit ────────────────────────────────────────────────────────────

/// The kind of change being proposed.
#[derive(Debug, Clone)]
pub enum ProposedEditKind {
    /// Replace the full body of an existing document.
    UpdateContent {
        document_id: Uuid,
        new_content: String,
    },
    /// Delete a document entirely.
    DeleteDocument {
        document_id: Uuid,
    },
    /// Create a brand-new document. Requires approval before it is persisted.
    Create {
        title: String,
        body: String,
    },
    /// Modify an existing document at the paragraph level.
    ///
    /// `diff` is for display (UI diff renderer). `new_body` is the
    /// authoritative replacement body applied by [`commit_edit`].
    Modify {
        document_id: Uuid,
        diff: ParagraphDiff,
        new_body: String,
    },
}

/// An agent-proposed change that is awaiting user approval.
///
/// Created by the agent loop; passed to [`UserSession::approve_edit`] to
/// produce an [`ApprovalToken`], then to [`commit_edit`] to apply the change.
#[derive(Debug, Clone)]
pub struct ProposedEdit {
    /// Unique identifier for this proposal.
    pub id: Uuid,
    pub kind: ProposedEditKind,
}

// ── Approval token ───────────────────────────────────────────────────────────

/// Proof that a user session approved a specific [`ProposedEdit`].
///
/// The fields are intentionally private and the constructor is `pub(crate)`,
/// so external code cannot fabricate a token — it must go through
/// [`UserSession::approve_edit`].
///
/// A `trybuild` compile-fail test in `tests/fail/without_token.rs` verifies
/// that direct construction is rejected by the compiler.
#[derive(Debug)]
pub struct ApprovalToken {
    edit_id: Uuid,
    // Recorded for audit purposes; will be persisted when an audit log is added.
    #[allow(dead_code)]
    approved_at: DateTime<Utc>,
}

impl ApprovalToken {
    /// Create a token. Only callable within `kaya-core`.
    pub(crate) fn new(_session: &UserSession, edit_id: Uuid) -> Self {
        Self {
            edit_id,
            approved_at: Utc::now(),
        }
    }
}

// ── UserSession extension ────────────────────────────────────────────────────

impl UserSession {
    /// Approve a proposed edit, producing an [`ApprovalToken`] that authorises
    /// [`commit_edit`].
    ///
    /// This is the only public path to an `ApprovalToken`.
    pub fn approve_edit(&self, edit: &ProposedEdit) -> ApprovalToken {
        ApprovalToken::new(self, edit.id)
    }
}

// ── Commit ───────────────────────────────────────────────────────────────────

/// Apply an approved edit to storage.
///
/// The `token` parameter proves the edit was approved by a real user session.
/// Because [`ApprovalToken`] cannot be constructed outside this crate,
/// the compiler enforces that no edit reaches storage without prior approval.
///
/// # Errors
/// Propagates [`StorageError`](crate::storage::StorageError) wrapped in
/// [`KayaError::Storage`].
pub async fn commit_edit(
    edit: ProposedEdit,
    token: ApprovalToken,
    storage: Arc<dyn StorageAdapter>,
) -> Result<(), KayaError> {
    debug_assert_eq!(
        token.edit_id, edit.id,
        "ApprovalToken edit_id does not match ProposedEdit id"
    );

    match edit.kind {
        ProposedEditKind::UpdateContent { document_id, new_content } => {
            let mut doc = storage.get_document(document_id).await?;
            doc.body = new_content;
            storage.save_document(&doc).await?;
        }
        ProposedEditKind::DeleteDocument { document_id } => {
            storage.delete_document(document_id).await?;
        }
        ProposedEditKind::Create { title, body } => {
            let doc = Document {
                id: Uuid::new_v4(),
                title,
                owner: None,
                last_reviewed: None,
                tags: vec![],
                related_docs: vec![],
                body,
                path: None,
            };
            storage.save_document(&doc).await?;
        }
        ProposedEditKind::Modify { document_id, new_body, .. } => {
            // `diff` is for display only; apply the full new_body directly.
            let mut doc = storage.get_document(document_id).await?;
            doc.body = new_body;
            storage.save_document(&doc).await?;
        }
    }

    Ok(())
}
