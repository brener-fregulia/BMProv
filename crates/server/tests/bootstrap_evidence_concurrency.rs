//! Deterministic PostgreSQL lock-order coverage for BootstrapEvidence A racing
//! a genuine reboot B. Test-local triggers pause the transaction only after
//! PostgreSQL has acquired the Endpoint row lock; no production hook exists.

mod support;

use std::sync::Arc;

use bamep_agent_protocol::BootstrapEvidenceMessage;
use bamep_domain::{BootNonce, EndpointId, TrustedBootstrapState};
use bamep_server::adapters::postgres::{
    PostgresBootContextRepository, PostgresCredentialRedemptionRepository,
    PostgresEndpointRepository,
};
use bamep_server::application::{
    BootOrchestrationService, BootstrapEvidenceResult, BootstrapEvidenceService, EnrollmentService,
    RedeemResult,
};
use bamep_server::ports::EndpointRepository;
use bamep_trusted_bootstrap::fixture::FixtureAssertionSigner;
use bamep_trusted_bootstrap::{AcceptedSiteKeys, ServerCertFingerprint};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use support::TestDatabase;

const EVIDENCE_BARRIER_KEY: i64 = 17_001;
const REBOOT_BARRIER_KEY: i64 = 17_002;

struct RaceFixture {
    endpoint_id: EndpointId,
    nonce_b: BootNonce,
    evidence_a: BootstrapEvidenceMessage,
    fingerprint: ServerCertFingerprint,
    enrollment:
        Arc<EnrollmentService<PostgresEndpointRepository, PostgresCredentialRedemptionRepository>>,
    evidence_service: Arc<BootstrapEvidenceService<PostgresEndpointRepository>>,
    enrollment_b_wire: String,
}

async fn setup_race(db: &TestDatabase, signal: &str) -> RaceFixture {
    let nonce_a = BootNonce::from_bytes([0xA1; 32]);
    let nonce_b = BootNonce::from_bytes([0xB2; 32]);
    let fingerprint = ServerCertFingerprint::from_sha256_digest([0xC3; 32]);
    let signer = FixtureAssertionSigner::from_seed([0xD4; 32]);
    let boot = BootOrchestrationService::new(
        Arc::new(PostgresBootContextRepository::new(db.pool.clone())),
        Duration::minutes(5),
    );
    let enrollment = Arc::new(EnrollmentService::new(
        Arc::new(PostgresEndpointRepository::new(db.pool.clone())),
        Arc::new(PostgresCredentialRedemptionRepository::new(db.pool.clone())),
    ));
    let enrollment_a = boot
        .issue_enrollment_credential(signal, nonce_a, Utc::now())
        .await
        .unwrap();
    let RedeemResult::Established { endpoint_id, .. } = enrollment
        .redeem(&enrollment_a.to_wire_value())
        .await
        .unwrap()
    else {
        panic!("boot A must establish the Endpoint")
    };
    let enrollment_b = boot
        .issue_enrollment_credential(signal, nonce_b, Utc::now())
        .await
        .unwrap();
    let evidence_a = BootstrapEvidenceMessage::new(
        nonce_a.to_wire_value(),
        signer.sign_v1(nonce_a, fingerprint).to_wire_value(),
    );
    let evidence_service = Arc::new(BootstrapEvidenceService::new(
        Arc::new(PostgresEndpointRepository::new(db.pool.clone())),
        AcceptedSiteKeys::single(signer.public_key()),
    ));
    RaceFixture {
        endpoint_id,
        nonce_b,
        evidence_a,
        fingerprint,
        enrollment,
        evidence_service,
        enrollment_b_wire: enrollment_b.to_wire_value(),
    }
}

async fn hold_advisory_lock(pool: &PgPool, key: i64) -> sqlx::pool::PoolConnection<sqlx::Postgres> {
    let mut connection = pool.acquire().await.unwrap();
    sqlx::query("SELECT pg_advisory_lock($1)")
        .bind(key)
        .execute(&mut *connection)
        .await
        .unwrap();
    connection
}

async fn release_advisory_lock(
    connection: &mut sqlx::pool::PoolConnection<sqlx::Postgres>,
    key: i64,
) {
    let released: bool = sqlx::query_scalar("SELECT pg_advisory_unlock($1)")
        .bind(key)
        .fetch_one(&mut **connection)
        .await
        .unwrap();
    assert!(released);
}

async fn wait_for_advisory_waiter(pool: &PgPool) {
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pg_locks WHERE locktype = 'advisory' AND database = (SELECT oid FROM pg_database WHERE datname = current_database()) AND NOT granted",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting >= 1 {
            return;
        }
        tokio::task::yield_now().await;
    }
}

async fn advisory_waiter_reached(pool: &PgPool) {
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        wait_for_advisory_waiter(pool),
    )
    .await
    .expect("the first transaction must reach its test-local advisory barrier");
}

async fn wait_for_two_lock_waiters(pool: &PgPool) {
    loop {
        let waiting: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM pg_stat_activity WHERE datname = current_database() AND wait_event_type = 'Lock'",
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting >= 2 {
            return;
        }
        tokio::task::yield_now().await;
    }
}

async fn both_operations_are_lock_blocked(pool: &PgPool) {
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        wait_for_two_lock_waiters(pool),
    )
    .await
    .expect("the competing transaction must wait on the Endpoint lock");
}

async fn assert_final_boot_b(db: &TestDatabase, fixture: &RaceFixture) {
    let endpoint = PostgresEndpointRepository::new(db.pool.clone())
        .find_by_id(fixture.endpoint_id)
        .await
        .unwrap()
        .unwrap();
    let current = endpoint.current_boot.unwrap();
    assert_eq!(current.boot_nonce(), fixture.nonce_b);
    assert_eq!(
        current.trusted_bootstrap(),
        TrustedBootstrapState::NotEstablished
    );
}

#[tokio::test]
async fn evidence_wins_endpoint_lock_then_reboot_b_resets_final_state() {
    let db = TestDatabase::setup().await;
    let fixture = setup_race(&db, "race-evidence-first").await;
    sqlx::query(
        "CREATE FUNCTION pause_evidence_update() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.trusted_bootstrap_state::text = 'NotEstablished' AND NEW.trusted_bootstrap_state::text = 'Established' THEN PERFORM pg_advisory_xact_lock(17001); END IF; RETURN NEW; END $$",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TRIGGER pause_evidence_update BEFORE UPDATE ON endpoints FOR EACH ROW EXECUTE FUNCTION pause_evidence_update()",
    )
    .execute(&db.pool)
    .await
    .unwrap();

    // Hold BootContext A throughout evidence processing. Reaching the
    // Endpoint-update trigger proves evidence has no BootContext dependency.
    let context_id: Vec<u8> =
        sqlx::query_scalar("SELECT current_boot_context_id FROM endpoints WHERE id = $1")
            .bind(fixture.endpoint_id.0)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    let mut context_lock = db.pool.begin().await.unwrap();
    sqlx::query("SELECT boot_context_id FROM boot_contexts WHERE boot_context_id = $1 FOR UPDATE")
        .bind(context_id)
        .fetch_one(&mut *context_lock)
        .await
        .unwrap();

    let mut barrier = hold_advisory_lock(&db.pool, EVIDENCE_BARRIER_KEY).await;
    let service = Arc::clone(&fixture.evidence_service);
    let evidence = fixture.evidence_a.clone();
    let endpoint_id = fixture.endpoint_id;
    let fingerprint = fixture.fingerprint;
    let evidence_task = tokio::spawn(async move {
        service
            .verify_and_establish(endpoint_id, &evidence, fingerprint)
            .await
    });
    advisory_waiter_reached(&db.pool).await;

    let enrollment = Arc::clone(&fixture.enrollment);
    let enrollment_b = fixture.enrollment_b_wire.clone();
    let reboot_task = tokio::spawn(async move { enrollment.redeem(&enrollment_b).await });
    both_operations_are_lock_blocked(&db.pool).await;
    release_advisory_lock(&mut barrier, EVIDENCE_BARRIER_KEY).await;

    assert_eq!(
        evidence_task.await.unwrap().unwrap(),
        BootstrapEvidenceResult::Established
    );
    assert!(matches!(
        reboot_task.await.unwrap().unwrap(),
        RedeemResult::Established { .. }
    ));
    context_lock.rollback().await.unwrap();
    drop(barrier);
    assert_final_boot_b(&db, &fixture).await;
    db.teardown().await;
}

#[tokio::test]
async fn reboot_b_wins_endpoint_lock_then_historical_evidence_is_rejected() {
    let db = TestDatabase::setup().await;
    let fixture = setup_race(&db, "race-reboot-first").await;
    sqlx::query(
        "CREATE FUNCTION pause_reboot_update() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN IF OLD.current_boot_nonce IS DISTINCT FROM NEW.current_boot_nonce THEN PERFORM pg_advisory_xact_lock(17002); END IF; RETURN NEW; END $$",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TRIGGER pause_reboot_update BEFORE UPDATE ON endpoints FOR EACH ROW EXECUTE FUNCTION pause_reboot_update()",
    )
    .execute(&db.pool)
    .await
    .unwrap();

    let mut barrier = hold_advisory_lock(&db.pool, REBOOT_BARRIER_KEY).await;
    let enrollment = Arc::clone(&fixture.enrollment);
    let enrollment_b = fixture.enrollment_b_wire.clone();
    let reboot_task = tokio::spawn(async move { enrollment.redeem(&enrollment_b).await });
    advisory_waiter_reached(&db.pool).await;

    let service = Arc::clone(&fixture.evidence_service);
    let evidence = fixture.evidence_a.clone();
    let endpoint_id = fixture.endpoint_id;
    let fingerprint = fixture.fingerprint;
    let evidence_task = tokio::spawn(async move {
        service
            .verify_and_establish(endpoint_id, &evidence, fingerprint)
            .await
    });
    both_operations_are_lock_blocked(&db.pool).await;
    release_advisory_lock(&mut barrier, REBOOT_BARRIER_KEY).await;

    assert!(matches!(
        reboot_task.await.unwrap().unwrap(),
        RedeemResult::Established { .. }
    ));
    assert_eq!(
        evidence_task.await.unwrap().unwrap(),
        BootstrapEvidenceResult::Rejected
    );
    drop(barrier);
    assert_final_boot_b(&db, &fixture).await;
    db.teardown().await;
}
