/// Structural enforcement: `ApprovalToken` cannot be fabricated externally.
///
/// - `pass/with_token.rs`   — obtaining a token via `UserSession::approve_edit` compiles.
/// - `fail/without_token.rs` — calling the private `ApprovalToken::new` does not compile.
#[test]
fn approval_token_is_unforgeable() {
    let t = trybuild::TestCases::new();
    t.pass("tests/pass/with_token.rs");
    t.compile_fail("tests/fail/without_token.rs");
}
