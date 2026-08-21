//! Server-side WSS transport foundation for Agent Protocol v1
//! (`docs/specifications/m0-agent-protocol-contract.md` "Transport and
//! handshake"): TLS 1.3 must fully terminate — including ordinary TLS
//! handshake signature verification proving possession of the configured
//! certificate's private key — *before* the WebSocket Upgrade is even
//! attempted.
//!
//! This is a transport-oriented Adapter boundary only. It does not parse
//! `AuthRequest` or any other Agent Protocol message, does not call
//! `EnrollmentService`, and does not know Domain or PostgreSQL — the
//! Agent Control Gateway that consumes the [`WebSocketStream`] this module
//! returns is a later checkpoint.
//!
//! Certificate/private-key material is accepted already-loaded
//! (`rustls::pki_types` types): production certificate-file configuration
//! is out of scope for this checkpoint (`m0-trusted-bootstrap-and-server-
//! fingerprint-contract.md` "Rotation, revocation, and recovery" — Server
//! certificate identity must be externally configured and stable across
//! ordinary restarts; this module does not decide how that material is
//! loaded from disk).

use std::sync::Arc;

use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::ServerConfig;
use tokio::net::TcpStream;
use tokio_rustls::server::TlsStream;
use tokio_rustls::TlsAcceptor;
use tokio_tungstenite::WebSocketStream;

/// Exactly TLS 1.3 — no TLS 1.2 negotiation (`m0-agent-protocol-contract.md`
/// "Transport and handshake": "Agent Protocol v1 does not negotiate TLS 1.2
/// or earlier"). Exposed so both the real `ServerConfig` construction below
/// and a narrow config-level test use the identical, single source value.
pub const SUPPORTED_TLS_VERSIONS: &[&rustls::SupportedProtocolVersion] = &[&rustls::version::TLS13];

#[derive(Debug, thiserror::Error)]
pub enum AgentTransportError {
    #[error("failed to build TLS server configuration")]
    TlsConfig(#[source] rustls::Error),
    #[error("TLS handshake failed")]
    TlsHandshake(#[source] std::io::Error),
    #[error("WebSocket upgrade failed")]
    WebSocketUpgrade(#[source] tokio_tungstenite::tungstenite::Error),
}

/// The TLS 1.3 -> WebSocket Upgrade transport boundary for one Server
/// listener. Construction fixes the Server's certificate identity for the
/// lifetime of this acceptor, consistent with production certificate
/// identity being stable across ordinary restarts.
pub struct AgentTransportAcceptor {
    tls_acceptor: TlsAcceptor,
}

impl AgentTransportAcceptor {
    /// Builds the acceptor from already-loaded certificate chain / private
    /// key material, using rustls's `ring` `CryptoProvider` explicitly
    /// (no process-global `CryptoProvider::install_default()`), TLS 1.3
    /// only, and no client-certificate authentication (Agent authentication
    /// is a credential, not mTLS — `m0-agent-protocol-contract.md`).
    pub fn new(
        cert_chain: Vec<CertificateDer<'static>>,
        private_key: PrivateKeyDer<'static>,
    ) -> Result<Self, AgentTransportError> {
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = ServerConfig::builder_with_provider(provider)
            .with_protocol_versions(SUPPORTED_TLS_VERSIONS)
            .map_err(AgentTransportError::TlsConfig)?
            .with_no_client_auth()
            .with_single_cert(cert_chain, private_key)
            .map_err(AgentTransportError::TlsConfig)?;

        Ok(Self {
            tls_acceptor: TlsAcceptor::from(Arc::new(config)),
        })
    }

    /// Accepts one already-connected `TcpStream`. TLS 1.3 must succeed —
    /// including the peer proving possession of the certificate's private
    /// key — before the WebSocket Upgrade is attempted; a TLS failure never
    /// reaches WebSocket or Agent Protocol code.
    pub async fn accept(
        &self,
        stream: TcpStream,
    ) -> Result<WebSocketStream<TlsStream<TcpStream>>, AgentTransportError> {
        let tls_stream = self
            .tls_acceptor
            .accept(stream)
            .await
            .map_err(AgentTransportError::TlsHandshake)?;

        let ws_stream = tokio_tungstenite::accept_async(tls_stream)
            .await
            .map_err(AgentTransportError::WebSocketUpgrade)?;

        Ok(ws_stream)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Narrow config-level proof, in addition to the real cross-crate WSS
    /// TLS 1.3 positive test (`bamep-simulator`'s
    /// `tests/wss_transport.rs`): the Server transport is configured for
    /// exactly TLS 1.3, never TLS 1.2 or earlier.
    #[test]
    fn server_transport_configures_tls13_only() {
        assert_eq!(SUPPORTED_TLS_VERSIONS, &[&rustls::version::TLS13]);
    }
}
