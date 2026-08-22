//! Lock-topology regression test for the CurrentBoot foundation
//! (`docs/decisions/0014-agent-credential-lookup-and-boot-context-correlation.md`;
//! `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
//! "Authoritative current boot and durable Server state"): proves the
//! *final* schema (after migration
//! `0005_remove_current_boot_fk_lock_dependency.sql`) does not silently
//! reacquire a `boot_contexts` lock during an Endpoint-only update.
//!
//! This matters because migration
//! `0004_current_boot_and_trusted_bootstrap_state.sql` originally added a
//! composite `(current_boot_context_id, current_boot_nonce)` FOREIGN KEY
//! referencing `boot_contexts (boot_context_id, boot_nonce)`. PostgreSQL
//! enforces a referencing-side composite FK via an internal trigger that
//! takes a `FOR KEY SHARE`-equivalent lock on the referenced row on every
//! UPDATE touching the referencing columns — with that FK still active, an
//! Endpoint-only UPDATE already holding the `endpoints` row lock would
//! therefore implicitly wait on `boot_contexts`, an
//! `Endpoint -> BootContext` dependency the accepted lock order forbids
//! (the forthcoming `BootstrapEvidence` transition must operate under
//! Endpoint lock alone). Migration 0005 removed that FK specifically to
//! eliminate this dependency.
//!
//! Requires a real, reachable PostgreSQL instance — see `support::TestDatabase`.

mod support;

use chrono::{Duration, Utc};
use sqlx::Row;
use support::TestDatabase;
use uuid::Uuid;

#[tokio::test]
async fn endpoint_only_update_succeeds_while_boot_context_row_remains_locked() {
    let db = TestDatabase::setup().await;

    // 1. A BootContext A with an exact 32-byte nonce A.
    let boot_context_id = Uuid::new_v4().into_bytes().to_vec();
    let nonce = vec![0x99u8; 32];
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO boot_contexts (boot_context_id, verifier, issued_at, expires_at, inventory_signal, boot_nonce) \
         VALUES ($1, $2, $3, $4, 'sim-lock-topology', $5)",
    )
    .bind(&boot_context_id)
    .bind(vec![0xCDu8; 48])
    .bind(now)
    .bind(now + Duration::minutes(5))
    .bind(&nonce)
    .execute(&db.pool)
    .await
    .expect("insert BootContext A");

    // 2. An Endpoint whose complete CurrentBoot points at A.
    let endpoint_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO endpoints (id, inventory_signal, identity_state, created_at, updated_at, \
         current_boot_context_id, current_boot_nonce, trusted_bootstrap_state) \
         VALUES ($1, $2, 'PendingEnrollment', $3, $3, $4, $5, 'NotEstablished')",
    )
    .bind(endpoint_id)
    .bind(format!("sim-lock-topology-{endpoint_id}"))
    .bind(now)
    .bind(&boot_context_id)
    .bind(&nonce)
    .execute(&db.pool)
    .await
    .expect("insert Endpoint with a complete CurrentBoot pointing at A");

    // 3. T1: lock BootContext A. `.await` here only resolves once the lock
    // is actually held (PostgreSQL grants it synchronously against this
    // still-uncontended row), so T1 is guaranteed to hold the lock before
    // any T2 statement below is even issued — no sleep needed to establish
    // this ordering.
    let mut t1 = db.pool.begin().await.expect("begin T1");
    let locked_row = sqlx::query(
        "SELECT boot_context_id FROM boot_contexts WHERE boot_context_id = $1 FOR UPDATE",
    )
    .bind(&boot_context_id)
    .fetch_one(&mut *t1)
    .await
    .expect("T1 must acquire the BootContext A row lock");
    let locked_id: Vec<u8> = locked_row.try_get("boot_context_id").unwrap();
    assert_eq!(locked_id, boot_context_id);

    // 4. T2: a separate connection/transaction (sqlx's pool hands out a
    // distinct physical connection since T1's connection is checked out and
    // held open), updating only Endpoint state. A small lock_timeout turns
    // any unexpected blocking into a prompt, diagnosable failure rather than
    // a hang — it is a watchdog, not the pass condition.
    let mut t2 = db
        .pool
        .begin()
        .await
        .expect("begin T2 on a separate connection");
    sqlx::query("SET LOCAL lock_timeout = '2s'")
        .execute(&mut *t2)
        .await
        .expect("set T2's lock_timeout watchdog");

    // 5. The Endpoint-only UPDATE must complete successfully while T1 still
    // holds BootContext A's row lock. If the removed composite FK (or any
    // replacement trigger/lookup) still implicitly locked boot_contexts,
    // this statement would block until lock_timeout fires and return an
    // error — a timeout/error here is a test FAILURE, not a pass condition.
    let update_result =
        sqlx::query("UPDATE endpoints SET trusted_bootstrap_state = 'Established' WHERE id = $1")
            .bind(endpoint_id)
            .execute(&mut *t2)
            .await;

    assert!(
        update_result.is_ok(),
        "an Endpoint-only UPDATE must succeed while a separate transaction holds BootContext \
         A's row lock — an error/timeout here means an Endpoint -> BootContext lock dependency \
         still exists: {:?}",
        update_result.err()
    );
    assert_eq!(update_result.unwrap().rows_affected(), 1);

    t2.commit().await.expect("commit T2");

    // T1 never wrote anything; release its lock explicitly rather than
    // leaving it for connection teardown.
    t1.rollback().await.expect("rollback T1");

    // The Endpoint-only change from T2 is durably visible.
    let final_state: String =
        sqlx::query_scalar("SELECT trusted_bootstrap_state::text FROM endpoints WHERE id = $1")
            .bind(endpoint_id)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(final_state, "Established");

    db.teardown().await;
}
