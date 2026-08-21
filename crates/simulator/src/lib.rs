//! Bamep Simulator — Agent-side real Agent Protocol v1 WSS transport client
//! (`docs/specifications/m0-simulator-contract-and-validation-strategy.md`
//! "Simulator fidelity boundary": a Simulated Endpoint's Agent participant
//! must use the real WSS transport end-to-end, never an in-process fake).
//!
//! Scope: this checkpoint (Issue #17 WP1) proves only the transport
//! boundary — pinned exact-leaf-certificate TLS 1.3 Server-identity
//! verification strictly before the WebSocket Upgrade, and real Agent
//! Protocol JSON-over-WebSocket carriage. It does not implement
//! trusted-bootstrap fixture semantics, `EnrollmentService`, or the
//! production Agent Control Gateway handshake — those belong to later
//! checkpoints of Issue #17.
//!
//! Production dependency direction: `bamep-simulator` depends only on
//! `bamep-agent-protocol` for the wire model. It does not depend on
//! `bamep-domain` or `bamep-server`.

pub mod fingerprint;
pub mod transport;
pub mod verifier;

pub use fingerprint::ServerCertFingerprint;
pub use transport::{connect_pinned_wss, SimulatorTransportError};
pub use verifier::PinnedServerCertVerifier;
