//! Real loopback TCP/TLS 1.3/WebSocket Agent Protocol transport tests
//! (Issue #17 WP1 checkpoint).
//!
//! Proves only the transport boundary defined by
//! `docs/specifications/m0-agent-protocol-contract.md` "Transport and
//! handshake": a correct pinned exact-leaf-certificate fingerprint
//! establishes a real WSS connection over which a real `AuthRequest` JSON
//! text frame crosses; a wrong pin fails during TLS before the WebSocket
//! Upgrade, and structurally before any Agent Protocol message could be
//! exchanged. Does not call `EnrollmentService`, and does not produce
//! `SessionEstablished`/`AuthError` — those belong to a later checkpoint.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bamep_agent_protocol::{decode, encode, AgentProtocolMessage, AuthRequestMessage};
use bamep_server::adapters::agent_transport::AgentTransportAcceptor;
use bamep_simulator::{connect_pinned_wss, ServerCertFingerprint};
use futures_util::{SinkExt, StreamExt};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::Message;

/// Generates one ephemeral self-signed leaf certificate/key pair, for test
/// fixtures only (`docs/development/testing.md` "Test isolation" — no
/// public Internet, no machine/OS certificate store).
fn generate_test_cert(subject_alt_name: &str) -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec![subject_alt_name.to_string()]).expect("cert generation");
    let cert_der = cert.der().clone();
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));
    (cert_der, key_der)
}

#[tokio::test]
async fn positive_pinned_wss_carries_real_auth_request() {
    let (cert_der, key_der) = generate_test_cert("localhost");
    let expected_fingerprint = ServerCertFingerprint::from_leaf_der(cert_der.as_ref());

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("local addr");

    let acceptor = AgentTransportAcceptor::new(vec![cert_der], key_der).expect("build acceptor");

    let server_task = tokio::spawn(async move {
        let (tcp_stream, _peer) = listener.accept().await.expect("accept tcp");
        let mut ws_stream = acceptor.accept(tcp_stream).await.expect("tls+ws accept");

        let frame = ws_stream
            .next()
            .await
            .expect("a frame is present")
            .expect("frame read ok");
        let text = frame.into_text().expect("text frame");
        decode(&text).expect("decode AuthRequest")
    });

    let mut client_ws = connect_pinned_wss(addr, "localhost", expected_fingerprint)
        .await
        .expect("pinned wss connect must succeed for the matching certificate");

    // Real-connection proof that only TLS 1.3 was negotiated
    // (`m0-agent-protocol-contract.md` "TLS 1.3 only"), rather than
    // inspecting an unrelated constant.
    let (_io, client_conn) = client_ws.get_ref().get_ref();
    assert_eq!(
        client_conn.protocol_version(),
        Some(rustls::ProtocolVersion::TLSv1_3)
    );

    let auth_request = AgentProtocolMessage::AuthRequest(AuthRequestMessage::new(
        "test-agent-credential-wire-value",
    ));
    let wire = encode(&auth_request).expect("encode AuthRequest");
    client_ws
        .send(Message::text(wire))
        .await
        .expect("send AuthRequest text frame");

    let decoded = server_task.await.expect("server task did not panic");
    match decoded {
        AgentProtocolMessage::AuthRequest(message) => {
            assert_eq!(message.body.credential, "test-agent-credential-wire-value");
        }
        other => panic!("expected AuthRequest, got {other:?}"),
    }
}

#[tokio::test]
async fn pin_mismatch_fails_closed_before_websocket_and_auth_request() {
    let (server_cert_der, server_key_der) = generate_test_cert("localhost");
    let (unrelated_cert_der, _unrelated_key_der) = generate_test_cert("localhost");
    let wrong_fingerprint = ServerCertFingerprint::from_leaf_der(unrelated_cert_der.as_ref());

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("local addr");

    let acceptor =
        AgentTransportAcceptor::new(vec![server_cert_der], server_key_der).expect("build acceptor");

    // Structural observability: these only ever become true after TLS
    // succeeds and the WebSocket Upgrade completes — never inferred from
    // `connect_pinned_wss`'s own return value.
    let tls_and_websocket_established = Arc::new(AtomicBool::new(false));
    let message_received = Arc::new(AtomicBool::new(false));
    let established_flag = tls_and_websocket_established.clone();
    let message_flag = message_received.clone();

    let server_task = tokio::spawn(async move {
        if let Ok((tcp_stream, _peer)) = listener.accept().await {
            if let Ok(mut ws_stream) = acceptor.accept(tcp_stream).await {
                established_flag.store(true, Ordering::SeqCst);
                if let Some(Ok(_frame)) = ws_stream.next().await {
                    message_flag.store(true, Ordering::SeqCst);
                }
            }
        }
    });

    let connect_result = connect_pinned_wss(addr, "localhost", wrong_fingerprint).await;
    assert!(
        connect_result.is_err(),
        "a fingerprint mismatch must fail the TLS handshake, never yield a WebSocket stream"
    );

    // Give the server task a bounded chance to observe the aborted
    // connection attempt before asserting the negative outcome.
    let _ = tokio::time::timeout(Duration::from_millis(500), server_task).await;

    assert!(
        !tls_and_websocket_established.load(Ordering::SeqCst),
        "TLS/WebSocket must never establish when the pinned fingerprint does not match"
    );
    assert!(
        !message_received.load(Ordering::SeqCst),
        "no Agent Protocol message can be exchanged when the WebSocket was never established"
    );
}
