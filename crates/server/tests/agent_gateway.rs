//! Gateway-semantics tests (Issue #17 WP1 handshake checkpoint):
//! `AgentControlGateway::handshake` driven directly over an in-memory
//! (no-TLS) established WebSocket, against the real PostgreSQL-backed
//! `EnrollmentService` (`support::TestDatabase`) — the same Application
//! stack `enrollment_lifecycle.rs` already exercises exhaustively at the
//! Domain/Application boundary. This file proves only the Gateway's own
//! wire-level behavior: decode/phase validation, generic `AuthError` policy,
//! protocol-version-before-redemption ordering, correlation, and
//! `SessionEstablished` construction.
//!
//! Real TLS/WSS transport composition (a valid handshake and a rejected
//! credential crossing the full TCP -> TLS 1.3 -> WebSocket -> Agent Protocol
//! boundary, plus the version-no-redeem proof) is covered separately by
//! `tests/agent_gateway_wss.rs`. `BootstrapEvidence` is used here only to
//! prove it is rejected as a wrong-phase message before `AuthRequest` — this
//! file does not process it.

mod support;

use std::sync::Arc;

use bamep_agent_protocol::{
    decode, encode, AgentProtocolMessage, AuthRequestMessage, BootstrapEvidenceMessage, Envelope,
    ProtocolErrorMessage, ProtocolVersion,
};
use bamep_domain::presented_credential::{CredentialKind, PresentedCredential};
use bamep_server::adapters::agent_gateway::{
    AgentControlGateway, AgentGatewayError, HandshakeOutcome,
};
use bamep_server::adapters::postgres::{
    PostgresBootContextRepository, PostgresCredentialRedemptionRepository,
    PostgresEndpointRepository,
};
use bamep_server::application::{
    ApplicationError, BootOrchestrationService, BootstrapEvidenceService, Clock, EnrollmentService,
    RedeemResult,
};
use bamep_trusted_bootstrap::ServerCertFingerprint;
use bamep_trusted_bootstrap::{fixture::FixtureAssertionSigner, AcceptedSiteKeys};
use chrono::{DateTime, Duration, TimeZone, Utc};
use futures_util::{SinkExt, StreamExt};
use sqlx::PgPool;
use support::{ManualClock, TestDatabase};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use uuid::Uuid;

type Enrollment =
    EnrollmentService<PostgresEndpointRepository, PostgresCredentialRedemptionRepository>;
type Gateway =
    AgentControlGateway<PostgresEndpointRepository, PostgresCredentialRedemptionRepository>;
type BootOrchestration = BootOrchestrationService<PostgresBootContextRepository>;

/// Truncated to millisecond precision, matching `MessageTimestamp`'s own
/// wire serialization precision (`bamep_agent_protocol::envelope`) — so a
/// full-precision `DateTime<Utc>` computed in the test compares equal to the
/// value that actually round-tripped over the wire.
fn truncate_to_millis(dt: DateTime<Utc>) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(dt.timestamp_millis()).unwrap()
}

fn build_services(
    pool: PgPool,
    clock: Arc<ManualClock>,
    credential_ttl: Duration,
) -> (BootOrchestration, Arc<Enrollment>) {
    let boot_repo = Arc::new(PostgresBootContextRepository::new(pool.clone()));
    let endpoint_repo = Arc::new(PostgresEndpointRepository::new(pool.clone()));
    let redemption_repo = Arc::new(PostgresCredentialRedemptionRepository::new(pool));
    let boot_orchestration = BootOrchestrationService::new(boot_repo, Duration::minutes(5));
    let enrollment =
        EnrollmentService::with_clock(endpoint_repo, redemption_repo, clock as Arc<dyn Clock>)
            .with_credential_ttl(credential_ttl);
    (boot_orchestration, Arc::new(enrollment))
}

async fn issue_e1(
    boot: &BootOrchestration,
    signal: &str,
    now: DateTime<Utc>,
) -> PresentedCredential {
    // This file proves Gateway wire-level semantics, not current-boot
    // persistence (see `enrollment_lifecycle.rs` for that) — a fresh,
    // discarded BootNonce is sufficient here.
    let boot_nonce =
        bamep_domain::BootNonce::generate().expect("OS CSPRNG must be available in tests");
    boot.issue_enrollment_credential(signal, boot_nonce, now)
        .await
        .expect("issuance must succeed")
}

async fn domain_event_count(pool: &PgPool, endpoint_id: Uuid) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM domain_events WHERE endpoint_id = $1")
        .bind(endpoint_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn endpoint_id_for_signal(pool: &PgPool, signal: &str) -> Uuid {
    sqlx::query_scalar("SELECT id FROM endpoints WHERE inventory_signal = $1")
        .bind(signal)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn total_endpoint_count(pool: &PgPool) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM endpoints")
        .fetch_one(pool)
        .await
        .unwrap()
}

/// A connected, no-TLS in-process WebSocket pair: a real WS Upgrade
/// handshake over an in-memory duplex pipe. Sufficient for handshake-phase
/// Gateway tests — TLS/WSS transport is a separate, already-proven boundary
/// (`src/adapters/agent_transport.rs`; `tests/agent_gateway_wss.rs`).
async fn websocket_pair() -> (
    WebSocketStream<tokio::io::DuplexStream>,
    WebSocketStream<tokio::io::DuplexStream>,
) {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let server_task =
        tokio::spawn(async move { tokio_tungstenite::accept_async(server_io).await.unwrap() });
    let (client_ws, _response) =
        tokio_tungstenite::client_async("ws://bamep-agent-test/", client_io)
            .await
            .expect("in-memory WebSocket Upgrade must succeed");
    let server_ws = server_task
        .await
        .expect("server accept task must not panic");
    (client_ws, server_ws)
}

async fn send_text(ws: &mut WebSocketStream<tokio::io::DuplexStream>, wire: String) {
    ws.send(Message::text(wire)).await.expect("send text frame");
}

async fn recv_message(ws: &mut WebSocketStream<tokio::io::DuplexStream>) -> AgentProtocolMessage {
    let frame = ws
        .next()
        .await
        .expect("a frame is present")
        .expect("frame read ok");
    decode(frame.into_text().expect("text frame").as_str()).expect("decode message")
}

#[tokio::test]
async fn authenticated_session_reports_wire_violations_but_keeps_invalid_evidence_silent() {
    let db = TestDatabase::setup().await;
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let (boot, enrollment) =
        build_services(db.pool.clone(), Arc::clone(&clock), Duration::minutes(10));
    let signer = FixtureAssertionSigner::from_seed([9; 32]);
    let evidence = Arc::new(BootstrapEvidenceService::new(
        Arc::new(PostgresEndpointRepository::new(db.pool.clone())),
        AcceptedSiteKeys::single(signer.public_key()),
    ));
    let gateway =
        Arc::new(Gateway::new(Arc::clone(&enrollment)).with_bootstrap_evidence_service(evidence));
    let e1 = issue_e1(&boot, "gw-session-violations", clock.now()).await;
    let (mut client_ws, mut server_ws) = websocket_pair().await;
    let request = AuthRequestMessage::new(e1.to_wire_value());
    send_text(
        &mut client_ws,
        encode(&AgentProtocolMessage::AuthRequest(request)).unwrap(),
    )
    .await;
    let HandshakeOutcome::Established(session) = gateway.handshake(&mut server_ws).await.unwrap()
    else {
        panic!()
    };
    let _ = recv_message(&mut client_ws).await;
    let server_gateway = Arc::clone(&gateway);
    let task = tokio::spawn(async move {
        server_gateway
            .run_authenticated_session(
                &mut server_ws,
                session,
                ServerCertFingerprint::from_sha256_digest([1; 32]),
            )
            .await
    });

    for frame in [
        Message::text("{"),
        Message::text(
            r#"{"type":"Unknown","message_id":"3fa85f64-5717-4562-b3fc-2c963f66afa6","protocol_version":"1","timestamp":"2026-08-14T21:00:00Z"}"#,
        ),
        Message::binary(vec![1, 2, 3]),
        Message::text(
            encode(&AgentProtocolMessage::AuthRequest(AuthRequestMessage::new(
                "again",
            )))
            .unwrap(),
        ),
    ] {
        client_ws.send(frame).await.unwrap();
        let AgentProtocolMessage::ProtocolError(error) = recv_message(&mut client_ws).await else {
            panic!()
        };
        assert_eq!(error.body.code, "GENERIC");
        assert_eq!(error.body.message, "protocol violation");
    }

    let invalid = BootstrapEvidenceMessage::new("not-a-nonce", "not-an-assertion");
    send_text(
        &mut client_ws,
        encode(&AgentProtocolMessage::BootstrapEvidence(invalid)).unwrap(),
    )
    .await;
    send_text(&mut client_ws, "{".to_string()).await;
    assert!(matches!(recv_message(&mut client_ws).await, AgentProtocolMessage::ProtocolError(_)),
        "invalid evidence emitted no response and the following violation proves the session stayed alive");

    let inbound = ProtocolErrorMessage::new("GENERIC", "protocol violation");
    send_text(
        &mut client_ws,
        encode(&AgentProtocolMessage::ProtocolError(inbound)).unwrap(),
    )
    .await;
    client_ws.close(None).await.unwrap();
    task.await.unwrap().unwrap();
    db.teardown().await;
}

#[tokio::test]
async fn valid_auth_request_establishes_session() {
    let db = TestDatabase::setup().await;
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let ttl = Duration::minutes(10);
    let (boot, enrollment) = build_services(db.pool.clone(), Arc::clone(&clock), ttl);
    let gateway = Gateway::new(Arc::clone(&enrollment));

    let now = clock.now();
    let expected_expiry = truncate_to_millis(now + ttl);
    let e1 = issue_e1(&boot, "gw-established-01", now).await;

    let (mut client_ws, mut server_ws) = websocket_pair().await;

    let auth_request = AuthRequestMessage::new(e1.to_wire_value());
    let request_message_id = auth_request.envelope.message_id;
    send_text(
        &mut client_ws,
        encode(&AgentProtocolMessage::AuthRequest(auth_request)).unwrap(),
    )
    .await;

    let outcome = gateway.handshake(&mut server_ws).await.unwrap();
    let HandshakeOutcome::Established(session) = outcome else {
        panic!("expected Established");
    };

    let response = recv_message(&mut client_ws).await;
    let AgentProtocolMessage::SessionEstablished(established) = response else {
        panic!("expected SessionEstablished, got {response:?}");
    };

    assert_eq!(
        established.envelope.correlation_id,
        Some(request_message_id),
        "correlation_id must equal the AuthRequest's own message_id"
    );
    assert_eq!(established.body.session_id, session.session_id);
    assert_eq!(
        session.session_id.as_uuid().get_version_num(),
        4,
        "session_id must be a fresh v4 UUID"
    );
    assert_eq!(
        truncate_to_millis(established.body.credential_expires_at.as_datetime()),
        expected_expiry,
        "credential_expires_at must be exactly the Application-returned expiry"
    );

    let endpoint_id = endpoint_id_for_signal(&db.pool, "gw-established-01").await;
    assert_eq!(session.endpoint_id.0, endpoint_id);
    assert_eq!(
        domain_event_count(&db.pool, endpoint_id).await,
        1,
        "exactly one redemption (first contact) must have durably committed"
    );

    // The delivered runtime_credential is exactly the Application-issued
    // value: only that exact credential can go on to authenticate.
    let reconnect_result = enrollment
        .redeem(&established.body.runtime_credential)
        .await
        .unwrap();
    assert!(matches!(reconnect_result, RedeemResult::Established { .. }));

    db.teardown().await;
}

#[tokio::test]
async fn rejected_credential_yields_generic_auth_error() {
    let db = TestDatabase::setup().await;
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let (_boot, enrollment) =
        build_services(db.pool.clone(), Arc::clone(&clock), Duration::minutes(10));
    let gateway = Gateway::new(Arc::clone(&enrollment));

    let bogus = PresentedCredential::generate(CredentialKind::Enrollment);
    let (mut client_ws, mut server_ws) = websocket_pair().await;

    let auth_request = AuthRequestMessage::new(bogus.to_wire_value());
    let request_message_id = auth_request.envelope.message_id;
    send_text(
        &mut client_ws,
        encode(&AgentProtocolMessage::AuthRequest(auth_request)).unwrap(),
    )
    .await;

    let outcome = gateway.handshake(&mut server_ws).await.unwrap();
    assert!(matches!(outcome, HandshakeOutcome::Rejected));

    let response = recv_message(&mut client_ws).await;
    let AgentProtocolMessage::AuthError(error) = response else {
        panic!("expected AuthError, got {response:?}");
    };
    assert_eq!(error.body.reason, "rejected");
    assert_eq!(error.envelope.correlation_id, Some(request_message_id));
    assert_eq!(total_endpoint_count(&db.pool).await, 0);

    db.teardown().await;
}

#[tokio::test]
async fn incompatible_protocol_version_rejects_without_redeeming() {
    let db = TestDatabase::setup().await;
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let (boot, enrollment) =
        build_services(db.pool.clone(), Arc::clone(&clock), Duration::minutes(10));
    let gateway = Gateway::new(Arc::clone(&enrollment));

    let e1 = issue_e1(&boot, "gw-version-01", clock.now()).await;
    let (mut client_ws, mut server_ws) = websocket_pair().await;

    let mut auth_request = AuthRequestMessage::new(e1.to_wire_value());
    auth_request.envelope.protocol_version = ProtocolVersion::new("2");
    let request_message_id = auth_request.envelope.message_id;
    send_text(
        &mut client_ws,
        encode(&AgentProtocolMessage::AuthRequest(auth_request)).unwrap(),
    )
    .await;

    let outcome = gateway.handshake(&mut server_ws).await.unwrap();
    assert!(matches!(outcome, HandshakeOutcome::Rejected));

    let response = recv_message(&mut client_ws).await;
    let AgentProtocolMessage::AuthError(error) = response else {
        panic!("expected AuthError, got {response:?}");
    };
    assert_eq!(error.body.reason, "rejected");
    assert_eq!(error.envelope.correlation_id, Some(request_message_id));

    // No credential transition: no Endpoint was ever created for this
    // inventory_signal.
    assert_eq!(total_endpoint_count(&db.pool).await, 0);

    // Strong proof: the SAME E1 still successfully establishes with v1 — an
    // incompatible-version attempt never consumed/rotated it.
    let (mut client_ws2, mut server_ws2) = websocket_pair().await;
    let retry_request = AuthRequestMessage::new(e1.to_wire_value());
    send_text(
        &mut client_ws2,
        encode(&AgentProtocolMessage::AuthRequest(retry_request)).unwrap(),
    )
    .await;
    let retry_outcome = gateway.handshake(&mut server_ws2).await.unwrap();
    assert!(
        matches!(retry_outcome, HandshakeOutcome::Established(_)),
        "E1 must still be fully valid for a genuine v1 AuthRequest"
    );

    db.teardown().await;
}

#[tokio::test]
async fn malformed_json_rejects_generically_without_correlation() {
    let db = TestDatabase::setup().await;
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let (_boot, enrollment) =
        build_services(db.pool.clone(), Arc::clone(&clock), Duration::minutes(10));
    let gateway = Gateway::new(Arc::clone(&enrollment));

    let (mut client_ws, mut server_ws) = websocket_pair().await;
    send_text(&mut client_ws, "{ this is not valid JSON".to_string()).await;

    let outcome = gateway.handshake(&mut server_ws).await.unwrap();
    assert!(matches!(outcome, HandshakeOutcome::Rejected));

    let response = recv_message(&mut client_ws).await;
    let AgentProtocolMessage::AuthError(error) = response else {
        panic!("expected AuthError, got {response:?}");
    };
    assert_eq!(error.body.reason, "rejected");
    assert_eq!(
        error.envelope.correlation_id, None,
        "no trustworthy message_id can be recovered from malformed JSON"
    );
    assert_eq!(total_endpoint_count(&db.pool).await, 0);

    db.teardown().await;
}

#[tokio::test]
async fn unknown_message_type_rejects_generically() {
    let db = TestDatabase::setup().await;
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let (_boot, enrollment) =
        build_services(db.pool.clone(), Arc::clone(&clock), Duration::minutes(10));
    let gateway = Gateway::new(Arc::clone(&enrollment));

    let (mut client_ws, mut server_ws) = websocket_pair().await;
    let envelope = Envelope::new();
    let wire = format!(
        r#"{{"type":"TotallyUnknownMessage","message_id":"{}","protocol_version":"1","timestamp":"{}"}}"#,
        envelope.message_id,
        envelope.timestamp.as_datetime().to_rfc3339(),
    );
    send_text(&mut client_ws, wire).await;

    let outcome = gateway.handshake(&mut server_ws).await.unwrap();
    assert!(matches!(outcome, HandshakeOutcome::Rejected));

    let response = recv_message(&mut client_ws).await;
    let AgentProtocolMessage::AuthError(error) = response else {
        panic!("expected AuthError, got {response:?}");
    };
    assert_eq!(error.body.reason, "rejected");
    assert_eq!(
        error.envelope.correlation_id, None,
        "an unknown type is a decode failure and never yields a trustworthy message_id"
    );
    assert_eq!(total_endpoint_count(&db.pool).await, 0);

    db.teardown().await;
}

#[tokio::test]
async fn bootstrap_evidence_before_auth_request_rejects_with_correlation() {
    let db = TestDatabase::setup().await;
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let (_boot, enrollment) =
        build_services(db.pool.clone(), Arc::clone(&clock), Duration::minutes(10));
    let gateway = Gateway::new(Arc::clone(&enrollment));

    let (mut client_ws, mut server_ws) = websocket_pair().await;
    let evidence = BootstrapEvidenceMessage::new("boot-nonce-01", "opaque-assertion-bytes");
    let evidence_message_id = evidence.envelope.message_id;
    send_text(
        &mut client_ws,
        encode(&AgentProtocolMessage::BootstrapEvidence(evidence)).unwrap(),
    )
    .await;

    let outcome = gateway.handshake(&mut server_ws).await.unwrap();
    assert!(matches!(outcome, HandshakeOutcome::Rejected));

    let response = recv_message(&mut client_ws).await;
    let AgentProtocolMessage::AuthError(error) = response else {
        panic!("expected AuthError, got {response:?}");
    };
    assert_eq!(error.body.reason, "rejected");
    assert_eq!(error.envelope.correlation_id, Some(evidence_message_id));
    assert_eq!(total_endpoint_count(&db.pool).await, 0);

    db.teardown().await;
}

#[tokio::test]
async fn binary_frame_during_handshake_rejects() {
    let db = TestDatabase::setup().await;
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let (_boot, enrollment) =
        build_services(db.pool.clone(), Arc::clone(&clock), Duration::minutes(10));
    let gateway = Gateway::new(Arc::clone(&enrollment));

    let (mut client_ws, mut server_ws) = websocket_pair().await;
    client_ws
        .send(Message::binary(vec![1u8, 2, 3]))
        .await
        .expect("send binary frame");

    let outcome = gateway.handshake(&mut server_ws).await.unwrap();
    assert!(matches!(outcome, HandshakeOutcome::Rejected));

    let response = recv_message(&mut client_ws).await;
    let AgentProtocolMessage::AuthError(error) = response else {
        panic!("expected AuthError, got {response:?}");
    };
    assert_eq!(error.body.reason, "rejected");
    assert_eq!(error.envelope.correlation_id, None);
    assert_eq!(total_endpoint_count(&db.pool).await, 0);

    db.teardown().await;
}

/// Proves whole-transaction rollback surfaces as a genuine `AgentGatewayError`
/// rather than being reinterpreted as a credential rejection — the same
/// test-local trigger technique `enrollment_lifecycle.rs`'s
/// `first_contact_rolls_back_entirely_when_lookup_projection_fails` uses,
/// applied here at the Gateway boundary.
#[tokio::test]
async fn repository_failure_surfaces_as_gateway_error_not_auth_error() {
    let db = TestDatabase::setup().await;
    let clock = Arc::new(ManualClock::new(Utc::now()));
    let (boot, enrollment) =
        build_services(db.pool.clone(), Arc::clone(&clock), Duration::minutes(10));
    let gateway = Gateway::new(Arc::clone(&enrollment));

    let e1 = issue_e1(&boot, "gw-repo-failure-01", clock.now()).await;

    sqlx::query(
        "CREATE FUNCTION fail_lookup_insert() RETURNS trigger AS $$
         BEGIN
             RAISE EXCEPTION 'test-induced failure: lookup insert must never become durable';
         END;
         $$ LANGUAGE plpgsql",
    )
    .execute(&db.pool)
    .await
    .unwrap();
    sqlx::query(
        "CREATE TRIGGER fail_lookup_insert_trigger \
         BEFORE INSERT ON endpoint_credential_lookups \
         FOR EACH ROW EXECUTE FUNCTION fail_lookup_insert()",
    )
    .execute(&db.pool)
    .await
    .unwrap();

    let (mut client_ws, mut server_ws) = websocket_pair().await;
    let auth_request = AuthRequestMessage::new(e1.to_wire_value());
    send_text(
        &mut client_ws,
        encode(&AgentProtocolMessage::AuthRequest(auth_request)).unwrap(),
    )
    .await;

    let result = gateway.handshake(&mut server_ws).await;
    assert!(
        matches!(
            result,
            Err(AgentGatewayError::Application(
                ApplicationError::Repository(_)
            ))
        ),
        "expected an AgentGatewayError wrapping a repository failure, got {result:?}"
    );
    assert_eq!(
        total_endpoint_count(&db.pool).await,
        0,
        "a failed transaction must not leave a partially committed Endpoint"
    );

    // Neither SessionEstablished nor AuthError was ever sent for this
    // attempt.
    let no_response =
        tokio::time::timeout(std::time::Duration::from_millis(200), client_ws.next()).await;
    assert!(
        no_response.is_err(),
        "no handshake response frame must be sent when redeem fails with a repository error"
    );

    db.teardown().await;
}
