//! Failing case: `ApprovalToken::new` is `pub(crate)` — calling it from
//! outside `kaya-core` must be rejected by the compiler.

fn main() {
    let session = kaya_core::auth::UserSession {
        user_id: Default::default(),
    };
    // ERROR: function `new` is private (pub(crate))
    let _token = kaya_core::edit::ApprovalToken::new(&session, Default::default());
}
