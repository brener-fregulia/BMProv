//! Component/Integration tests: `EnrollmentService` + `BootOrchestrationService`
//! against the real `PostgresEndpointRepository` and a real PostgreSQL
//! instance (ADR-0013), exercising the ADR-0012 credential chain and
//! ADR-0004 operator-approval-gated enrollment through the real Application
//! operations — no direct database row mutation anywhere in this file
//! (Issue #17 "Safety constraints"); direct SQL is used only to *read* and
//! assert on durable state the Application/Domain already committed, and —
//! in exactly one test — to install a test-local trigger that forces a
//! deterministic mid-transaction failure inside this disposable database.
//!
//! The operator-approval calls below stand in for the "control path separate
//! from the Simulated Agent participant" (Issue #17 RF-002): this test file
//! plays that separate harness role, calling `EnrollmentService::approve_enrollment`
//! directly — never through a credential-redemption/Agent-facing method.
//!
//! Every test uses a `ManualClock` (`support::ManualClock`) instead of real
//! wall-clock sleeps, so `EnrollmentService::redeem`'s decision-time clock
//! read (see `application::Clock`) stays under precise, deterministic test
//! control — including while a concurrent request is genuinely blocked on a
//! PostgreSQL lock.
//!
//! Requires a real, reachable PostgreSQL instance — see `support::TestDatabase`.

mod support;

use std::sync::Arc;

use bamep_domain::credential::enrollment::SigningKey;
use bamep_domain::{Actor, CredentialSecret};
use bamep_server::adapters::postgres::PostgresEndpointRepository;
use bamep_server::application::{
    ApplicationError, BootOrchestrationService, Clock, EnrollmentService, RedeemResult,
};
use bamep_server::ports::EndpointRepository;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use support::{ManualClock, TestDatabase};

fn build_services(
    pool: PgPool,
) -> (
    BootOrchestrationService,
    EnrollmentService<PostgresEndpointRepository>,
    Arc<ManualClock>,
) {
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let repo = Arc::new(PostgresEndpointRepository::new(pool));
    let key = SigningKey::generate();
    let boot = BootOrchestrationService::new(key.clone(), Duration::minutes(5));
    let enrollment = EnrollmentService::with_clock(repo, key, Arc::clone(&clock) as Arc<dyn Clock>);
    (boot, enrollment, clock)
}

async fn domain_event_count(pool: &PgPool, endpoint_id: bamep_domain::EndpointId) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM domain_events WHERE endpoint_id = $1")
        .bind(endpoint_id.0)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn audit_record_count(pool: &PgPool, endpoint_id: bamep_domain::EndpointId) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM audit_records WHERE endpoint_id = $1")
        .bind(endpoint_id.0)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn endpoint_row_count(pool: &PgPool, inventory_signal: &str) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM endpoints WHERE inventory_signal = $1")
        .bind(inventory_signal)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn identity_state(pool: &PgPool, endpoint_id: bamep_domain::EndpointId) -> String {
    sqlx::query_scalar("SELECT identity_state FROM endpoints WHERE id = $1")
        .bind(endpoint_id.0)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn migrations_apply_cleanly_to_a_fresh_database() {
    let db = TestDatabase::setup().await;

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT table_name FROM information_schema.tables \
         WHERE table_schema = 'public' AND table_name NOT LIKE '\\_sqlx%' \
         ORDER BY table_name",
    )
    .fetch_all(&db.pool)
    .await
    .unwrap();

    assert_eq!(
        tables,
        vec![
            "audit_records",
            "domain_events",
            "endpoint_credentials",
            "endpoints",
        ]
    );

    db.teardown().await;
}

#[tokio::test]
async fn valid_first_enrollment_reaches_pending_enrollment_and_credential_active() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let result = enrollment.redeem("sim-endpoint-01", e1).await.unwrap();

    assert!(matches!(result, RedeemResult::Established { .. }));

    db.teardown().await;
}

#[tokio::test]
async fn rejected_credential_yields_no_session_and_no_persisted_transition() {
    let db = TestDatabase::setup().await;
    let (_boot, enrollment, _clock) = build_services(db.pool.clone());

    let bogus = CredentialSecret("not-a-real-enrollment-credential".into());
    let result = enrollment.redeem("sim-endpoint-02", bogus).await.unwrap();

    assert_eq!(result, RedeemResult::Rejected);
    assert_eq!(endpoint_row_count(&db.pool, "sim-endpoint-02").await, 0);

    db.teardown().await;
}

#[tokio::test]
async fn expired_enrollment_credential_is_rejected() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    clock.advance(Duration::minutes(10)); // past the 5-minute enrollment TTL

    let result = enrollment.redeem("sim-endpoint-03", e1).await.unwrap();
    assert_eq!(result, RedeemResult::Rejected);

    db.teardown().await;
}

#[tokio::test]
async fn operator_approval_via_separate_control_path_transitions_to_enrolled() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established { endpoint_id, .. } =
        enrollment.redeem("sim-endpoint-04", e1).await.unwrap()
    else {
        panic!("first contact must establish a session");
    };

    // Approval originates from this harness, not from the Agent-facing
    // `redeem` method — a structurally separate call.
    enrollment
        .approve_enrollment(
            endpoint_id,
            Actor::Operator {
                label: "wp1-harness".into(),
            },
            clock.now(),
        )
        .await
        .unwrap();

    db.teardown().await;
}

#[tokio::test]
async fn approve_enrollment_commits_state_event_and_audit_atomically() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established { endpoint_id, .. } = enrollment
        .redeem("sim-endpoint-atomic-01", e1)
        .await
        .unwrap()
    else {
        panic!("first contact must establish a session");
    };

    assert_eq!(
        domain_event_count(&db.pool, endpoint_id).await,
        1,
        "first contact emits exactly EndpointPendingEnrollment"
    );
    assert_eq!(audit_record_count(&db.pool, endpoint_id).await, 0);

    enrollment
        .approve_enrollment(
            endpoint_id,
            Actor::Operator {
                label: "wp1-harness".into(),
            },
            clock.now(),
        )
        .await
        .unwrap();

    assert_eq!(
        domain_event_count(&db.pool, endpoint_id).await,
        3,
        "approval commits EndpointEnrolled + OperatorDecisionRecorded alongside the earlier event"
    );
    assert_eq!(
        audit_record_count(&db.pool, endpoint_id).await,
        1,
        "approval commits exactly one immutable audit record in the same transaction"
    );

    assert_eq!(identity_state(&db.pool, endpoint_id).await, "Enrolled");

    db.teardown().await;
}

#[tokio::test]
async fn invalid_transition_rolls_back_without_partial_writes() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established { endpoint_id, .. } = enrollment
        .redeem("sim-endpoint-invalid-01", e1)
        .await
        .unwrap()
    else {
        panic!("first contact must establish a session");
    };
    let operator = Actor::Operator {
        label: "wp1-harness".into(),
    };
    enrollment
        .approve_enrollment(endpoint_id, operator.clone(), clock.now())
        .await
        .unwrap();

    let events_before = domain_event_count(&db.pool, endpoint_id).await;
    let audit_before = audit_record_count(&db.pool, endpoint_id).await;

    // Already Enrolled: approving again is an illegal transition. Nothing
    // may be written for a rejected transition, exactly like a rejected
    // AuthRequest.
    let err = enrollment
        .approve_enrollment(endpoint_id, operator, clock.now())
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::InvalidTransition(_)));

    assert_eq!(
        domain_event_count(&db.pool, endpoint_id).await,
        events_before
    );
    assert_eq!(
        audit_record_count(&db.pool, endpoint_id).await,
        audit_before
    );

    db.teardown().await;
}

#[tokio::test]
async fn reconnect_preserves_enrolled_identity_without_repeated_approval() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established {
        endpoint_id,
        runtime_credential: r1,
        ..
    } = enrollment.redeem("sim-endpoint-05", e1).await.unwrap()
    else {
        panic!("first contact must establish a session");
    };

    enrollment
        .approve_enrollment(
            endpoint_id,
            Actor::Operator {
                label: "wp1-harness".into(),
            },
            clock.now(),
        )
        .await
        .unwrap();

    // Fresh handshake reconnect, redeeming the previously issued runtime
    // credential — no operator approval call happens here.
    clock.advance(Duration::seconds(30));
    let result = enrollment.redeem("sim-endpoint-05", r1).await.unwrap();

    assert!(
        matches!(result, RedeemResult::Established { endpoint_id: id, .. } if id == endpoint_id)
    );
    assert_eq!(identity_state(&db.pool, endpoint_id).await, "Enrolled");

    db.teardown().await;
}

#[tokio::test]
async fn predecessor_retry_after_unconfirmed_successor_supersedes_and_reissues() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established {
        runtime_credential: r1,
        ..
    } = enrollment
        .redeem("sim-endpoint-06", e1.clone())
        .await
        .unwrap()
    else {
        panic!();
    };

    // E1 is presented again before R1 ever authenticated (dropped
    // connection between commit and delivery).
    clock.advance(Duration::seconds(5));
    let RedeemResult::Established {
        runtime_credential: r1_prime,
        ..
    } = enrollment.redeem("sim-endpoint-06", e1).await.unwrap()
    else {
        panic!("predecessor must still authenticate");
    };
    assert_ne!(r1, r1_prime);

    // The superseded R1 must now be rejected with a generic outcome.
    clock.advance(Duration::seconds(5));
    let rejected = enrollment.redeem("sim-endpoint-06", r1).await.unwrap();
    assert_eq!(rejected, RedeemResult::Rejected);

    // R1' (the fresh successor) authenticates and confirms.
    let confirmed = enrollment
        .redeem("sim-endpoint-06", r1_prime)
        .await
        .unwrap();
    assert!(matches!(confirmed, RedeemResult::Established { .. }));

    db.teardown().await;
}

#[tokio::test]
async fn revoked_chain_rejects_every_credential_in_it() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established {
        endpoint_id,
        runtime_credential: r1,
        ..
    } = enrollment
        .redeem("sim-endpoint-07", e1.clone())
        .await
        .unwrap()
    else {
        panic!();
    };

    enrollment
        .revoke_credential(endpoint_id, clock.now())
        .await
        .unwrap();

    clock.advance(Duration::seconds(1));
    assert_eq!(
        enrollment.redeem("sim-endpoint-07", r1).await.unwrap(),
        RedeemResult::Rejected
    );
    // Even the original, still within its own validity window, enrollment
    // credential must not resurrect a revoked chain (see also
    // `genuine_reboot_is_not_attempted_against_a_revoked_chain` below,
    // which exercises this with a *fresh* E2 rather than a retried E1).
    assert_eq!(
        enrollment.redeem("sim-endpoint-07", e1).await.unwrap(),
        RedeemResult::Rejected
    );

    db.teardown().await;
}

#[tokio::test]
async fn approving_unknown_endpoint_fails_explicitly() {
    let db = TestDatabase::setup().await;
    let (_boot, enrollment, clock) = build_services(db.pool.clone());

    let err = enrollment
        .approve_enrollment(bamep_domain::EndpointId::new(), Actor::System, clock.now())
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::EndpointNotFound(_)));

    db.teardown().await;
}

#[tokio::test]
async fn durable_state_survives_a_fresh_pool_instance() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established { endpoint_id, .. } = enrollment
        .redeem("sim-endpoint-durable-01", e1)
        .await
        .unwrap()
    else {
        panic!("first contact must establish a session");
    };
    enrollment
        .approve_enrollment(
            endpoint_id,
            Actor::Operator {
                label: "wp1-harness".into(),
            },
            clock.now(),
        )
        .await
        .unwrap();

    // A brand-new pool/repository/Application stack against the same
    // database — a different in-process instance than the one that
    // committed the above, so this proves durable persistence rather than
    // an in-process cache.
    let fresh_pool = PgPool::connect(&db.db_url)
        .await
        .expect("reconnect to the same durable database");
    let fresh_repo = Arc::new(PostgresEndpointRepository::new(fresh_pool));
    let fetched = fresh_repo
        .find_by_id(endpoint_id)
        .await
        .unwrap()
        .expect("state committed by the original pool must be visible to a fresh one");
    assert_eq!(fetched.inventory_signal, "sim-endpoint-durable-01");
    assert_eq!(fetched.identity, bamep_domain::IdentityState::Enrolled);

    db.teardown().await;
}

#[tokio::test]
async fn concurrent_first_contact_does_not_create_duplicate_identity() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());
    let enrollment = Arc::new(enrollment);

    let e1 = boot.issue_enrollment_credential(clock.now());

    // Two concurrent AuthRequests presenting the same boot-scoped
    // enrollment credential for a never-before-seen inventory_signal — e.g.
    // a duplicate/retried boot message. Persistence must serialize these so
    // exactly one Endpoint identity and one EndpointPendingEnrollment event
    // are ever created, never two.
    let enrollment_a = Arc::clone(&enrollment);
    let e1_a = e1.clone();
    let task_a =
        tokio::spawn(async move { enrollment_a.redeem("sim-endpoint-race-01", e1_a).await });

    let enrollment_b = Arc::clone(&enrollment);
    let e1_b = e1.clone();
    let task_b =
        tokio::spawn(async move { enrollment_b.redeem("sim-endpoint-race-01", e1_b).await });

    let (result_a, result_b) = tokio::join!(task_a, task_b);
    let result_a = result_a.unwrap().unwrap();
    let result_b = result_b.unwrap().unwrap();

    let RedeemResult::Established {
        endpoint_id: id_a, ..
    } = result_a
    else {
        panic!("A must authenticate: E1 is a valid enrollment credential")
    };
    let RedeemResult::Established {
        endpoint_id: id_b, ..
    } = result_b
    else {
        panic!("B must authenticate: E1 is a valid enrollment credential")
    };
    assert_eq!(
        id_a, id_b,
        "both racing requests must resolve to the same Endpoint identity"
    );

    assert_eq!(
        endpoint_row_count(&db.pool, "sim-endpoint-race-01").await,
        1,
        "exactly one Endpoint row must exist for this inventory_signal"
    );
    assert_eq!(
        domain_event_count(&db.pool, id_a).await,
        1,
        "exactly one EndpointPendingEnrollment event must be committed, never two"
    );

    db.teardown().await;
}

#[tokio::test]
async fn concurrent_same_predecessor_redemption_only_last_commit_wins() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established { .. } = enrollment
        .redeem("sim-endpoint-race-02", e1.clone())
        .await
        .unwrap()
    else {
        panic!();
    };

    // E1's successor R1 was never confirmed — E1 is still a valid
    // predecessor. Two concurrent AuthRequests both retry E1 (two racing
    // reconnect attempts). ADR-0012 point 7: both must individually
    // authenticate, but only the last *committed* successor may remain
    // valid afterward — a stale-read successor must never coexist with it.
    let enrollment = Arc::new(enrollment);
    clock.advance(Duration::seconds(5));

    let enrollment_a = Arc::clone(&enrollment);
    let e1_a = e1.clone();
    let task_a =
        tokio::spawn(async move { enrollment_a.redeem("sim-endpoint-race-02", e1_a).await });

    let enrollment_b = Arc::clone(&enrollment);
    let e1_b = e1.clone();
    let task_b =
        tokio::spawn(async move { enrollment_b.redeem("sim-endpoint-race-02", e1_b).await });

    let (result_a, result_b) = tokio::join!(task_a, task_b);
    let result_a = result_a.unwrap().unwrap();
    let result_b = result_b.unwrap().unwrap();

    let RedeemResult::Established {
        runtime_credential: issued_a,
        ..
    } = result_a
    else {
        panic!("A must authenticate: E1 is still a valid predecessor")
    };
    let RedeemResult::Established {
        runtime_credential: issued_b,
        ..
    } = result_b
    else {
        panic!("B must authenticate: E1 is still a valid predecessor")
    };
    assert_ne!(
        issued_a, issued_b,
        "each concurrent redemption must mint its own fresh successor"
    );

    // Exactly one of the two racing successors may still be valid — a
    // credential invalidated by the other's later commit must not mint a
    // further successor from stale state.
    clock.advance(Duration::seconds(1));
    let outcome_a = enrollment
        .redeem("sim-endpoint-race-02", issued_a)
        .await
        .unwrap();
    let outcome_b = enrollment
        .redeem("sim-endpoint-race-02", issued_b)
        .await
        .unwrap();
    let accepted = [&outcome_a, &outcome_b]
        .into_iter()
        .filter(|r| matches!(r, RedeemResult::Established { .. }))
        .count();
    assert_eq!(
        accepted, 1,
        "exactly one of the two racing successors must remain valid — never both, never neither"
    );

    db.teardown().await;
}

/// Finding 1 (commit-time credential validity): proves the timing-boundary
/// fix, not just ordinary expiry. A confirmed runtime predecessor (R1 — an
/// opaque, non-enrollment-token value, so it cannot be rescued by the
/// genuine-reboot fallback) is retried while a *separate* transaction holds
/// the exact PostgreSQL advisory lock `EndpointRepository::redeem` itself
/// takes for this `inventory_signal`. Only after the concurrent `redeem`
/// call is provably blocked on that lock does the test advance the shared
/// `ManualClock` past R1's expiry and release the lock. If `redeem` used a
/// timestamp captured before the lock wait (the pre-fix bug), it would
/// still see R1 as valid and incorrectly accept it; with the fix, the
/// decision closure reads the clock only once it actually runs — after the
/// lock — and correctly rejects.
#[tokio::test]
async fn commit_time_credential_validity_is_evaluated_after_lock_acquisition() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());
    let enrollment = Arc::new(enrollment.with_credential_ttl(Duration::milliseconds(300)));

    let signal = "sim-endpoint-clock-race-01";
    let t0 = clock.now();
    let e1 = boot.issue_enrollment_credential(t0);
    let RedeemResult::Established {
        runtime_credential: r1,
        ..
    } = enrollment.redeem(signal, e1).await.unwrap()
    else {
        panic!("first contact must establish a session");
    };

    // Confirm R1: it becomes the predecessor. Confirmation does not refresh
    // its expiry — it keeps the 300ms window from t0.
    clock.set(t0 + Duration::milliseconds(10));
    let RedeemResult::Established { .. } = enrollment.redeem(signal, r1.clone()).await.unwrap()
    else {
        panic!("R1 must confirm")
    };

    // Hold the exact advisory lock the Adapter itself takes for this
    // inventory_signal, in a separate transaction, so a concurrent
    // `redeem` retrying R1 (still valid at this instant) is forced to
    // block on it.
    let mut lock_tx = db.pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(signal)
        .execute(&mut *lock_tx)
        .await
        .unwrap();

    let enrollment_task = Arc::clone(&enrollment);
    let r1_task = r1.clone();
    let redeem_task = tokio::spawn(async move { enrollment_task.redeem(signal, r1_task).await });

    // Best-effort only: gives the spawned task a chance to actually reach
    // and block on the lock before we act. Not load-bearing for
    // correctness — the task structurally cannot proceed past the lock
    // until `lock_tx` below is rolled back, regardless of scheduling.
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // While the concurrent request is blocked waiting for our lock,
    // advance the clock past R1's expiry (t0 + 300ms).
    clock.set(t0 + Duration::milliseconds(400));

    lock_tx.rollback().await.unwrap(); // release the lock; no writes made

    let result = redeem_task.await.unwrap().unwrap();

    assert_eq!(
        result,
        RedeemResult::Rejected,
        "a predecessor that expired while waiting for the commit-time lock must be \
         rejected — the decision must never use a timestamp captured before the lock"
    );

    db.teardown().await;
}

/// Finding 2 (genuine reboot): a fresh, valid boot-scoped enrollment
/// credential (E2) re-establishes a brand-new runtime-credential chain for
/// an already-known, `Enrolled` Endpoint (ADR-0012 point 1) — without
/// re-running operator approval and without any new domain event/audit
/// record, since this is a credential-dimension transition only.
#[tokio::test]
async fn genuine_reboot_reestablishes_chain_for_known_enrolled_endpoint_without_reapproval() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established {
        endpoint_id,
        runtime_credential: r1,
        ..
    } = enrollment
        .redeem("sim-endpoint-reboot-01", e1)
        .await
        .unwrap()
    else {
        panic!("first contact must establish a session");
    };

    enrollment
        .approve_enrollment(
            endpoint_id,
            Actor::Operator {
                label: "wp1-harness".into(),
            },
            clock.now(),
        )
        .await
        .unwrap();

    let events_before = domain_event_count(&db.pool, endpoint_id).await;
    let audit_before = audit_record_count(&db.pool, endpoint_id).await;

    // Genuine reboot: the Agent restarts, re-engages the Boot Orchestrator,
    // and presents a fresh enrollment credential (E2) rather than anything
    // from its old runtime chain — long after that chain's TTL, though the
    // TTL elapsing is incidental here, not what triggers this path.
    clock.advance(Duration::hours(2));
    let e2 = boot.issue_enrollment_credential(clock.now());
    let result = enrollment
        .redeem("sim-endpoint-reboot-01", e2)
        .await
        .unwrap();

    let RedeemResult::Established {
        endpoint_id: id_after_reboot,
        runtime_credential: r_after_reboot,
        ..
    } = result
    else {
        panic!(
            "a fresh, valid enrollment credential must re-establish a chain for a known Endpoint"
        )
    };
    assert_eq!(
        id_after_reboot, endpoint_id,
        "Endpoint identity must be preserved across a genuine reboot"
    );
    assert_ne!(
        r_after_reboot, r1,
        "the post-reboot chain must not reuse any pre-reboot runtime credential"
    );

    // No re-approval: identity stays Enrolled, and no new event/audit was
    // committed for this purely credential-dimension transition.
    assert_eq!(identity_state(&db.pool, endpoint_id).await, "Enrolled");
    assert_eq!(
        domain_event_count(&db.pool, endpoint_id).await,
        events_before
    );
    assert_eq!(
        audit_record_count(&db.pool, endpoint_id).await,
        audit_before
    );

    // The pre-reboot runtime credential is no longer valid — the old chain
    // was fully superseded, not merged.
    let stale_r1_result = enrollment
        .redeem("sim-endpoint-reboot-01", r1)
        .await
        .unwrap();
    assert_eq!(stale_r1_result, RedeemResult::Rejected);

    db.teardown().await;
}

#[tokio::test]
async fn genuine_reboot_does_not_bypass_authentication_with_invalid_credential() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established { .. } = enrollment
        .redeem("sim-endpoint-reboot-02", e1)
        .await
        .unwrap()
    else {
        panic!();
    };

    // Neither a match against the current chain nor a legitimate,
    // correctly-signed enrollment credential — must be rejected outright,
    // never treated as a "genuine reboot."
    let garbage = CredentialSecret("not-a-real-credential".into());
    let result = enrollment
        .redeem("sim-endpoint-reboot-02", garbage)
        .await
        .unwrap();
    assert_eq!(result, RedeemResult::Rejected);

    db.teardown().await;
}

/// Finding 2's explicitly reported open question, exercised rather than
/// silently decided: whether an explicit `CredentialRevoked` should survive
/// a genuine reboot is not resolved by any current ADR/Specification. This
/// test documents the deliberately preserved pre-existing behavior — a
/// revoked chain stays rejected even against a brand-new, independently
/// valid enrollment credential — until the owner decides otherwise.
#[tokio::test]
async fn genuine_reboot_is_not_attempted_against_a_revoked_chain() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established { endpoint_id, .. } = enrollment
        .redeem("sim-endpoint-reboot-03", e1)
        .await
        .unwrap()
    else {
        panic!();
    };

    enrollment
        .revoke_credential(endpoint_id, clock.now())
        .await
        .unwrap();

    // A brand-new, independently valid enrollment credential — not a
    // retried/stale value from before the revocation.
    clock.advance(Duration::seconds(1));
    let e2 = boot.issue_enrollment_credential(clock.now());
    let result = enrollment
        .redeem("sim-endpoint-reboot-03", e2)
        .await
        .unwrap();

    assert_eq!(
        result,
        RedeemResult::Rejected,
        "a revoked chain must not be silently re-established by a genuine reboot \
         until this policy question is explicitly resolved"
    );

    db.teardown().await;
}

/// Finding 3: proves whole-transaction rollback under a genuine
/// mid-transaction PostgreSQL failure — not merely a rejected Domain
/// transition (see `invalid_transition_rolls_back_without_partial_writes`
/// above, which only proves the latter). A test-local trigger, installed
/// only in this disposable per-test database and dropped with it at
/// teardown, forces the `audit_records` insert — the *last* statement
/// `approve_enrollment`'s transaction issues, after the `endpoints` update,
/// the `endpoint_credentials` update, and both `domain_events` inserts have
/// already been sent — to fail deterministically.
#[tokio::test]
async fn transaction_rolls_back_entirely_when_a_later_statement_fails() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment, clock) = build_services(db.pool.clone());

    let e1 = boot.issue_enrollment_credential(clock.now());
    let RedeemResult::Established { endpoint_id, .. } = enrollment
        .redeem("sim-endpoint-rollback-01", e1)
        .await
        .unwrap()
    else {
        panic!("first contact must establish a session");
    };

    assert_eq!(
        identity_state(&db.pool, endpoint_id).await,
        "PendingEnrollment"
    );
    let events_before = domain_event_count(&db.pool, endpoint_id).await;
    assert_eq!(events_before, 1);

    sqlx::query(
        "CREATE FUNCTION fail_audit_insert() RETURNS trigger AS $$
         BEGIN
             RAISE EXCEPTION 'test-induced failure: audit insert must never become durable';
         END;
         $$ LANGUAGE plpgsql",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TRIGGER fail_audit_insert_trigger \
         BEFORE INSERT ON audit_records \
         FOR EACH ROW EXECUTE FUNCTION fail_audit_insert()",
    )
    .execute(&db.pool)
    .await
    .unwrap();

    let err = enrollment
        .approve_enrollment(
            endpoint_id,
            Actor::Operator {
                label: "wp1-harness".into(),
            },
            clock.now(),
        )
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::Repository(_)));

    // Nothing from the failed transaction became durable — not the state
    // change, not the two events inserted earlier in the same transaction,
    // not the audit record.
    assert_eq!(
        identity_state(&db.pool, endpoint_id).await,
        "PendingEnrollment",
        "the state change must not survive rollback of a transaction that failed later"
    );
    assert_eq!(
        domain_event_count(&db.pool, endpoint_id).await,
        events_before,
        "events inserted earlier in the same failed transaction must not survive its rollback"
    );
    assert_eq!(audit_record_count(&db.pool, endpoint_id).await, 0);

    db.teardown().await;
}
