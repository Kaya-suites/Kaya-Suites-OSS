//! Passing case: a token obtained through `UserSession::approve_edit` compiles.

fn main() {
    let session = kaya_core::auth::UserSession {
        user_id: Default::default(),
    };
    let edit = kaya_core::edit::ProposedEdit {
        id: Default::default(),
        kind: kaya_core::edit::ProposedEditKind::DeleteDocument {
            document_id: Default::default(),
        },
    };
    // The only public path to an ApprovalToken.
    let _token = session.approve_edit(&edit);
}
