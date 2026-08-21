//! Component/Integration test: `BootOrchestrationService` against the real
//! `PostgresBootContextRepository` and a real PostgreSQL instance (ADR-0013),
//! proving ADR-0014 point 11's persist-before-deliver issuance ordering end
//! to end — not just against the in-memory fake used by the Application-level
//! unit tests in `crates/server/src/application/mod.rs`. Specifically: that
//! an Application issuance using the real Adapter returns a credential only
//! after its `BootContext` row is durably committed and queryable by a
//! separate connection.
//!
//! Requires a real, reachable PostgreSQL instance — see `support::TestDatabase`.

mod support;

use std::sync::Arc;

use bamep_domain::presented_credential::CredentialKind;
use bamep_domain::BootNonce;
use bamep_server::adapters::postgres::PostgresBootContextRepository;
use bamep_server::application::BootOrchestrationService;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use support::TestDatabase;

async fn boot_context_row_exists(pool: &PgPool, boot_context_id: &[u8]) -> bool {
    let count: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM boot_contexts WHERE boot_context_id = $1")
            .bind(boot_context_id.to_vec())
            .fetch_one(pool)
            .await
            .unwrap();
    count == 1
}

#[tokio::test]
async fn issuance_returns_only_after_the_boot_context_row_is_durably_queryable() {
    let db = TestDatabase::setup().await;
    let repo = Arc::new(PostgresBootContextRepository::new(db.pool.clone()));
    let service = BootOrchestrationService::new(repo, Duration::minutes(5));

    let now = Utc::now();
    let boot_nonce = BootNonce::from_bytes([0x11; 32]);
    let credential = service
        .issue_enrollment_credential("sim-boot-orch-pg-01", boot_nonce, now)
        .await
        .expect("issuance against the real PostgreSQL adapter must succeed");

    assert_eq!(credential.kind(), CredentialKind::Enrollment);

    // A separate pool/connection than the one the service issued through —
    // proving durable commit, not merely in-process/same-connection
    // visibility.
    let fresh_pool = PgPool::connect(&db.db_url)
        .await
        .expect("reconnect to the same durable database");
    assert!(
        boot_context_row_exists(&fresh_pool, &credential.lookup_id().to_bytes()).await,
        "the BootContext row must already be committed and visible to a fresh connection \
         by the time issue_enrollment_credential returns"
    );

    db.teardown().await;
}
