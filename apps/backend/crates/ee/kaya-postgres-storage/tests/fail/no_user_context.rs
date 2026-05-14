//! Failing case: `PostgresAdapter` has private fields. Struct-literal
//! construction from outside the crate is rejected by the compiler, proving
//! that the only construction path is `PostgresAdapter::new(pool, user_context)`.

fn main() {
    // ERROR: fields `pool` and `user_context` are private
    let _ = kaya_postgres_storage::PostgresAdapter {
        pool: todo!(),
        user_context: todo!(),
    };
}
