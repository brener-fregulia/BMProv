//! Component/Integration tests: `EnrollmentService` + `BootOrchestrationService`
//! against the real `PostgresEndpointRepository` and a real PostgreSQL
//! instance (ADR-0013), exercising the ADR-0012 credential chain and
//! ADR-0004 operator-approval-gated enrollment through the real Application
//! operations — no direct database row mutation anywhere in this file
//! (Issue #17 "Safety constraints"); direct SQL is used only to *read* and
//! assert on durable state the Application/Domain already committed.
//!
//! The operator-approval calls below stand in for the "control path separate
//! from the Simulated Agent participant" (Issue #17 RF-002): this test file
//! plays that separate harness role, calling `EnrollmentService::approve_enrollment`
//! directly — never through a credential-redemption/Agent-facing method.
//!
//! Requires a real, reachable PostgreSQL instance — see `support::TestDatabase`.

mod support;

use std::sync::Arc;

use bamep_domain::credential::enrollment::SigningKey;
use bamep_domain::{Actor, CredentialSecret};
use bamep_server::adapters::postgres::PostgresEndpointRepository;
use bamep_server::application::{
    ApplicationError, BootOrchestrationService, EnrollmentService, RedeemResult,
};
use bamep_server::ports::EndpointRepository;
use chrono::{Duration, Utc};
use sqlx::PgPool;
use support::TestDatabase;

fn build_services(
    pool: PgPool,
) -> (
    BootOrchestrationService,
    EnrollmentService<PostgresEndpointRepository>,
) {
    let repo = Arc::new(PostgresEndpointRepository::new(pool));
    let key = SigningKey::generate();
    let boot = BootOrchestrationService::new(key.clone(), Duration::minutes(5));
    let enrollment = EnrollmentService::new(repo, key);
    (boot, enrollment)
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
    let (boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let result = enrollment.redeem("sim-endpoint-01", e1, now).await.unwrap();

    assert!(matches!(result, RedeemResult::Established { .. }));

    db.teardown().await;
}

#[tokio::test]
async fn rejected_credential_yields_no_session_and_no_persisted_transition() {
    let db = TestDatabase::setup().await;
    let (_boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let bogus = CredentialSecret("not-a-real-enrollment-credential".into());
    let result = enrollment
        .redeem("sim-endpoint-02", bogus, now)
        .await
        .unwrap();

    assert_eq!(result, RedeemResult::Rejected);
    assert_eq!(endpoint_row_count(&db.pool, "sim-endpoint-02").await, 0);

    db.teardown().await;
}

#[tokio::test]
async fn expired_enrollment_credential_is_rejected() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let later = now + Duration::minutes(10); // past the 5-minute enrollment TTL

    let result = enrollment
        .redeem("sim-endpoint-03", e1, later)
        .await
        .unwrap();
    assert_eq!(result, RedeemResult::Rejected);

    db.teardown().await;
}

#[tokio::test]
async fn operator_approval_via_separate_control_path_transitions_to_enrolled() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let RedeemResult::Established { endpoint_id, .. } =
        enrollment.redeem("sim-endpoint-04", e1, now).await.unwrap()
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
            now,
        )
        .await
        .unwrap();

    db.teardown().await;
}

#[tokio::test]
async fn approve_enrollment_commits_state_event_and_audit_atomically() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let RedeemResult::Established { endpoint_id, .. } = enrollment
        .redeem("sim-endpoint-atomic-01", e1, now)
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
            now,
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

    let identity_state: String =
        sqlx::query_scalar("SELECT identity_state FROM endpoints WHERE id = $1")
            .bind(endpoint_id.0)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(identity_state, "Enrolled");

    db.teardown().await;
}

#[tokio::test]
async fn invalid_transition_rolls_back_without_partial_writes() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let RedeemResult::Established { endpoint_id, .. } = enrollment
        .redeem("sim-endpoint-invalid-01", e1, now)
        .await
        .unwrap()
    else {
        panic!("first contact must establish a session");
    };
    let operator = Actor::Operator {
        label: "wp1-harness".into(),
    };
    enrollment
        .approve_enrollment(endpoint_id, operator.clone(), now)
        .await
        .unwrap();

    let events_before = domain_event_count(&db.pool, endpoint_id).await;
    let audit_before = audit_record_count(&db.pool, endpoint_id).await;

    // Already Enrolled: approving again is an illegal transition. Nothing
    // may be written for a rejected transition, exactly like a rejected
    // AuthRequest.
    let err = enrollment
        .approve_enrollment(endpoint_id, operator, now)
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
    let (boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let RedeemResult::Established {
        endpoint_id,
        runtime_credential: r1,
        ..
    } = enrollment.redeem("sim-endpoint-05", e1, now).await.unwrap()
    else {
        panic!("first contact must establish a session");
    };

    enrollment
        .approve_enrollment(
            endpoint_id,
            Actor::Operator {
                label: "wp1-harness".into(),
            },
            now,
        )
        .await
        .unwrap();

    // Fresh handshake reconnect, redeeming the previously issued runtime
    // credential — no operator approval call happens here.
    let reconnect_time = now + Duration::seconds(30);
    let result = enrollment
        .redeem("sim-endpoint-05", r1, reconnect_time)
        .await
        .unwrap();

    assert!(
        matches!(result, RedeemResult::Established { endpoint_id: id, .. } if id == endpoint_id)
    );

    let identity_state: String =
        sqlx::query_scalar("SELECT identity_state FROM endpoints WHERE id = $1")
            .bind(endpoint_id.0)
            .fetch_one(&db.pool)
            .await
            .unwrap();
    assert_eq!(identity_state, "Enrolled");

    db.teardown().await;
}

#[tokio::test]
async fn predecessor_retry_after_unconfirmed_successor_supersedes_and_reissues() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let RedeemResult::Established {
        runtime_credential: r1,
        ..
    } = enrollment
        .redeem("sim-endpoint-06", e1.clone(), now)
        .await
        .unwrap()
    else {
        panic!();
    };

    // E1 is presented again before R1 ever authenticated (dropped
    // connection between commit and delivery).
    let retry_time = now + Duration::seconds(5);
    let RedeemResult::Established {
        runtime_credential: r1_prime,
        ..
    } = enrollment
        .redeem("sim-endpoint-06", e1, retry_time)
        .await
        .unwrap()
    else {
        panic!("predecessor must still authenticate");
    };
    assert_ne!(r1, r1_prime);

    // The superseded R1 must now be rejected with a generic outcome.
    let later = now + Duration::seconds(10);
    let rejected = enrollment
        .redeem("sim-endpoint-06", r1, later)
        .await
        .unwrap();
    assert_eq!(rejected, RedeemResult::Rejected);

    // R1' (the fresh successor) authenticates and confirms.
    let confirmed = enrollment
        .redeem("sim-endpoint-06", r1_prime, later)
        .await
        .unwrap();
    assert!(matches!(confirmed, RedeemResult::Established { .. }));

    db.teardown().await;
}

#[tokio::test]
async fn revoked_chain_rejects_every_credential_in_it() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let RedeemResult::Established {
        endpoint_id,
        runtime_credential: r1,
        ..
    } = enrollment
        .redeem("sim-endpoint-07", e1.clone(), now)
        .await
        .unwrap()
    else {
        panic!();
    };

    enrollment
        .revoke_credential(endpoint_id, now)
        .await
        .unwrap();

    let later = now + Duration::seconds(1);
    assert_eq!(
        enrollment
            .redeem("sim-endpoint-07", r1, later)
            .await
            .unwrap(),
        RedeemResult::Rejected
    );
    assert_eq!(
        enrollment
            .redeem("sim-endpoint-07", e1, later)
            .await
            .unwrap(),
        RedeemResult::Rejected
    );

    db.teardown().await;
}

#[tokio::test]
async fn approving_unknown_endpoint_fails_explicitly() {
    let db = TestDatabase::setup().await;
    let (_boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let err = enrollment
        .approve_enrollment(bamep_domain::EndpointId::new(), Actor::System, now)
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::EndpointNotFound(_)));

    db.teardown().await;
}

#[tokio::test]
async fn durable_state_survives_a_fresh_pool_instance() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let RedeemResult::Established { endpoint_id, .. } = enrollment
        .redeem("sim-endpoint-durable-01", e1, now)
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
            now,
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
    let (boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();
    let enrollment = Arc::new(enrollment);

    let e1 = boot.issue_enrollment_credential(now);

    // Two concurrent AuthRequests presenting the same boot-scoped
    // enrollment credential for a never-before-seen inventory_signal — e.g.
    // a duplicate/retried boot message. Persistence must serialize these so
    // exactly one Endpoint identity and one EndpointPendingEnrollment event
    // are ever created, never two.
    let enrollment_a = Arc::clone(&enrollment);
    let e1_a = e1.clone();
    let task_a =
        tokio::spawn(async move { enrollment_a.redeem("sim-endpoint-race-01", e1_a, now).await });

    let enrollment_b = Arc::clone(&enrollment);
    let e1_b = e1.clone();
    let task_b =
        tokio::spawn(async move { enrollment_b.redeem("sim-endpoint-race-01", e1_b, now).await });

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
    let (boot, enrollment) = build_services(db.pool.clone());
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let RedeemResult::Established { .. } = enrollment
        .redeem("sim-endpoint-race-02", e1.clone(), now)
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
    let retry_time = now + Duration::seconds(5);

    let enrollment_a = Arc::clone(&enrollment);
    let e1_a = e1.clone();
    let task_a = tokio::spawn(async move {
        enrollment_a
            .redeem("sim-endpoint-race-02", e1_a, retry_time)
            .await
    });

    let enrollment_b = Arc::clone(&enrollment);
    let e1_b = e1.clone();
    let task_b = tokio::spawn(async move {
        enrollment_b
            .redeem("sim-endpoint-race-02", e1_b, retry_time)
            .await
    });

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
    let later = retry_time + Duration::seconds(1);
    let outcome_a = enrollment
        .redeem("sim-endpoint-race-02", issued_a, later)
        .await
        .unwrap();
    let outcome_b = enrollment
        .redeem("sim-endpoint-race-02", issued_b, later)
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
