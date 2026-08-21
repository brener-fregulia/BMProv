//! Bamep Simulator — Agent-side real Agent Protocol v1 WSS transport client
//! (`docs/specifications/m0-simulator-contract-and-validation-strategy.md`
//! "Simulator fidelity boundary": a Simulated Endpoint's Agent participant
//! must use the real WSS transport end-to-end, never an in-process fake).
//!
//! Scope: the transport checkpoint (Issue #17 WP1) proved the transport
//! boundary — pinned exact-leaf-certificate TLS 1.3 Server-identity
//! verification strictly before the WebSocket Upgrade, and real Agent
//! Protocol JSON-over-WebSocket carriage. This checkpoint adds the
//! Simulator-side handshake helper ([`handshake::authenticate`]) that sends
//! `AuthRequest` and validates the `SessionEstablished`/`AuthError` response
//! over an already-established WSS connection. It does not implement
//! trusted-bootstrap fixture semantics or `BootstrapEvidence` processing —
//! those belong to a later checkpoint of Issue #17.
//!
//! Production dependency direction: `bamep-simulator` depends only on
//! `bamep-agent-protocol` for the wire model. It does not depend on
//! `bamep-domain` or `bamep-server`.

pub mod fingerprint;
pub mod handshake;
pub mod transport;
pub mod verifier;

pub use fingerprint::ServerCertFingerprint;
pub use handshake::{authenticate, SimulatorHandshakeError, SimulatorHandshakeOutcome};
pub use transport::{connect_pinned_wss, SimulatorTransportError};
pub use verifier::PinnedServerCertVerifier;
