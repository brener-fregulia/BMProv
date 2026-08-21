//! Simulator local trusted-bootstrap establishment
//! (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
//! Section 8 "Simulator contract" — "Structural local gate", "Key
//! separation"): local establishment must strictly precede the WSS
//! connection, using only the authenticated `ServerCertFingerprint` an
//! [`EstablishedTrustedBootstrap`] supplies — never a `trusted: bool`, never
//! an unchecked constructor, never a public constructor for the fixture
//! issuer's private key to leak into the Simulated Agent's own trust state.
//!
//! Kept out of `transport.rs`/`handshake.rs`/`verifier.rs`: those own the
//! already-approved WSS/TLS/Agent Protocol handshake mechanics; this module
//! owns only the pre-WSS local trust decision and its composition with them.
//!
//! Does not implement `BootstrapEvidence` processing (Server-side) — that
//! belongs to a later checkpoint of Issue #17.

use std::net::SocketAddr;

use bamep_trusted_bootstrap::fixture::{FixtureAssertionSigner, FixtureSignerError};
use bamep_trusted_bootstrap::{
    AcceptedSiteKeys, AssertionParseError, BootNonce, BootstrapAssertion, ServerCertFingerprint,
    SitePublicKey, VerificationError,
};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;
use tokio_tungstenite::WebSocketStream;

use crate::transport::{connect_pinned_wss, SimulatorTransportError};

/// Owns the fixture Ed25519 private signing key
/// (`m0-trusted-bootstrap-and-server-fingerprint-contract.md` Section 8 "Key
/// separation"). Never handed to [`SimulatedPairedTrust`] or any Simulated
/// Agent/trust object — only its public key crosses that boundary. WP1
/// fixture/test support, not a production Server assertion-signing service
/// or the real ADR-0011 pairing ceremony.
pub struct TrustedBootstrapFixtureIssuer {
    signer: FixtureAssertionSigner,
}

#[derive(Debug, thiserror::Error)]
pub enum TrustedBootstrapFixtureError {
    #[error("failed to generate the fixture site signing key")]
    KeyGeneration(#[from] FixtureSignerError),
}

impl TrustedBootstrapFixtureIssuer {
    /// Fresh CSPRNG-generated fixture site signing key.
    pub fn generate() -> Result<Self, TrustedBootstrapFixtureError> {
        Ok(Self {
            signer: FixtureAssertionSigner::generate()?,
        })
    }

    /// Deterministic construction from a fixed 32-byte seed, for
    /// reproducible Simulator scenarios and tests.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        Self {
            signer: FixtureAssertionSigner::from_seed(seed),
        }
    }

    /// The public key a [`SimulatedPairedTrust`] may accept. Never exposes
    /// the private signing key.
    pub fn public_key(&self) -> SitePublicKey {
        self.signer.public_key()
    }

    /// Issues a signed V1 assertion for the given boot's nonce and expected
    /// Server fingerprint. `SiteKeyId` is always derived from this issuer's
    /// own public key.
    pub fn issue(
        &self,
        boot_nonce: BootNonce,
        server_fingerprint: ServerCertFingerprint,
    ) -> BootstrapAssertion {
        self.signer.sign_v1(boot_nonce, server_fingerprint)
    }
}

/// The Simulated Agent's own paired-trust state: only already-accepted site
/// public key(s), never a private key
/// (`m0-trusted-bootstrap-and-server-fingerprint-contract.md` Section 8 "Key
/// separation").
pub struct SimulatedPairedTrust {
    accepted: AcceptedSiteKeys,
}

impl SimulatedPairedTrust {
    pub fn new(accepted_site_public_keys: impl IntoIterator<Item = SitePublicKey>) -> Self {
        Self {
            accepted: AcceptedSiteKeys::new(accepted_site_public_keys),
        }
    }

    pub fn single(accepted_site_public_key: SitePublicKey) -> Self {
        Self::new([accepted_site_public_key])
    }
}

/// The signed assertion material presented for one simulated boot attempt,
/// carried as its canonical V1 wire value.
pub struct SimulatedBootstrapMaterial {
    assertion_carrier: String,
}

impl SimulatedBootstrapMaterial {
    pub fn from_assertion(assertion: &BootstrapAssertion) -> Self {
        Self {
            assertion_carrier: assertion.to_wire_value(),
        }
    }

    /// For negative-fixture tests that need to present an arbitrary or
    /// corrupted carrier string rather than a validly-encoded
    /// [`BootstrapAssertion`].
    pub fn from_wire_value(carrier: impl Into<String>) -> Self {
        Self {
            assertion_carrier: carrier.into(),
        }
    }
}

/// Successful local establishment
/// (`m0-trusted-bootstrap-and-server-fingerprint-contract.md` Section 8
/// "Structural local gate"). Fields are private; the only normal way to
/// obtain this type is [`establish_trusted_bootstrap`] succeeding.
pub struct EstablishedTrustedBootstrap {
    boot_nonce: BootNonce,
    server_fingerprint: ServerCertFingerprint,
    assertion_carrier: String,
}

impl EstablishedTrustedBootstrap {
    pub fn boot_nonce(&self) -> BootNonce {
        self.boot_nonce
    }

    pub fn server_fingerprint(&self) -> ServerCertFingerprint {
        self.server_fingerprint
    }

    pub fn assertion_wire_value(&self) -> &str {
        &self.assertion_carrier
    }
}

#[derive(Debug, thiserror::Error)]
pub enum LocalBootstrapError {
    #[error("bootstrap assertion carrier failed strict canonical parsing")]
    Malformed(#[from] AssertionParseError),
    #[error("bootstrap assertion failed cryptographic verification")]
    VerificationFailed(#[from] VerificationError),
    #[error("verified assertion BootNonce does not match this simulated boot's current BootNonce")]
    NonceMismatch,
}

/// Pure, synchronous local establishment — never performs I/O, so a caller
/// can prove a rejection happened strictly before any network activity
/// (`m0-trusted-bootstrap-and-server-fingerprint-contract.md` Section 8
/// "Structural local gate"): parse strictly/canonically, verify against
/// `paired_trust`'s already-accepted site keys, then require the verified
/// `BootNonce` to equal `expected_boot_nonce` for this simulated boot.
pub fn establish_trusted_bootstrap(
    paired_trust: &SimulatedPairedTrust,
    expected_boot_nonce: BootNonce,
    material: &SimulatedBootstrapMaterial,
) -> Result<EstablishedTrustedBootstrap, LocalBootstrapError> {
    let assertion = BootstrapAssertion::parse_wire_value(&material.assertion_carrier)?;
    let verified = assertion.verify(&paired_trust.accepted)?;

    if verified.boot_nonce() != expected_boot_nonce {
        return Err(LocalBootstrapError::NonceMismatch);
    }

    Ok(EstablishedTrustedBootstrap {
        boot_nonce: verified.boot_nonce(),
        server_fingerprint: verified.server_fingerprint(),
        assertion_carrier: material.assertion_carrier.clone(),
    })
}

/// Result of the normal WP1 composition: the established WebSocket, plus
/// the [`EstablishedTrustedBootstrap`] material later code needs for
/// `BootstrapEvidence`.
pub struct TrustedBootstrapConnection<S> {
    pub websocket: WebSocketStream<S>,
    pub established: EstablishedTrustedBootstrap,
}

#[derive(Debug, thiserror::Error)]
pub enum ConnectAfterTrustedBootstrapError {
    #[error(transparent)]
    LocalBootstrap(#[from] LocalBootstrapError),
    #[error(transparent)]
    Transport(#[from] SimulatorTransportError),
}

/// The normal WP1 flow, and the required security ordering:
/// [`establish_trusted_bootstrap`] first, then use the resulting
/// authenticated `ServerCertFingerprint` — and only that — to pin
/// [`connect_pinned_wss`]. Never connect first and verify the assertion
/// later. Does not perform `AuthRequest`: transport establishment and Agent
/// Protocol authentication remain distinct steps.
pub async fn connect_after_trusted_bootstrap(
    addr: SocketAddr,
    server_name: &str,
    paired_trust: &SimulatedPairedTrust,
    expected_boot_nonce: BootNonce,
    material: &SimulatedBootstrapMaterial,
) -> Result<TrustedBootstrapConnection<TlsStream<TcpStream>>, ConnectAfterTrustedBootstrapError> {
    let established = establish_trusted_bootstrap(paired_trust, expected_boot_nonce, material)?;
    let fingerprint = established.server_fingerprint();
    let websocket = connect_pinned_wss(addr, server_name, fingerprint).await?;
    Ok(TrustedBootstrapConnection {
        websocket,
        established,
    })
}
