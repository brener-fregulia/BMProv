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
use support::TestDatabase;

async fn endpoint_for_boot(db: &TestDatabase, nonce: BootNonce, signal: &str) -> EndpointId {
    let boot = BootOrchestrationService::new(
        Arc::new(PostgresBootContextRepository::new(db.pool.clone())),
        Duration::minutes(5),
    );
    let credential = boot
        .issue_enrollment_credential(signal, nonce, Utc::now())
        .await
        .unwrap();
    let enrollment = EnrollmentService::new(
        Arc::new(PostgresEndpointRepository::new(db.pool.clone())),
        Arc::new(PostgresCredentialRedemptionRepository::new(db.pool.clone())),
    );
    let RedeemResult::Established { endpoint_id, .. } = enrollment
        .redeem(&credential.to_wire_value())
        .await
        .unwrap()
    else {
        panic!()
    };
    endpoint_id
}

async fn state(db: &TestDatabase, endpoint: EndpointId) -> TrustedBootstrapState {
    PostgresEndpointRepository::new(db.pool.clone())
        .find_by_id(endpoint)
        .await
        .unwrap()
        .unwrap()
        .current_boot
        .unwrap()
        .trusted_bootstrap()
}

#[tokio::test]
async fn verification_order_rejects_all_invalid_evidence_without_mutation_then_establishes_idempotently(
) {
    let db = TestDatabase::setup().await;
    let signer = FixtureAssertionSigner::from_seed([0x11; 32]);
    let wrong = FixtureAssertionSigner::from_seed([0x12; 32]);
    let nonce = BootNonce::from_bytes([0x21; 32]);
    let fingerprint = ServerCertFingerprint::from_sha256_digest([0x31; 32]);
    let endpoint = endpoint_for_boot(&db, nonce, "evidence-service").await;
    let service = BootstrapEvidenceService::new(
        Arc::new(PostgresEndpointRepository::new(db.pool.clone())),
        AcceptedSiteKeys::single(signer.public_key()),
    );
    let valid_assertion = signer.sign_v1(nonce, fingerprint).to_wire_value();

    let wrong_signer = BootstrapEvidenceMessage::new(
        nonce.to_wire_value(),
        wrong.sign_v1(nonce, fingerprint).to_wire_value(),
    );
    let declaration_mismatch = BootstrapEvidenceMessage::new(
        BootNonce::from_bytes([0x22; 32]).to_wire_value(),
        valid_assertion.clone(),
    );
    let historical = signer.sign_v1(BootNonce::from_bytes([0x23; 32]), fingerprint);
    let historical = BootstrapEvidenceMessage::new(
        BootNonce::from_bytes([0x23; 32]).to_wire_value(),
        historical.to_wire_value(),
    );
    let bad_fingerprint = BootstrapEvidenceMessage::new(
        nonce.to_wire_value(),
        signer
            .sign_v1(nonce, ServerCertFingerprint::from_sha256_digest([0x32; 32]))
            .to_wire_value(),
    );
    let malformed_nonce = BootstrapEvidenceMessage::new("bad", valid_assertion.clone());
    let malformed_assertion = BootstrapEvidenceMessage::new(nonce.to_wire_value(), "bad");
    let mut corrupted = valid_assertion.clone();
    let last = corrupted.pop().unwrap();
    corrupted.push(if last == 'A' { 'B' } else { 'A' });
    let corrupted = BootstrapEvidenceMessage::new(nonce.to_wire_value(), corrupted);

    for evidence in [
        &wrong_signer,
        &declaration_mismatch,
        &historical,
        &bad_fingerprint,
        &malformed_nonce,
        &malformed_assertion,
        &corrupted,
    ] {
        assert_eq!(
            service
                .verify_and_establish(endpoint, evidence, fingerprint)
                .await
                .unwrap(),
            BootstrapEvidenceResult::Rejected
        );
        assert_eq!(
            state(&db, endpoint).await,
            TrustedBootstrapState::NotEstablished
        );
    }

    let valid = BootstrapEvidenceMessage::new(nonce.to_wire_value(), valid_assertion);
    assert_eq!(
        service
            .verify_and_establish(endpoint, &valid, fingerprint)
            .await
            .unwrap(),
        BootstrapEvidenceResult::Established
    );
    assert_eq!(
        service
            .verify_and_establish(endpoint, &valid, fingerprint)
            .await
            .unwrap(),
        BootstrapEvidenceResult::Established
    );
    assert_eq!(
        state(&db, endpoint).await,
        TrustedBootstrapState::Established
    );

    let none_endpoint =
        endpoint_for_boot(&db, BootNonce::from_bytes([0x24; 32]), "evidence-none").await;
    sqlx::query("UPDATE endpoints SET current_boot_context_id = NULL, current_boot_nonce = NULL, trusted_bootstrap_state = NULL WHERE id = $1")
        .bind(none_endpoint.0).execute(&db.pool).await.unwrap();
    assert_eq!(
        service
            .verify_and_establish(none_endpoint, &valid, fingerprint)
            .await
            .unwrap(),
        BootstrapEvidenceResult::Rejected
    );
    db.teardown().await;
}
