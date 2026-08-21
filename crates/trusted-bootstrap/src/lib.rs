//! Bamep trusted-bootstrap shared contract implementation
//! (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
//! "Shared contract implementation boundary"): `BootNonce`,
//! `ServerCertFingerprint`, `SiteKeyId`, the site public-key representation,
//! and assertion-schema-V1 parsing/transcript/verification.
//!
//! This crate does not depend on `bamep-domain`, `bamep-server`,
//! `bamep-simulator`, `bamep-agent-protocol`, `tokio`, `rustls`, or any
//! WebSocket library. `bamep-simulator` depends on it, not the other way
//! around; later Domain/Server checkpoints may also depend on it. This is
//! not a generic `common`/`shared`/`utils` crate — it owns exactly the
//! trusted-bootstrap contract representations and operations above.
//!
//! Fixture-only Ed25519 signing ([`fixture`]) is gated behind the
//! non-default `fixture-signing` Cargo feature: ordinary consumers never
//! link private-key/signing material into a production code path.

mod assertion;
mod boot_nonce;
mod fingerprint;
mod site_key;
mod verify;

#[cfg(feature = "fixture-signing")]
pub mod fixture;

pub use assertion::{
    AssertionParseError, BootstrapAssertion, ASSERTION_LEN, CARRIER_LEN, TRANSCRIPT_LEN,
};
pub use boot_nonce::{BootNonce, BootNonceError, BOOT_NONCE_BYTES, BOOT_NONCE_WIRE_LEN};
pub use fingerprint::ServerCertFingerprint;
pub use site_key::{AcceptedSiteKeys, SiteKeyId, SitePublicKey, SitePublicKeyError};
pub use verify::{VerificationError, VerifiedBootstrapAssertion};
