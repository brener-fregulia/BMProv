// Cargo does not track the `migrations` directory read by `sqlx::migrate!`
// in `src/adapters/postgres/mod.rs`. Without this, adding or editing a
// migration file alone does not invalidate this crate's build, and an
// incremental `cargo build`/`cargo test` can keep running against a stale
// embedded migration set.
fn main() {
    println!("cargo:rerun-if-changed=migrations");
}
