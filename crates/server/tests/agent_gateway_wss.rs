//! Real loopback TCP -> pinned TLS 1.3 -> WebSocket -> Agent Protocol
//! integration for the Issue #17 WP1 handshake checkpoint: composes the real
//! WSS transport (`bamep_server::adapters::agent_transport`), the real
//! `AgentControlGateway`, and the real PostgreSQL-backed `EnrollmentService`
//! end to end, using the real Simulator-side pinned-WSS connect and
//! handshake helpers (`bamep_simulator`).
//!
//! Narrower Gateway-semantics assertions (decode/phase validation,
//! correlation, generic `AuthError` policy) are already covered by
//! `tests/agent_gateway.rs` and are not duplicated here — this file proves
//! only composition: WSS + Gateway + Application redemption + wire response,
//! and the version-no-redeem guarantee crossing the real WSS boundary at
//! least once, per Issue #17's Simulator validation requirements.

mod support;

use std::net::SocketAddr;
use std::sync::Arc;

use bamep_agent_protocol::{
    decode, encode, AgentProtocolMessage, AuthRequestMessage, BootstrapEvidenceMessage,
    ProtocolVersion,
};
use bamep_domain::presented_credential::{CredentialKind, PresentedCredential};
use bamep_domain::{Actor, BootNonce, EndpointId};
use bamep_server::adapters::agent_gateway::{AgentControlGateway, HandshakeOutcome};
use bamep_server::adapters::agent_transport::AgentTransportAcceptor;
use bamep_server::adapters::postgres::{
    PostgresBootContextRepository, PostgresCredentialRedemptionRepository,
    PostgresEndpointRepository,
};
use bamep_server::application::{
    BootOrchestrationService, BootstrapEvidenceService, EnrollmentService,
};
use bamep_simulator::{
    authenticate, connect_after_trusted_bootstrap, connect_pinned_wss, send_bootstrap_evidence,
    ServerCertFingerprint, SimulatedBootstrapMaterial, SimulatedPairedTrust,
    SimulatorHandshakeOutcome, TrustedBootstrapFixtureIssuer,
};
use bamep_trusted_bootstrap::AcceptedSiteKeys;
use chrono::{Duration, Utc};
use futures_util::{SinkExt, StreamExt};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use sqlx::PgPool;
use support::TestDatabase;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

type Enrollment =
    EnrollmentService<PostgresEndpointRepository, PostgresCredentialRedemptionRepository>;
type Gateway =
    AgentControlGateway<PostgresEndpointRepository, PostgresCredentialRedemptionRepository>;
type BootOrchestration = BootOrchestrationService<PostgresBootContextRepository>;

fn generate_test_cert(subject_alt_name: &str) -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec![subject_alt_name.to_string()]).expect("cert generation");
    let cert_der = cert.der().clone();
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));
    (cert_der, key_der)
}

fn build_services(pool: PgPool) -> (BootOrchestration, Arc<Enrollment>) {
    let boot_repo = Arc::new(PostgresBootContextRepository::new(pool.clone()));
    let endpoint_repo = Arc::new(PostgresEndpointRepository::new(pool.clone()));
    let redemption_repo = Arc::new(PostgresCredentialRedemptionRepository::new(pool));
    let boot_orchestration = BootOrchestrationService::new(boot_repo, Duration::minutes(5));
    let enrollment = Arc::new(EnrollmentService::new(endpoint_repo, redemption_repo));
    (boot_orchestration, enrollment)
}

async fn issue_e1(boot: &BootOrchestration, signal: &str) -> PresentedCredential {
    // This file proves WSS/Gateway composition, not current-boot persistence
    // (see `enrollment_lifecycle.rs` for that) — a fresh, discarded
    // BootNonce is sufficient here.
    let boot_nonce =
        bamep_domain::BootNonce::generate().expect("OS CSPRNG must be available in tests");
    boot.issue_enrollment_credential(signal, boot_nonce, Utc::now())
        .await
        .expect("issuance must succeed")
}

#[tokio::test]
async fn valid_auth_request_over_real_wss_reaches_session_established() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let issuer = TrustedBootstrapFixtureIssuer::from_seed([0x31; 32]);
    let evidence_service = Arc::new(BootstrapEvidenceService::new(
        Arc::new(PostgresEndpointRepository::new(db.pool.clone())),
        AcceptedSiteKeys::single(issuer.public_key()),
    ));
    let gateway = Arc::new(
        Gateway::new(Arc::clone(&enrollment)).with_bootstrap_evidence_service(evidence_service),
    );

    let (cert_der, key_der) = generate_test_cert("localhost");
    let expected_fingerprint = ServerCertFingerprint::from_leaf_der(cert_der.as_ref());
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("local addr");
    let acceptor = AgentTransportAcceptor::new(vec![cert_der], key_der).expect("build acceptor");

    let server_gateway = Arc::clone(&gateway);
    let server_task = tokio::spawn(async move {
        let (tcp_stream, _peer) = listener.accept().await.expect("accept tcp");
        let mut connection = acceptor.accept(tcp_stream).await.expect("tls+ws accept");
        let outcome = server_gateway.handshake(&mut connection.websocket).await?;
        if let HandshakeOutcome::Established(session) = &outcome {
            server_gateway
                .run_authenticated_session(
                    &mut connection.websocket,
                    *session,
                    connection.server_fingerprint,
                )
                .await?;
        }
        Ok::<_, bamep_server::adapters::agent_gateway::AgentGatewayError>(outcome)
    });

    let nonce = bamep_domain::BootNonce::from_bytes([0x51; 32]);
    let assertion = issuer.issue(nonce, expected_fingerprint);
    let material = SimulatedBootstrapMaterial::from_assertion(&assertion);
    let paired = SimulatedPairedTrust::single(issuer.public_key());
    let mut connection =
        connect_after_trusted_bootstrap(addr, "localhost", &paired, nonce, &material)
            .await
            .expect("local trust then pinned WSS succeeds");
    let e1 = boot
        .issue_enrollment_credential("wss-established-01", nonce, Utc::now())
        .await
        .unwrap();
    let outcome = authenticate(&mut connection.websocket, &e1.to_wire_value())
        .await
        .expect("Simulator handshake helper must not error on a valid AuthRequest");

    let SimulatorHandshakeOutcome::Established(established) = outcome else {
        panic!("expected Established, got {outcome:?}");
    };
    assert!(!established.body.runtime_credential.is_empty());
    let before_evidence: (String, String) = sqlx::query_as(
        "SELECT identity_state::text, trusted_bootstrap_state::text FROM endpoints WHERE inventory_signal = $1",
    )
    .bind("wss-established-01")
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        before_evidence,
        ("PendingEnrollment".into(), "NotEstablished".into()),
        "SessionEstablished must precede and remain independent from BootstrapEvidence"
    );
    send_bootstrap_evidence(&mut connection.websocket, &connection.established)
        .await
        .unwrap();
    connection.websocket.close(None).await.unwrap();

    let server_outcome = server_task
        .await
        .expect("server task must not panic")
        .expect("server-side Gateway handshake must succeed");
    assert!(matches!(server_outcome, HandshakeOutcome::Established(_)));
    let row: (String, String) = sqlx::query_as(
        "SELECT identity_state::text, trusted_bootstrap_state::text FROM endpoints WHERE inventory_signal = $1",
    )
    .bind("wss-established-01")
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(row, ("PendingEnrollment".into(), "Established".into()));

    let endpoint_id: uuid::Uuid =
        sqlx::query_scalar("SELECT id FROM endpoints WHERE inventory_signal = $1")
            .bind("wss-established-01")
            .fetch_one(&db.pool)
            .await
            .unwrap();
    enrollment
        .approve_enrollment(
            EndpointId(endpoint_id),
            Actor::Operator {
                label: "wp1-wss-harness".into(),
            },
            Utc::now(),
        )
        .await
        .unwrap();
    let final_row: (String, String) = sqlx::query_as(
        "SELECT identity_state::text, trusted_bootstrap_state::text FROM endpoints WHERE id = $1",
    )
    .bind(endpoint_id)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(final_row, ("Enrolled".into(), "Established".into()));

    db.teardown().await;
}

#[tokio::test]
async fn authenticated_session_closed_without_evidence_remains_not_established() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let issuer = TrustedBootstrapFixtureIssuer::from_seed([0x32; 32]);
    let evidence_service = Arc::new(BootstrapEvidenceService::new(
        Arc::new(PostgresEndpointRepository::new(db.pool.clone())),
        AcceptedSiteKeys::single(issuer.public_key()),
    ));
    let gateway =
        Arc::new(Gateway::new(enrollment).with_bootstrap_evidence_service(evidence_service));
    let (cert_der, key_der) = generate_test_cert("localhost");
    let fingerprint = ServerCertFingerprint::from_leaf_der(cert_der.as_ref());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let acceptor = AgentTransportAcceptor::new(vec![cert_der], key_der).unwrap();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut connection = acceptor.accept(tcp).await.unwrap();
        let HandshakeOutcome::Established(session) =
            gateway.handshake(&mut connection.websocket).await?
        else {
            panic!("authentication must establish")
        };
        gateway
            .run_authenticated_session(
                &mut connection.websocket,
                session,
                connection.server_fingerprint,
            )
            .await
    });
    let nonce = BootNonce::from_bytes([0x52; 32]);
    let credential = boot
        .issue_enrollment_credential("wss-missing-evidence", nonce, Utc::now())
        .await
        .unwrap();
    let mut websocket = connect_pinned_wss(addr, "localhost", fingerprint)
        .await
        .unwrap();
    assert!(matches!(
        authenticate(&mut websocket, &credential.to_wire_value())
            .await
            .unwrap(),
        SimulatorHandshakeOutcome::Established(_)
    ));
    websocket.close(None).await.unwrap();
    server.await.unwrap().unwrap();
    let state: String = sqlx::query_scalar(
        "SELECT trusted_bootstrap_state::text FROM endpoints WHERE inventory_signal = $1",
    )
    .bind("wss-missing-evidence")
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(state, "NotEstablished");
    db.teardown().await;
}

#[tokio::test]
async fn evidence_for_certificate_b_is_silently_rejected_on_certificate_a_connection() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let issuer = TrustedBootstrapFixtureIssuer::from_seed([0x33; 32]);
    let evidence_service = Arc::new(BootstrapEvidenceService::new(
        Arc::new(PostgresEndpointRepository::new(db.pool.clone())),
        AcceptedSiteKeys::single(issuer.public_key()),
    ));
    let gateway =
        Arc::new(Gateway::new(enrollment).with_bootstrap_evidence_service(evidence_service));
    let (cert_a, key_a) = generate_test_cert("localhost");
    let fingerprint_a = ServerCertFingerprint::from_leaf_der(cert_a.as_ref());
    let (cert_b, _key_b) = generate_test_cert("localhost");
    let fingerprint_b = ServerCertFingerprint::from_leaf_der(cert_b.as_ref());
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let acceptor = AgentTransportAcceptor::new(vec![cert_a], key_a).unwrap();
    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.unwrap();
        let mut connection = acceptor.accept(tcp).await.unwrap();
        assert_eq!(connection.server_fingerprint, fingerprint_a);
        let HandshakeOutcome::Established(session) =
            gateway.handshake(&mut connection.websocket).await?
        else {
            panic!("authentication must establish")
        };
        gateway
            .run_authenticated_session(
                &mut connection.websocket,
                session,
                connection.server_fingerprint,
            )
            .await
    });
    let nonce = BootNonce::from_bytes([0x53; 32]);
    let credential = boot
        .issue_enrollment_credential("wss-fingerprint-bound", nonce, Utc::now())
        .await
        .unwrap();
    // Pin A normally. The deliberately mismatching B assertion is sent only
    // after authentication, bypassing the normal local-bootstrap gate.
    let mut websocket = connect_pinned_wss(addr, "localhost", fingerprint_a)
        .await
        .unwrap();
    assert!(matches!(
        authenticate(&mut websocket, &credential.to_wire_value())
            .await
            .unwrap(),
        SimulatorHandshakeOutcome::Established(_)
    ));
    let evidence = BootstrapEvidenceMessage::new(
        nonce.to_wire_value(),
        issuer.issue(nonce, fingerprint_b).to_wire_value(),
    );
    websocket
        .send(Message::text(
            encode(&AgentProtocolMessage::BootstrapEvidence(evidence)).unwrap(),
        ))
        .await
        .unwrap();
    websocket.send(Message::text("{")).await.unwrap();
    let response = websocket.next().await.unwrap().unwrap();
    assert!(matches!(
        decode(response.into_text().unwrap().as_str()).unwrap(),
        AgentProtocolMessage::ProtocolError(_)
    ));
    websocket.close(None).await.unwrap();
    server.await.unwrap().unwrap();
    let state: String = sqlx::query_scalar(
        "SELECT trusted_bootstrap_state::text FROM endpoints WHERE inventory_signal = $1",
    )
    .bind("wss-fingerprint-bound")
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(state, "NotEstablished");
    db.teardown().await;
}

#[tokio::test]
async fn old_boot_a_evidence_cannot_establish_authenticated_boot_b_session() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let issuer = TrustedBootstrapFixtureIssuer::from_seed([0x34; 32]);
    let evidence_service = Arc::new(BootstrapEvidenceService::new(
        Arc::new(PostgresEndpointRepository::new(db.pool.clone())),
        AcceptedSiteKeys::single(issuer.public_key()),
    ));
    let gateway = Arc::new(
        Gateway::new(Arc::clone(&enrollment)).with_bootstrap_evidence_service(evidence_service),
    );
    let (cert_der, key_der) = generate_test_cert("localhost");
    let fingerprint = ServerCertFingerprint::from_leaf_der(cert_der.as_ref());
    let listener = Arc::new(TcpListener::bind("127.0.0.1:0").await.unwrap());
    let addr = listener.local_addr().unwrap();
    let acceptor = Arc::new(AgentTransportAcceptor::new(vec![cert_der], key_der).unwrap());
    let nonce_a = BootNonce::from_bytes([0x54; 32]);
    let assertion_a = issuer.issue(nonce_a, fingerprint);
    let old_evidence =
        BootstrapEvidenceMessage::new(nonce_a.to_wire_value(), assertion_a.to_wire_value());
    let credential_a = boot
        .issue_enrollment_credential("wss-historical", nonce_a, Utc::now())
        .await
        .unwrap();

    let listener_a = Arc::clone(&listener);
    let acceptor_a = Arc::clone(&acceptor);
    let gateway_a = Arc::clone(&gateway);
    let server_a = tokio::spawn(async move {
        let (tcp, _) = listener_a.accept().await.unwrap();
        let mut connection = acceptor_a.accept(tcp).await.unwrap();
        let HandshakeOutcome::Established(session) =
            gateway_a.handshake(&mut connection.websocket).await?
        else {
            panic!()
        };
        gateway_a
            .run_authenticated_session(
                &mut connection.websocket,
                session,
                connection.server_fingerprint,
            )
            .await
    });
    let material_a = SimulatedBootstrapMaterial::from_assertion(&assertion_a);
    let paired = SimulatedPairedTrust::single(issuer.public_key());
    let mut connection_a =
        connect_after_trusted_bootstrap(addr, "localhost", &paired, nonce_a, &material_a)
            .await
            .unwrap();
    assert!(matches!(
        authenticate(&mut connection_a.websocket, &credential_a.to_wire_value())
            .await
            .unwrap(),
        SimulatorHandshakeOutcome::Established(_)
    ));
    send_bootstrap_evidence(&mut connection_a.websocket, &connection_a.established)
        .await
        .unwrap();
    connection_a.websocket.close(None).await.unwrap();
    server_a.await.unwrap().unwrap();

    let nonce_b = BootNonce::from_bytes([0x55; 32]);
    let credential_b = boot
        .issue_enrollment_credential("wss-historical", nonce_b, Utc::now())
        .await
        .unwrap();
    let listener_b = Arc::clone(&listener);
    let acceptor_b = Arc::clone(&acceptor);
    let gateway_b = Arc::clone(&gateway);
    let server_b = tokio::spawn(async move {
        let (tcp, _) = listener_b.accept().await.unwrap();
        let mut connection = acceptor_b.accept(tcp).await.unwrap();
        let HandshakeOutcome::Established(session) =
            gateway_b.handshake(&mut connection.websocket).await?
        else {
            panic!()
        };
        gateway_b
            .run_authenticated_session(
                &mut connection.websocket,
                session,
                connection.server_fingerprint,
            )
            .await
    });
    let mut websocket_b = connect_pinned_wss(addr, "localhost", fingerprint)
        .await
        .unwrap();
    assert!(matches!(
        authenticate(&mut websocket_b, &credential_b.to_wire_value())
            .await
            .unwrap(),
        SimulatorHandshakeOutcome::Established(_)
    ));
    let before: (Vec<u8>, String) = sqlx::query_as(
        "SELECT current_boot_nonce, trusted_bootstrap_state::text FROM endpoints WHERE inventory_signal = $1",
    )
    .bind("wss-historical")
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        before,
        (nonce_b.as_bytes().to_vec(), "NotEstablished".into())
    );
    websocket_b
        .send(Message::text(
            encode(&AgentProtocolMessage::BootstrapEvidence(old_evidence)).unwrap(),
        ))
        .await
        .unwrap();
    websocket_b.send(Message::text("{")).await.unwrap();
    let response = websocket_b.next().await.unwrap().unwrap();
    assert!(matches!(
        decode(response.into_text().unwrap().as_str()).unwrap(),
        AgentProtocolMessage::ProtocolError(_)
    ));
    websocket_b.close(None).await.unwrap();
    server_b.await.unwrap().unwrap();
    let after: (Vec<u8>, String) = sqlx::query_as(
        "SELECT current_boot_nonce, trusted_bootstrap_state::text FROM endpoints WHERE inventory_signal = $1",
    )
    .bind("wss-historical")
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(
        after,
        (nonce_b.as_bytes().to_vec(), "NotEstablished".into())
    );
    db.teardown().await;
}

#[tokio::test]
async fn enrolled_endpoint_reconnects_over_fresh_wss_without_second_approval() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let gateway = Arc::new(Gateway::new(Arc::clone(&enrollment)));
    let (cert_der, key_der) = generate_test_cert("localhost");
    let fingerprint = ServerCertFingerprint::from_leaf_der(cert_der.as_ref());
    let listener = Arc::new(TcpListener::bind("127.0.0.1:0").await.unwrap());
    let addr = listener.local_addr().unwrap();
    let acceptor = Arc::new(AgentTransportAcceptor::new(vec![cert_der], key_der).unwrap());
    let e1 = issue_e1(&boot, "wss-enrolled-reconnect").await;

    let listener_first = Arc::clone(&listener);
    let acceptor_first = Arc::clone(&acceptor);
    let gateway_first = Arc::clone(&gateway);
    let server_first = tokio::spawn(async move {
        let (tcp, _) = listener_first.accept().await.unwrap();
        let mut connection = acceptor_first.accept(tcp).await.unwrap();
        gateway_first.handshake(&mut connection.websocket).await
    });
    let mut websocket_first = connect_pinned_wss(addr, "localhost", fingerprint)
        .await
        .unwrap();
    let SimulatorHandshakeOutcome::Established(first_established) =
        authenticate(&mut websocket_first, &e1.to_wire_value())
            .await
            .unwrap()
    else {
        panic!("first contact must establish")
    };
    drop(websocket_first);
    let HandshakeOutcome::Established(first_session) = server_first.await.unwrap().unwrap() else {
        panic!("Server must establish first session")
    };

    enrollment
        .approve_enrollment(
            first_session.endpoint_id,
            Actor::Operator {
                label: "wp1-reconnect-harness".into(),
            },
            Utc::now(),
        )
        .await
        .unwrap();
    let approval_events_before: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM domain_events WHERE endpoint_id = $1 AND event_type = 'EndpointEnrolled'",
    )
    .bind(first_session.endpoint_id.0)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(approval_events_before, 1);

    let listener_reconnect = Arc::clone(&listener);
    let acceptor_reconnect = Arc::clone(&acceptor);
    let gateway_reconnect = Arc::clone(&gateway);
    let server_reconnect = tokio::spawn(async move {
        let (tcp, _) = listener_reconnect.accept().await.unwrap();
        let mut connection = acceptor_reconnect.accept(tcp).await.unwrap();
        gateway_reconnect.handshake(&mut connection.websocket).await
    });
    let mut websocket_reconnect = connect_pinned_wss(addr, "localhost", fingerprint)
        .await
        .unwrap();
    let SimulatorHandshakeOutcome::Established(reconnect_established) = authenticate(
        &mut websocket_reconnect,
        &first_established.body.runtime_credential,
    )
    .await
    .unwrap() else {
        panic!("runtime credential reconnect must establish")
    };
    assert_ne!(
        reconnect_established.body.session_id, first_established.body.session_id,
        "reconnect must establish a fresh session"
    );
    drop(websocket_reconnect);
    let HandshakeOutcome::Established(reconnect_session) = server_reconnect.await.unwrap().unwrap()
    else {
        panic!("Server must establish reconnect session")
    };
    assert_eq!(reconnect_session.endpoint_id, first_session.endpoint_id);

    let (identity, approval_events_after): (String, i64) = sqlx::query_as(
        "SELECT e.identity_state::text, (SELECT COUNT(*) FROM domain_events d WHERE d.endpoint_id = e.id AND d.event_type = 'EndpointEnrolled') FROM endpoints e WHERE e.id = $1",
    )
    .bind(first_session.endpoint_id.0)
    .fetch_one(&db.pool)
    .await
    .unwrap();
    assert_eq!(identity, "Enrolled");
    assert_eq!(approval_events_after, approval_events_before);
    db.teardown().await;
}

#[tokio::test]
async fn predecessor_retry_replaces_successor_and_recovers_from_superseded_successor_over_wss() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let gateway = Arc::new(Gateway::new(enrollment));
    let (cert_der, key_der) = generate_test_cert("localhost");
    let fingerprint = ServerCertFingerprint::from_leaf_der(cert_der.as_ref());
    let listener = Arc::new(TcpListener::bind("127.0.0.1:0").await.unwrap());
    let addr = listener.local_addr().unwrap();
    let acceptor = Arc::new(AgentTransportAcceptor::new(vec![cert_der], key_der).unwrap());
    let e1 = issue_e1(&boot, "wss-predecessor-recovery").await;

    let listener_one = Arc::clone(&listener);
    let acceptor_one = Arc::clone(&acceptor);
    let gateway_one = Arc::clone(&gateway);
    let server_one = tokio::spawn(async move {
        let (tcp, _) = listener_one.accept().await.unwrap();
        let mut connection = acceptor_one.accept(tcp).await.unwrap();
        gateway_one.handshake(&mut connection.websocket).await
    });
    let mut websocket_one = connect_pinned_wss(addr, "localhost", fingerprint)
        .await
        .unwrap();
    let SimulatorHandshakeOutcome::Established(established_one) =
        authenticate(&mut websocket_one, &e1.to_wire_value())
            .await
            .unwrap()
    else {
        panic!("E1 first authentication must establish")
    };
    let r1 = established_one.body.runtime_credential;
    drop(websocket_one);
    assert!(matches!(
        server_one.await.unwrap().unwrap(),
        HandshakeOutcome::Established(_)
    ));

    // R1 was delivered but never authenticated. The still-valid predecessor
    // E1 must replace it with a fresh R1' through the real wire path.
    let listener_two = Arc::clone(&listener);
    let acceptor_two = Arc::clone(&acceptor);
    let gateway_two = Arc::clone(&gateway);
    let server_two = tokio::spawn(async move {
        let (tcp, _) = listener_two.accept().await.unwrap();
        let mut connection = acceptor_two.accept(tcp).await.unwrap();
        gateway_two.handshake(&mut connection.websocket).await
    });
    let mut websocket_two = connect_pinned_wss(addr, "localhost", fingerprint)
        .await
        .unwrap();
    let SimulatorHandshakeOutcome::Established(established_two) =
        authenticate(&mut websocket_two, &e1.to_wire_value())
            .await
            .unwrap()
    else {
        panic!("E1 predecessor retry must establish")
    };
    let r1_prime = established_two.body.runtime_credential;
    assert_ne!(r1_prime, r1);
    drop(websocket_two);
    assert!(matches!(
        server_two.await.unwrap().unwrap(),
        HandshakeOutcome::Established(_)
    ));

    // The superseded R1 is rejected generically and never establishes a
    // session.
    let listener_three = Arc::clone(&listener);
    let acceptor_three = Arc::clone(&acceptor);
    let gateway_three = Arc::clone(&gateway);
    let server_three = tokio::spawn(async move {
        let (tcp, _) = listener_three.accept().await.unwrap();
        let mut connection = acceptor_three.accept(tcp).await.unwrap();
        gateway_three.handshake(&mut connection.websocket).await
    });
    let mut websocket_three = connect_pinned_wss(addr, "localhost", fingerprint)
        .await
        .unwrap();
    let SimulatorHandshakeOutcome::Rejected(rejected) =
        authenticate(&mut websocket_three, &r1).await.unwrap()
    else {
        panic!("superseded R1 must be rejected")
    };
    assert_eq!(rejected.body.reason, "rejected");
    drop(websocket_three);
    assert!(matches!(
        server_three.await.unwrap().unwrap(),
        HandshakeOutcome::Rejected
    ));

    // Recovery remains possible with E1, which replaces unconfirmed R1'
    // with another fresh successor.
    let listener_four = Arc::clone(&listener);
    let acceptor_four = Arc::clone(&acceptor);
    let gateway_four = Arc::clone(&gateway);
    let server_four = tokio::spawn(async move {
        let (tcp, _) = listener_four.accept().await.unwrap();
        let mut connection = acceptor_four.accept(tcp).await.unwrap();
        gateway_four.handshake(&mut connection.websocket).await
    });
    let mut websocket_four = connect_pinned_wss(addr, "localhost", fingerprint)
        .await
        .unwrap();
    let SimulatorHandshakeOutcome::Established(established_four) =
        authenticate(&mut websocket_four, &e1.to_wire_value())
            .await
            .unwrap()
    else {
        panic!("E1 recovery retry must establish")
    };
    assert_ne!(established_four.body.runtime_credential, r1_prime);
    drop(websocket_four);
    assert!(matches!(
        server_four.await.unwrap().unwrap(),
        HandshakeOutcome::Established(_)
    ));
    db.teardown().await;
}

#[tokio::test]
async fn rejected_credential_over_real_wss_reaches_auth_error_after_pin_success() {
    let db = TestDatabase::setup().await;
    let (_boot, enrollment) = build_services(db.pool.clone());
    let gateway = Arc::new(Gateway::new(Arc::clone(&enrollment)));

    let (cert_der, key_der) = generate_test_cert("localhost");
    let expected_fingerprint = ServerCertFingerprint::from_leaf_der(cert_der.as_ref());
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("local addr");
    let acceptor = AgentTransportAcceptor::new(vec![cert_der], key_der).expect("build acceptor");

    let server_gateway = Arc::clone(&gateway);
    let server_task = tokio::spawn(async move {
        let (tcp_stream, _peer) = listener.accept().await.expect("accept tcp");
        // TLS pin verification (Server presents the expected certificate)
        // and the WebSocket Upgrade both succeed here, strictly before any
        // Agent Protocol message is exchanged.
        let mut ws_stream = acceptor.accept(tcp_stream).await.expect("tls+ws accept");
        server_gateway.handshake(&mut ws_stream).await
    });

    let mut client_ws = connect_pinned_wss(addr, "localhost", expected_fingerprint)
        .await
        .expect("pinned wss connect must succeed — TLS pin verification succeeds first");

    let bogus = PresentedCredential::generate(CredentialKind::Enrollment);
    let outcome = authenticate(&mut client_ws, &bogus.to_wire_value())
        .await
        .expect("Simulator handshake helper must not error on a rejected credential");

    let SimulatorHandshakeOutcome::Rejected(error) = outcome else {
        panic!("expected Rejected, got {outcome:?}");
    };
    assert_eq!(error.body.reason, "rejected");

    let server_outcome = server_task
        .await
        .expect("server task must not panic")
        .expect("a credential rejection is HandshakeOutcome::Rejected, not an AgentGatewayError");
    assert!(matches!(server_outcome, HandshakeOutcome::Rejected));

    db.teardown().await;
}

/// Strong proof, crossing the real WSS boundary, that an incompatible
/// `protocol_version` never redeems/rotates the presented credential
/// (`m0-agent-protocol-contract.md` "Transport and handshake" ordering
/// requirement): E1 is rejected once with `protocol_version = "2"`, then the
/// *same* E1 successfully establishes a session over a fresh WSS connection
/// with `protocol_version = "1"`.
#[tokio::test]
async fn incompatible_protocol_version_over_real_wss_never_redeems_the_credential() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let gateway = Arc::new(Gateway::new(Arc::clone(&enrollment)));

    let (cert_der, key_der) = generate_test_cert("localhost");
    let expected_fingerprint = ServerCertFingerprint::from_leaf_der(cert_der.as_ref());
    let listener = Arc::new(TcpListener::bind("127.0.0.1:0").await.expect("bind"));
    let addr: SocketAddr = listener.local_addr().expect("local addr");
    let acceptor =
        Arc::new(AgentTransportAcceptor::new(vec![cert_der], key_der).expect("build acceptor"));

    let e1 = issue_e1(&boot, "wss-version-01").await;

    // --- First, independent connection: E1 presented with protocol_version
    // = "2" — constructed manually, bypassing the Simulator `authenticate`
    // helper (which always sends v1), since this is precisely the
    // incompatible-version case under test.
    let listener_a = Arc::clone(&listener);
    let acceptor_a = Arc::clone(&acceptor);
    let gateway_a = Arc::clone(&gateway);
    let server_task_a = tokio::spawn(async move {
        let (tcp_stream, _peer) = listener_a.accept().await.expect("accept tcp (v2 attempt)");
        let mut ws_stream = acceptor_a
            .accept(tcp_stream)
            .await
            .expect("tls+ws accept (v2 attempt)");
        let result = gateway_a.handshake(&mut ws_stream).await;
        (ws_stream, result)
    });

    let mut client_ws_a = connect_pinned_wss(addr, "localhost", expected_fingerprint)
        .await
        .expect("pinned wss connect must succeed (v2 attempt)");

    let mut v2_request = AuthRequestMessage::new(e1.to_wire_value());
    v2_request.envelope.protocol_version = ProtocolVersion::new("2");
    let wire =
        encode(&AgentProtocolMessage::AuthRequest(v2_request)).expect("encode v2 AuthRequest");
    client_ws_a
        .send(Message::text(wire))
        .await
        .expect("send v2 AuthRequest");

    let response_frame = client_ws_a
        .next()
        .await
        .expect("a response frame is present")
        .expect("frame read ok");
    let response =
        decode(response_frame.into_text().expect("text frame").as_str()).expect("decode response");
    let AgentProtocolMessage::AuthError(error) = response else {
        panic!("expected AuthError for protocol_version = \"2\", got {response:?}");
    };
    assert_eq!(error.body.reason, "rejected");

    let (_ws_stream_a, server_result_a) =
        server_task_a.await.expect("server task a must not panic");
    assert!(matches!(
        server_result_a.expect("Rejected is not an AgentGatewayError"),
        HandshakeOutcome::Rejected
    ));

    // --- Second, fresh connection: the SAME E1 with protocol_version = "1"
    // must still successfully establish — proving the v2 attempt never
    // consumed/rotated it.
    let listener_b = Arc::clone(&listener);
    let acceptor_b = Arc::clone(&acceptor);
    let gateway_b = Arc::clone(&gateway);
    let server_task_b = tokio::spawn(async move {
        let (tcp_stream, _peer) = listener_b.accept().await.expect("accept tcp (v1 retry)");
        let mut ws_stream = acceptor_b
            .accept(tcp_stream)
            .await
            .expect("tls+ws accept (v1 retry)");
        gateway_b.handshake(&mut ws_stream).await
    });

    let mut client_ws_b = connect_pinned_wss(addr, "localhost", expected_fingerprint)
        .await
        .expect("pinned wss connect must succeed (v1 retry)");

    let outcome_b = authenticate(&mut client_ws_b, &e1.to_wire_value())
        .await
        .expect("Simulator handshake helper must not error on the v1 retry");
    assert!(
        matches!(outcome_b, SimulatorHandshakeOutcome::Established(_)),
        "E1 must still be fully valid for a genuine v1 AuthRequest, got {outcome_b:?}"
    );

    let server_outcome_b = server_task_b
        .await
        .expect("server task b must not panic")
        .expect("server-side Gateway handshake must succeed on the v1 retry");
    assert!(matches!(server_outcome_b, HandshakeOutcome::Established(_)));

    db.teardown().await;
}
