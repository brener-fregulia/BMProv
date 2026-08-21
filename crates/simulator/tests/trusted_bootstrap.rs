//! Trusted-bootstrap-then-WSS composition tests (Issue #17 WP1 checkpoint).
//!
//! Proves, using a real loopback `TcpListener` as an observable boundary
//! (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
//! Section 8 "Simulator contract" — "Structural local gate"): wrong signer,
//! corrupted signature, and an assertion for an old/nonmatching nonce each
//! fail local trusted-bootstrap establishment strictly before any TCP
//! connection is attempted; and a valid fixture establishes trust locally,
//! then a real pinned TLS 1.3 WSS connection succeeds using only the
//! fingerprint `EstablishedTrustedBootstrap` supplies.

use std::net::SocketAddr;
use std::time::Duration;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

use bamep_server::adapters::agent_transport::AgentTransportAcceptor;
use bamep_simulator::{
    connect_after_trusted_bootstrap, ConnectAfterTrustedBootstrapError, LocalBootstrapError,
    SimulatedBootstrapMaterial, SimulatedPairedTrust, TrustedBootstrapFixtureIssuer,
};
use bamep_trusted_bootstrap::{BootNonce, ServerCertFingerprint};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::net::TcpListener;

/// Generates one ephemeral self-signed leaf certificate/key pair, for test
/// fixtures only (`docs/development/testing.md` "Test isolation").
fn generate_test_cert(subject_alt_name: &str) -> (CertificateDer<'static>, PrivateKeyDer<'static>) {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec![subject_alt_name.to_string()]).expect("cert generation");
    let cert_der = cert.der().clone();
    let key_der = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));
    (cert_der, key_der)
}

/// Deterministic structural proof that zero TCP connection was ever queued:
/// nothing in this test suite ever dials `listener`'s address, so a bounded
/// `accept()` must time out. The timeout is only a watchdog around the
/// necessarily-never-resolving `accept()` future, not the pass condition
/// itself — the pass condition is that no connection exists.
async fn assert_zero_tcp_connections(listener: TcpListener) {
    let accept_result = tokio::time::timeout(Duration::from_millis(200), listener.accept()).await;
    assert!(
        accept_result.is_err(),
        "no TCP connection may ever be queued when local trusted-bootstrap establishment fails first"
    );
}

#[tokio::test]
async fn wrong_signer_fails_before_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("local addr");

    let legitimate_issuer = TrustedBootstrapFixtureIssuer::from_seed([0x11; 32]);
    let wrong_issuer = TrustedBootstrapFixtureIssuer::from_seed([0x22; 32]);
    let paired_trust = SimulatedPairedTrust::single(legitimate_issuer.public_key());

    let boot_nonce = BootNonce::from_bytes([0x33; 32]);
    let fingerprint = ServerCertFingerprint::from_sha256_digest([0x44; 32]);

    // Signed by a key the Simulated Agent's paired trust never accepted.
    let assertion = wrong_issuer.issue(boot_nonce, fingerprint);
    let material = SimulatedBootstrapMaterial::from_assertion(&assertion);

    let result =
        connect_after_trusted_bootstrap(addr, "localhost", &paired_trust, boot_nonce, &material)
            .await;
    match result {
        Err(ConnectAfterTrustedBootstrapError::LocalBootstrap(
            LocalBootstrapError::VerificationFailed(_),
        )) => {}
        Err(other) => panic!("expected verification failure, got a different error: {other}"),
        Ok(_) => panic!(
            "expected local verification to reject the wrong signer, but establishment succeeded"
        ),
    }

    assert_zero_tcp_connections(listener).await;
}

#[tokio::test]
async fn corrupted_signature_fails_before_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("local addr");

    let issuer = TrustedBootstrapFixtureIssuer::from_seed([0x55; 32]);
    let paired_trust = SimulatedPairedTrust::single(issuer.public_key());

    let boot_nonce = BootNonce::from_bytes([0x66; 32]);
    let fingerprint = ServerCertFingerprint::from_sha256_digest([0x77; 32]);
    let assertion = issuer.issue(boot_nonce, fingerprint);

    // Corrupt one byte inside the Ed25519 signature portion (offset 133..197
    // of the decoded 197-byte assertion) while leaving the transcript intact.
    let mut decoded = URL_SAFE_NO_PAD
        .decode(assertion.to_wire_value())
        .expect("valid assertion decodes");
    assert_eq!(decoded.len(), 197);
    decoded[150] ^= 0xFF;
    let corrupted_carrier = URL_SAFE_NO_PAD.encode(&decoded);
    let material = SimulatedBootstrapMaterial::from_wire_value(corrupted_carrier);

    let result =
        connect_after_trusted_bootstrap(addr, "localhost", &paired_trust, boot_nonce, &material)
            .await;
    match result {
        Err(ConnectAfterTrustedBootstrapError::LocalBootstrap(
            LocalBootstrapError::VerificationFailed(_),
        )) => {}
        Err(other) => panic!("expected verification failure, got a different error: {other}"),
        Ok(_) => panic!("expected local verification to reject the corrupted signature, but establishment succeeded"),
    }

    assert_zero_tcp_connections(listener).await;
}

#[tokio::test]
async fn nonce_mismatch_replay_fails_before_tcp() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("local addr");

    let issuer = TrustedBootstrapFixtureIssuer::from_seed([0x88; 32]);
    let paired_trust = SimulatedPairedTrust::single(issuer.public_key());

    let old_boot_nonce_a = BootNonce::from_bytes([0xA1; 32]);
    let current_boot_nonce_b = BootNonce::from_bytes([0xB2; 32]);
    let fingerprint = ServerCertFingerprint::from_sha256_digest([0x99; 32]);

    // A cryptographically valid assertion signed for the OLD nonce A.
    let assertion = issuer.issue(old_boot_nonce_a, fingerprint);
    let material = SimulatedBootstrapMaterial::from_assertion(&assertion);

    // The current simulated boot expects nonce B.
    let result = connect_after_trusted_bootstrap(
        addr,
        "localhost",
        &paired_trust,
        current_boot_nonce_b,
        &material,
    )
    .await;
    match result {
        Err(ConnectAfterTrustedBootstrapError::LocalBootstrap(
            LocalBootstrapError::NonceMismatch,
        )) => {}
        Err(other) => panic!("expected a nonce mismatch, got a different error: {other}"),
        Ok(_) => panic!("expected local establishment to reject the replayed old-nonce assertion, but establishment succeeded"),
    }

    assert_zero_tcp_connections(listener).await;
}

#[tokio::test]
async fn positive_local_trust_then_real_pinned_wss() {
    let (cert_der, key_der) = generate_test_cert("localhost");
    let expected_fingerprint = ServerCertFingerprint::from_leaf_der(cert_der.as_ref());

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr: SocketAddr = listener.local_addr().expect("local addr");
    let acceptor = AgentTransportAcceptor::new(vec![cert_der], key_der).expect("build acceptor");

    let server_task = tokio::spawn(async move {
        let (tcp_stream, _peer) = listener.accept().await.expect("accept tcp");
        acceptor.accept(tcp_stream).await.expect("tls+ws accept");
    });

    let issuer = TrustedBootstrapFixtureIssuer::from_seed([0x99; 32]);
    let paired_trust = SimulatedPairedTrust::single(issuer.public_key());
    let boot_nonce = BootNonce::from_bytes([0xC3; 32]);

    // The fixture-signed assertion carries the real Server certificate's
    // fingerprint, so the real WSS pin check must genuinely succeed.
    let assertion = issuer.issue(boot_nonce, expected_fingerprint);
    let material = SimulatedBootstrapMaterial::from_assertion(&assertion);

    let connection =
        connect_after_trusted_bootstrap(addr, "localhost", &paired_trust, boot_nonce, &material)
            .await
            .expect("local trusted-bootstrap establishment then pinned WSS must succeed");

    assert_eq!(connection.established.boot_nonce(), boot_nonce);
    assert_eq!(
        connection.established.server_fingerprint(),
        expected_fingerprint
    );
    assert_eq!(
        connection.established.assertion_wire_value(),
        assertion.to_wire_value()
    );

    server_task.await.expect("server task did not panic");
}
