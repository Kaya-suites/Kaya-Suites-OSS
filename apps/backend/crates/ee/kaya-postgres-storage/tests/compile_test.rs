/// Structural enforcement: `PostgresAdapter` cannot exist without a `UserContext`.
///
/// - `fail/no_user_context.rs` — struct-literal construction exposes private fields,
///   proving no code path can bypass the `new(pool, user_context)` constructor.
#[test]
fn postgres_adapter_requires_user_context() {
    let t = trybuild::TestCases::new();
    t.compile_fail("tests/fail/no_user_context.rs");
}
