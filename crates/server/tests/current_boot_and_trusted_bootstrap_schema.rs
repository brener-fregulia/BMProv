//! Component/Integration tests proving migration
//! `0004_current_boot_and_trusted_bootstrap_state.sql` exists as intended:
//! `boot_contexts.boot_nonce`, the `trusted_bootstrap_state` native enum, the
//! nullable all-or-none Endpoint current-boot projection
//! (`current_boot_context_id`/`current_boot_nonce`/`trusted_bootstrap_state`),
//! and the composite `(boot_context_id, boot_nonce)` relational correlation
//! invariant between them.
//!
//! This is a schema-only checkpoint for these specific constraints (no
//! evidence-processing Adapter behavior exists yet), so these tests assert
//! PostgreSQL schema invariants directly via `information_schema`/
//! `pg_catalog` and raw SQL — never Domain/Application business logic. Real
//! redemption-path current-boot behavior is covered separately by
//! `tests/enrollment_lifecycle.rs`.
//!
//! Requires a real, reachable PostgreSQL instance — see `support::TestDatabase`.

mod support;

use chrono::{Duration, Utc};
use sqlx::PgPool;
use support::TestDatabase;
use uuid::Uuid;

async fn insert_endpoint(pool: &PgPool) -> Uuid {
    let id = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO endpoints (id, inventory_signal, identity_state, created_at, updated_at) \
         VALUES ($1, $2, 'PendingEnrollment', $3, $3)",
    )
    .bind(id)
    .bind(format!("current-boot-schema-test-{id}"))
    .bind(now)
    .execute(pool)
    .await
    .expect("insert baseline endpoint row");
    id
}

/// Inserts a `boot_contexts` row with the given exact 32-byte nonce,
/// returning its 16-byte `boot_context_id` so callers can reference the
/// resulting `(boot_context_id, boot_nonce)` pair from an Endpoint's
/// current-boot projection.
async fn insert_boot_context_with_nonce(pool: &PgPool, nonce: [u8; 32]) -> Vec<u8> {
    let boot_context_id = Uuid::new_v4().into_bytes().to_vec();
    let now = Utc::now();
    sqlx::query(
        "INSERT INTO boot_contexts (boot_context_id, verifier, issued_at, expires_at, inventory_signal, boot_nonce) \
         VALUES ($1, $2, $3, $4, 'sim-current-boot-schema', $5)",
    )
    .bind(&boot_context_id)
    .bind(vec![0xCDu8; 48])
    .bind(now)
    .bind(now + Duration::minutes(5))
    .bind(nonce.to_vec())
    .execute(pool)
    .await
    .expect("insert boot_contexts row with an exact 32-byte nonce");
    boot_context_id
}

async fn udt_name(pool: &PgPool, table_name: &str, column_name: &str) -> String {
    sqlx::query_scalar(
        "SELECT udt_name FROM information_schema.columns \
         WHERE table_name = $1 AND column_name = $2",
    )
    .bind(table_name)
    .bind(column_name)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn migration_0004_columns_exist_and_older_migrations_are_unchanged() {
    let db = TestDatabase::setup().await;

    // New columns from migration 0004.
    assert_eq!(
        udt_name(&db.pool, "boot_contexts", "boot_nonce").await,
        "bytea"
    );
    assert_eq!(
        udt_name(&db.pool, "endpoints", "current_boot_context_id").await,
        "bytea"
    );
    assert_eq!(
        udt_name(&db.pool, "endpoints", "current_boot_nonce").await,
        "bytea"
    );
    assert_eq!(
        udt_name(&db.pool, "endpoints", "trusted_bootstrap_state").await,
        "trusted_bootstrap_state"
    );

    // Columns owned by 0001-0003 remain exactly as those migrations left
    // them — proving 0004 only added to the schema, never altered them.
    assert_eq!(
        udt_name(&db.pool, "endpoints", "identity_state").await,
        "endpoint_identity_state"
    );
    assert_eq!(
        udt_name(&db.pool, "boot_contexts", "boot_context_id").await,
        "bytea"
    );

    let applied_versions: Vec<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&db.pool)
            .await
            .unwrap();
    assert_eq!(applied_versions, vec![1, 2, 3, 4]);

    db.teardown().await;
}

#[tokio::test]
async fn trusted_bootstrap_state_is_native_postgres_enum_with_expected_labels() {
    let db = TestDatabase::setup().await;

    let type_kind: String = sqlx::query_scalar(
        "SELECT typtype::text FROM pg_type WHERE typname = 'trusted_bootstrap_state'",
    )
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        type_kind, "e",
        "trusted_bootstrap_state must be a native enum"
    );

    let labels: Vec<String> = sqlx::query_scalar(
        "SELECT enumlabel FROM pg_enum \
         JOIN pg_type ON pg_enum.enumtypid = pg_type.oid \
         WHERE pg_type.typname = 'trusted_bootstrap_state' \
         ORDER BY enumsortorder",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();
    assert_eq!(labels, vec!["NotEstablished", "Established"]);

    db.teardown().await;
}

#[tokio::test]
async fn boot_nonce_accepts_exactly_32_bytes() {
    let db = TestDatabase::setup().await;
    insert_boot_context_with_nonce(&db.pool, [0x11; 32]).await;
    db.teardown().await;
}

#[tokio::test]
async fn boot_nonce_permits_null_for_historical_rows() {
    let db = TestDatabase::setup().await;
    let now = Utc::now();

    sqlx::query(
        "INSERT INTO boot_contexts (boot_context_id, verifier, issued_at, expires_at, inventory_signal, boot_nonce) \
         VALUES ($1, $2, $3, $4, 'sim-historical', NULL)",
    )
    .bind(Uuid::new_v4().into_bytes().to_vec())
    .bind(vec![0xCDu8; 48])
    .bind(now)
    .bind(now + Duration::minutes(5))
    .execute(&db.pool)
    .await
    .expect("a NULL boot_nonce must be accepted for a historical row");

    db.teardown().await;
}

#[tokio::test]
async fn boot_nonce_rejects_a_non_32_byte_non_null_value() {
    let db = TestDatabase::setup().await;
    let now = Utc::now();

    let result = sqlx::query(
        "INSERT INTO boot_contexts (boot_context_id, verifier, issued_at, expires_at, inventory_signal, boot_nonce) \
         VALUES ($1, $2, $3, $4, 'sim-bad-nonce', $5)",
    )
    .bind(Uuid::new_v4().into_bytes().to_vec())
    .bind(vec![0xCDu8; 48])
    .bind(now)
    .bind(now + Duration::minutes(5))
    .bind(vec![0x01u8; 31])
    .execute(&db.pool)
    .await;

    assert!(result.is_err(), "a 31-byte boot_nonce must be rejected");

    db.teardown().await;
}

#[tokio::test]
async fn endpoint_current_boot_all_null_is_accepted() {
    let db = TestDatabase::setup().await;
    // `insert_endpoint` never sets the current-boot columns, so this proves
    // the all-NULL case (legacy/unknown current boot) directly.
    insert_endpoint(&db.pool).await;
    db.teardown().await;
}

#[tokio::test]
async fn endpoint_current_boot_complete_triple_is_accepted() {
    let db = TestDatabase::setup().await;
    let nonce = [0x22u8; 32];
    let boot_context_id = insert_boot_context_with_nonce(&db.pool, nonce).await;
    let endpoint_id = insert_endpoint(&db.pool).await;

    sqlx::query(
        "UPDATE endpoints SET current_boot_context_id = $1, current_boot_nonce = $2, \
         trusted_bootstrap_state = 'NotEstablished' WHERE id = $3",
    )
    .bind(&boot_context_id)
    .bind(nonce.to_vec())
    .bind(endpoint_id)
    .execute(&db.pool)
    .await
    .expect("a complete, correctly-paired current-boot triple must be accepted");

    db.teardown().await;
}

#[tokio::test]
async fn endpoint_current_boot_partial_combinations_are_rejected() {
    let db = TestDatabase::setup().await;
    let nonce = [0x33u8; 32];
    let boot_context_id = insert_boot_context_with_nonce(&db.pool, nonce).await;
    let endpoint_id = insert_endpoint(&db.pool).await;

    // Only current_boot_context_id set.
    let only_id = sqlx::query("UPDATE endpoints SET current_boot_context_id = $1 WHERE id = $2")
        .bind(&boot_context_id)
        .bind(endpoint_id)
        .execute(&db.pool)
        .await;
    assert!(
        only_id.is_err(),
        "current_boot_context_id alone (nonce/state NULL) must be rejected"
    );

    // Only current_boot_nonce and trusted_bootstrap_state set, id missing.
    let missing_id = sqlx::query(
        "UPDATE endpoints SET current_boot_nonce = $1, trusted_bootstrap_state = 'NotEstablished' \
         WHERE id = $2",
    )
    .bind(nonce.to_vec())
    .bind(endpoint_id)
    .execute(&db.pool)
    .await;
    assert!(
        missing_id.is_err(),
        "current_boot_nonce/state without current_boot_context_id must be rejected"
    );

    db.teardown().await;
}

#[tokio::test]
async fn endpoint_current_boot_bad_byte_lengths_are_rejected() {
    let db = TestDatabase::setup().await;
    let nonce = [0x44u8; 32];
    let boot_context_id = insert_boot_context_with_nonce(&db.pool, nonce).await;
    let endpoint_id = insert_endpoint(&db.pool).await;

    let short_context_id = sqlx::query(
        "UPDATE endpoints SET current_boot_context_id = $1, current_boot_nonce = $2, \
         trusted_bootstrap_state = 'NotEstablished' WHERE id = $3",
    )
    .bind(vec![0xAAu8; 15])
    .bind(nonce.to_vec())
    .bind(endpoint_id)
    .execute(&db.pool)
    .await;
    assert!(
        short_context_id.is_err(),
        "a 15-byte current_boot_context_id must be rejected"
    );

    let short_nonce = sqlx::query(
        "UPDATE endpoints SET current_boot_context_id = $1, current_boot_nonce = $2, \
         trusted_bootstrap_state = 'NotEstablished' WHERE id = $3",
    )
    .bind(&boot_context_id)
    .bind(vec![0xBBu8; 31])
    .bind(endpoint_id)
    .execute(&db.pool)
    .await;
    assert!(
        short_nonce.is_err(),
        "a 31-byte current_boot_nonce must be rejected"
    );

    db.teardown().await;
}

#[tokio::test]
async fn endpoint_current_boot_fk_rejects_a_mismatched_boot_context_nonce_pair() {
    let db = TestDatabase::setup().await;
    let real_nonce = [0x55u8; 32];
    let boot_context_id = insert_boot_context_with_nonce(&db.pool, real_nonce).await;
    let endpoint_id = insert_endpoint(&db.pool).await;

    // A well-formed 32-byte nonce, but NOT the one actually persisted for
    // this boot_context_id — the composite FK must reject this pairing even
    // though each individual column is independently valid.
    let wrong_nonce = [0x66u8; 32];
    let result = sqlx::query(
        "UPDATE endpoints SET current_boot_context_id = $1, current_boot_nonce = $2, \
         trusted_bootstrap_state = 'NotEstablished' WHERE id = $3",
    )
    .bind(&boot_context_id)
    .bind(wrong_nonce.to_vec())
    .bind(endpoint_id)
    .execute(&db.pool)
    .await;

    assert!(
        result.is_err(),
        "a (boot_context_id, boot_nonce) pair that does not match any real BootContext row \
         must be rejected by the composite foreign key"
    );

    db.teardown().await;
}

#[tokio::test]
async fn endpoint_current_boot_fk_rejects_a_dangling_boot_context_id() {
    let db = TestDatabase::setup().await;
    let endpoint_id = insert_endpoint(&db.pool).await;

    let dangling_id = Uuid::new_v4().into_bytes().to_vec();
    let nonce = [0x77u8; 32];
    let result = sqlx::query(
        "UPDATE endpoints SET current_boot_context_id = $1, current_boot_nonce = $2, \
         trusted_bootstrap_state = 'NotEstablished' WHERE id = $3",
    )
    .bind(dangling_id)
    .bind(nonce.to_vec())
    .bind(endpoint_id)
    .execute(&db.pool)
    .await;

    assert!(
        result.is_err(),
        "a current_boot_context_id with no matching boot_contexts row must be rejected"
    );

    db.teardown().await;
}
