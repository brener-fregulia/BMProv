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
    decode, encode, AgentProtocolMessage, AuthRequestMessage, ProtocolVersion,
};
use bamep_domain::presented_credential::{CredentialKind, PresentedCredential};
use bamep_server::adapters::agent_gateway::{AgentControlGateway, HandshakeOutcome};
use bamep_server::adapters::agent_transport::AgentTransportAcceptor;
use bamep_server::adapters::postgres::{
    PostgresBootContextRepository, PostgresCredentialRedemptionRepository,
    PostgresEndpointRepository,
};
use bamep_server::application::{BootOrchestrationService, EnrollmentService};
use bamep_simulator::{
    authenticate, connect_pinned_wss, ServerCertFingerprint, SimulatorHandshakeOutcome,
};
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
    boot.issue_enrollment_credential(signal, Utc::now())
        .await
        .expect("issuance must succeed")
}

#[tokio::test]
async fn valid_auth_request_over_real_wss_reaches_session_established() {
    let db = TestDatabase::setup().await;
    let (boot, enrollment) = build_services(db.pool.clone());
    let gateway = Arc::new(Gateway::new(Arc::clone(&enrollment)));

    let (cert_der, key_der) = generate_test_cert("localhost");
    let expected_fingerprint = ServerCertFingerprint::from_leaf_der(cert_der.as_ref());
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("local addr");
    let acceptor = AgentTransportAcceptor::new(vec![cert_der], key_der).expect("build acceptor");

    let server_gateway = Arc::clone(&gateway);
    let server_task = tokio::spawn(async move {
        let (tcp_stream, _peer) = listener.accept().await.expect("accept tcp");
        let mut ws_stream = acceptor.accept(tcp_stream).await.expect("tls+ws accept");
        server_gateway.handshake(&mut ws_stream).await
    });

    let mut client_ws = connect_pinned_wss(addr, "localhost", expected_fingerprint)
        .await
        .expect("pinned wss connect must succeed for the matching certificate");

    let e1 = issue_e1(&boot, "wss-established-01").await;
    let outcome = authenticate(&mut client_ws, &e1.to_wire_value())
        .await
        .expect("Simulator handshake helper must not error on a valid AuthRequest");

    let SimulatorHandshakeOutcome::Established(established) = outcome else {
        panic!("expected Established, got {outcome:?}");
    };
    assert!(!established.body.runtime_credential.is_empty());

    let server_outcome = server_task
        .await
        .expect("server task must not panic")
        .expect("server-side Gateway handshake must succeed");
    assert!(matches!(server_outcome, HandshakeOutcome::Established(_)));

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
