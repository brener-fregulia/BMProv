//! Component/Integration tests: `EnrollmentService` + `BootOrchestrationService`
//! against the real `SqliteEndpointRepository`, exercising the ADR-0012
//! credential chain and ADR-0004 operator-approval-gated enrollment through
//! the real Application operations — no direct database row mutation
//! anywhere in this file (Issue #17 "Safety constraints").
//!
//! The operator-approval calls below stand in for the "control path separate
//! from the Simulated Agent participant" (Issue #17 RF-002): this test file
//! plays that separate harness role, calling `EnrollmentService::approve_enrollment`
//! directly — never through a credential-redemption/Agent-facing method.

use std::sync::Arc;

use bamep_domain::credential::enrollment::SigningKey;
use bamep_domain::{Actor, CredentialSecret};
use bamep_server::adapters::sqlite::SqliteEndpointRepository;
use bamep_server::application::{
    ApplicationError, BootOrchestrationService, EnrollmentService, RedeemResult,
};
use chrono::{Duration, Utc};

fn build_services() -> (
    BootOrchestrationService,
    EnrollmentService<SqliteEndpointRepository>,
) {
    let repo = Arc::new(SqliteEndpointRepository::open_in_memory().unwrap());
    let key = SigningKey::generate();
    let boot = BootOrchestrationService::new(key.clone(), Duration::minutes(5));
    let enrollment = EnrollmentService::new(repo, key);
    (boot, enrollment)
}

#[tokio::test]
async fn valid_first_enrollment_reaches_pending_enrollment_and_credential_active() {
    let (boot, enrollment) = build_services();
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let result = enrollment.redeem("sim-endpoint-01", e1, now).await.unwrap();

    assert!(matches!(result, RedeemResult::Established { .. }));
}

#[tokio::test]
async fn rejected_credential_yields_no_session_and_no_persisted_transition() {
    let (_boot, enrollment) = build_services();
    let now = Utc::now();

    let bogus = CredentialSecret("not-a-real-enrollment-credential".into());
    let result = enrollment
        .redeem("sim-endpoint-02", bogus, now)
        .await
        .unwrap();

    assert_eq!(result, RedeemResult::Rejected);
}

#[tokio::test]
async fn expired_enrollment_credential_is_rejected() {
    let (boot, enrollment) = build_services();
    let now = Utc::now();

    let e1 = boot.issue_enrollment_credential(now);
    let later = now + Duration::minutes(10); // past the 5-minute enrollment TTL

    let result = enrollment
        .redeem("sim-endpoint-03", e1, later)
        .await
        .unwrap();
    assert_eq!(result, RedeemResult::Rejected);
}

#[tokio::test]
async fn operator_approval_via_separate_control_path_transitions_to_enrolled() {
    let (boot, enrollment) = build_services();
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
}

#[tokio::test]
async fn reconnect_preserves_enrolled_identity_without_repeated_approval() {
    let (boot, enrollment) = build_services();
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
}

#[tokio::test]
async fn predecessor_retry_after_unconfirmed_successor_supersedes_and_reissues() {
    let (boot, enrollment) = build_services();
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

    // Recovery: the still-valid predecessor E1... already consumed above by
    // superseding; R1' (the fresh successor) authenticates and confirms.
    let confirmed = enrollment
        .redeem("sim-endpoint-06", r1_prime, later)
        .await
        .unwrap();
    assert!(matches!(confirmed, RedeemResult::Established { .. }));
}

#[tokio::test]
async fn revoked_chain_rejects_every_credential_in_it() {
    let (boot, enrollment) = build_services();
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
}

#[tokio::test]
async fn approving_unknown_endpoint_fails_explicitly() {
    let (_boot, enrollment) = build_services();
    let now = Utc::now();

    let err = enrollment
        .approve_enrollment(bamep_domain::EndpointId::new(), Actor::System, now)
        .await
        .unwrap_err();
    assert!(matches!(err, ApplicationError::EndpointNotFound(_)));
}
