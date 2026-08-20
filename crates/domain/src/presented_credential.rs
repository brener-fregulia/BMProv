//! Self-locating presented-credential representation
//! (`docs/decisions/0014-agent-credential-lookup-and-boot-context-correlation.md`;
//! `docs/specifications/m0-endpoint-identity-lifecycle.md` "Credential chain,
//! rotation, and revocation").
//!
//! ADR-0014 establishes that a presented credential — whether an unresolved
//! boot-scoped enrollment credential or a runtime credential — internally
//! carries a non-secret **lookup identifier** alongside its high-entropy
//! **secret material**, and that a presented value's credential kind and
//! lookup identifier must always be determined unambiguously, without
//! consulting the secret. This module defines that representation and its
//! wire (de)serialization only. It does not decide how the lookup identifier
//! is indexed in PostgreSQL, how `BootContext` is persisted, or how routing
//! selects a credential chain to verify against — those remain later
//! checkpoints.
//!
//! # Wire encoding
//!
//! ```text
//! v1.<kind>.<lookup_id>.<secret>
//! ```
//!
//! - `v1` — the fixed representation-version tag for this encoding. A future
//!   incompatible representation uses a different tag; [`parse`](PresentedCredential::parse)
//!   rejects every other value with [`PresentedCredentialParseError::UnsupportedVersion`].
//! - `<kind>` — `enrollment` or `runtime` ([`CredentialKind`]).
//! - `<lookup_id>` — 16 cryptographically random bytes, `base64url` without
//!   padding ([`CredentialLookupId`]). Non-secret: it only narrows which
//!   persisted chain/`BootContext` to check, never authenticates by itself.
//! - `<secret>` — 32 cryptographically random bytes, `base64url` without
//!   padding ([`PresentedCredentialSecret`]). High-entropy authentication
//!   material.
//!
//! `.` is never produced by `base64url` output, so splitting the wire value
//! on `.` and requiring exactly four non-empty segments is unambiguous. The
//! whole value contains no whitespace and uses only URL-safe characters, so
//! it transports as-is inside a JSON string (e.g. Agent Protocol's opaque
//! `AuthRequest.credential` / `SessionEstablished.runtime_credential`).
//!
//! This is a small project-owned encoding, not a token standard: there is no
//! signature, no embedded expiry, and no claim of self-verification. A
//! presented value is only ever *evidence of format* — actual authentication
//! still requires verifying the secret against a persisted one-way verifier
//! (ADR-0012 point 10; ADR-0014 point 2), which this module does not perform.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use rand::RngCore;
use std::fmt;

/// Fixed wire-representation-version tag understood by [`PresentedCredential::parse`].
/// A future incompatible representation must use a different tag rather than
/// changing the meaning of this one.
pub const WIRE_VERSION: &str = "v1";

const LOOKUP_ID_LEN: usize = 16;
const SECRET_LEN: usize = 32;

/// Distinguishes an unresolved boot-scoped enrollment credential (E1/E2, ADR-0014
/// point 3) from a runtime credential belonging to an already-persisted
/// Endpoint credential chain (ADR-0014 point 2).
///
/// This is credential-lifecycle vocabulary, not commercial product
/// vocabulary (`docs/decisions/0015-...`), and intentionally has exactly two
/// variants: E1/E2 are enrollment credentials from different boots, and
/// R1/R2/... are runtime credentials across rotations, but their sequence
/// number is not part of the credential kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CredentialKind {
    Enrollment,
    Runtime,
}

impl CredentialKind {
    fn to_wire(self) -> &'static str {
        match self {
            CredentialKind::Enrollment => "enrollment",
            CredentialKind::Runtime => "runtime",
        }
    }

    fn from_wire(s: &str) -> Option<Self> {
        match s {
            "enrollment" => Some(CredentialKind::Enrollment),
            "runtime" => Some(CredentialKind::Runtime),
            _ => None,
        }
    }
}

/// The non-secret, unpredictable lookup/routing key carried by a presented
/// credential (ADR-0014 point 2/3). 16 bytes of secure random data — enough
/// entropy that accidental collision or practical enumeration is not a
/// realistic concern at Bamep's scale, while staying compact on the wire.
///
/// Deliberately not derived from `EndpointId`, MAC, any other inventory
/// signal, customer/site information, or a counter (ADR-0014 "Alternatives
/// considered": "Using MAC/inventory signal as authentication" — rejected).
///
/// Non-secret: safe to log, index, and derive `Debug` for. Possession of a
/// lookup identifier alone never authenticates anything (ADR-0014 point 7).
#[derive(Clone, PartialEq, Eq)]
pub struct CredentialLookupId([u8; LOOKUP_ID_LEN]);

impl CredentialLookupId {
    /// Generates a fresh lookup identifier using the OS CSPRNG.
    pub fn generate() -> Self {
        let mut bytes = [0u8; LOOKUP_ID_LEN];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    fn to_wire(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    fn from_wire(s: &str) -> Result<Self, PresentedCredentialParseError> {
        let decoded = URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|_| PresentedCredentialParseError::InvalidLookupIdEncoding)?;
        let bytes: [u8; LOOKUP_ID_LEN] = decoded
            .try_into()
            .map_err(|_| PresentedCredentialParseError::InvalidLookupIdLength)?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for CredentialLookupId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("CredentialLookupId")
            .field(&self.to_wire())
            .finish()
    }
}

/// High-entropy secret material carried by a presented credential (ADR-0014
/// point 2/3). 32 bytes of secure random data, generated the same way the
/// existing transitional `CredentialSecret` is (`credential::CredentialSecret::generate`).
///
/// Never derives `Debug`/`Display` — those would print the raw secret bytes
/// through ordinary logging or `{:?}` formatting. Use
/// [`PresentedCredential::to_wire_value`] as the one deliberate
/// serialization boundary instead.
#[derive(Clone, PartialEq, Eq)]
pub struct PresentedCredentialSecret([u8; SECRET_LEN]);

impl PresentedCredentialSecret {
    /// Generates fresh secret material using the OS CSPRNG.
    pub fn generate() -> Self {
        let mut bytes = [0u8; SECRET_LEN];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        Self(bytes)
    }

    /// Raw secret bytes, for a future checkpoint that verifies this material
    /// against a persisted one-way verifier. An explicit, deliberately named
    /// accessor rather than `Deref`/`AsRef`, so every read site is visibly a
    /// secret-handling call.
    pub fn expose_secret_bytes(&self) -> &[u8; SECRET_LEN] {
        &self.0
    }

    fn to_wire(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    fn from_wire(s: &str) -> Result<Self, PresentedCredentialParseError> {
        let decoded = URL_SAFE_NO_PAD
            .decode(s)
            .map_err(|_| PresentedCredentialParseError::InvalidSecretEncoding)?;
        let bytes: [u8; SECRET_LEN] = decoded
            .try_into()
            .map_err(|_| PresentedCredentialParseError::InvalidSecretLength)?;
        Ok(Self(bytes))
    }
}

impl fmt::Debug for PresentedCredentialSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("PresentedCredentialSecret")
            .field(&"REDACTED")
            .finish()
    }
}

/// Rejects a wire value that is malformed or uses an unsupported/unknown
/// representation version or credential kind. Deliberately does not carry
/// the offending input or any decoded field value — only which structural
/// property failed — so that a caller cannot use a `Debug`/`Display` of the
/// error to reconstruct secret-bearing input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum PresentedCredentialParseError {
    #[error("unsupported credential wire representation version")]
    UnsupportedVersion,
    #[error("unknown credential kind")]
    UnknownKind,
    #[error("malformed credential wire value")]
    MalformedStructure,
    #[error("invalid lookup identifier encoding")]
    InvalidLookupIdEncoding,
    #[error("invalid lookup identifier length")]
    InvalidLookupIdLength,
    #[error("invalid secret encoding")]
    InvalidSecretEncoding,
    #[error("invalid secret length")]
    InvalidSecretLength,
}

/// A presented credential's parsed Domain representation: credential kind,
/// non-secret lookup identifier, and secret material kept as three distinct
/// fields (ADR-0014 point 2/3) — never collapsed into one opaque blob inside
/// Domain merely because the wire representation is one string.
///
/// This type does not authenticate anything by itself. It only makes the
/// kind and lookup identifier available to a future routing/lookup
/// checkpoint without exposing or depending on the secret to do so, and
/// keeps the secret available separately for that same future checkpoint's
/// one-way verification step.
#[derive(Clone, PartialEq, Eq)]
pub struct PresentedCredential {
    kind: CredentialKind,
    lookup_id: CredentialLookupId,
    secret: PresentedCredentialSecret,
}

impl PresentedCredential {
    /// Generates a fresh presented credential of the given kind: a new
    /// random lookup identifier and new random secret material.
    pub fn generate(kind: CredentialKind) -> Self {
        Self {
            kind,
            lookup_id: CredentialLookupId::generate(),
            secret: PresentedCredentialSecret::generate(),
        }
    }

    pub fn kind(&self) -> CredentialKind {
        self.kind
    }

    pub fn lookup_id(&self) -> &CredentialLookupId {
        &self.lookup_id
    }

    pub fn secret(&self) -> &PresentedCredentialSecret {
        &self.secret
    }

    /// Serializes into the one opaque wire string transported as Agent
    /// Protocol's opaque `credential`/`runtime_credential` value. An
    /// explicit method rather than `Display`, so printing a
    /// `PresentedCredential` (e.g. accidentally via `{}`) cannot happen
    /// without deliberately calling this.
    pub fn to_wire_value(&self) -> String {
        format!(
            "{WIRE_VERSION}.{}.{}.{}",
            self.kind.to_wire(),
            self.lookup_id.to_wire(),
            self.secret.to_wire(),
        )
    }

    /// Parses a wire value produced by [`to_wire_value`](Self::to_wire_value).
    /// Deterministic and requires no I/O or machine-local state: any
    /// malformed or unsupported value is rejected explicitly rather than
    /// best-effort interpreted.
    pub fn parse(wire: &str) -> Result<Self, PresentedCredentialParseError> {
        let segments: Vec<&str> = wire.split('.').collect();
        let [version, kind, lookup_id, secret] = segments.as_slice() else {
            return Err(PresentedCredentialParseError::MalformedStructure);
        };
        if version.is_empty() || kind.is_empty() || lookup_id.is_empty() || secret.is_empty() {
            return Err(PresentedCredentialParseError::MalformedStructure);
        }
        if *version != WIRE_VERSION {
            return Err(PresentedCredentialParseError::UnsupportedVersion);
        }
        let kind =
            CredentialKind::from_wire(kind).ok_or(PresentedCredentialParseError::UnknownKind)?;
        let lookup_id = CredentialLookupId::from_wire(lookup_id)?;
        let secret = PresentedCredentialSecret::from_wire(secret)?;
        Ok(Self {
            kind,
            lookup_id,
            secret,
        })
    }
}

impl fmt::Debug for PresentedCredential {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PresentedCredential")
            .field("kind", &self.kind)
            .field("lookup_id", &self.lookup_id)
            .field("secret", &"REDACTED")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_enrollment_credential_round_trips() {
        let credential = PresentedCredential::generate(CredentialKind::Enrollment);
        let wire = credential.to_wire_value();
        let parsed = PresentedCredential::parse(&wire).expect("must parse");
        assert_eq!(parsed.kind(), CredentialKind::Enrollment);
        assert_eq!(parsed.lookup_id(), credential.lookup_id());
        assert_eq!(parsed.secret(), credential.secret());
    }

    #[test]
    fn generate_runtime_credential_round_trips() {
        let credential = PresentedCredential::generate(CredentialKind::Runtime);
        let wire = credential.to_wire_value();
        let parsed = PresentedCredential::parse(&wire).expect("must parse");
        assert_eq!(parsed.kind(), CredentialKind::Runtime);
        assert_eq!(parsed.lookup_id(), credential.lookup_id());
        assert_eq!(parsed.secret(), credential.secret());
    }

    #[test]
    fn round_trip_wire_value_starts_with_version_tag() {
        let credential = PresentedCredential::generate(CredentialKind::Runtime);
        assert!(credential.to_wire_value().starts_with("v1.runtime."));
    }

    #[test]
    fn enrollment_and_runtime_kinds_remain_distinguishable() {
        let enrollment = PresentedCredential::generate(CredentialKind::Enrollment);
        let runtime = PresentedCredential::generate(CredentialKind::Runtime);
        assert_ne!(enrollment.kind(), runtime.kind());
        assert!(enrollment.to_wire_value().contains(".enrollment."));
        assert!(runtime.to_wire_value().contains(".runtime."));
    }

    #[test]
    fn separate_generated_credentials_use_different_lookup_ids_and_secrets() {
        let a = PresentedCredential::generate(CredentialKind::Runtime);
        let b = PresentedCredential::generate(CredentialKind::Runtime);
        assert_ne!(a.lookup_id(), b.lookup_id());
        assert_ne!(a.secret(), b.secret());
    }

    #[test]
    fn malformed_wire_value_missing_segments_rejected() {
        assert_eq!(
            PresentedCredential::parse("v1.runtime.onlyonefield"),
            Err(PresentedCredentialParseError::MalformedStructure)
        );
    }

    #[test]
    fn malformed_wire_value_extra_segments_rejected() {
        let credential = PresentedCredential::generate(CredentialKind::Runtime);
        let extra = format!("{}.unexpected", credential.to_wire_value());
        assert_eq!(
            PresentedCredential::parse(&extra),
            Err(PresentedCredentialParseError::MalformedStructure)
        );
    }

    #[test]
    fn empty_wire_value_rejected() {
        assert_eq!(
            PresentedCredential::parse(""),
            Err(PresentedCredentialParseError::MalformedStructure)
        );
    }

    #[test]
    fn empty_field_rejected() {
        let credential = PresentedCredential::generate(CredentialKind::Runtime);
        let wire = credential.to_wire_value();
        let secret_wire = credential.secret().to_wire();
        // Remove the secret bytes, leaving an empty last segment:
        // "v1.runtime.ABCDE.FGHIJ" → "v1.runtime.ABCDE."
        let with_empty_secret = wire.replacen(&secret_wire, "", 1);
        // Sanity: this replace must have actually produced a 4-segment,
        // empty-last-field string for the assertion below to be meaningful.
        assert_eq!(with_empty_secret.split('.').count(), 4);
        assert_eq!(
            PresentedCredential::parse(&with_empty_secret),
            Err(PresentedCredentialParseError::MalformedStructure)
        );
    }

    #[test]
    fn unknown_version_rejected() {
        let credential = PresentedCredential::generate(CredentialKind::Runtime);
        let wire = credential.to_wire_value();
        let tampered = wire.replacen("v1.", "v2.", 1);
        assert_eq!(
            PresentedCredential::parse(&tampered),
            Err(PresentedCredentialParseError::UnsupportedVersion)
        );
    }

    #[test]
    fn unknown_kind_rejected() {
        let credential = PresentedCredential::generate(CredentialKind::Runtime);
        let wire = credential.to_wire_value();
        let tampered = wire.replacen(".runtime.", ".bogus.", 1);
        assert_eq!(
            PresentedCredential::parse(&tampered),
            Err(PresentedCredentialParseError::UnknownKind)
        );
    }

    #[test]
    fn invalid_lookup_id_encoding_rejected() {
        let credential = PresentedCredential::generate(CredentialKind::Runtime);
        let lookup_id_wire = credential.lookup_id().to_wire();
        let wire = credential.to_wire_value();
        let tampered = wire.replacen(&lookup_id_wire, "not!valid!base64", 1);
        assert_eq!(
            PresentedCredential::parse(&tampered),
            Err(PresentedCredentialParseError::InvalidLookupIdEncoding)
        );
    }

    #[test]
    fn invalid_lookup_id_length_rejected() {
        let credential = PresentedCredential::generate(CredentialKind::Runtime);
        let lookup_id_wire = credential.lookup_id().to_wire();
        let wire = credential.to_wire_value();
        // Valid base64url, but decodes to fewer than 16 bytes.
        let short = URL_SAFE_NO_PAD.encode([0u8; 8]);
        let tampered = wire.replacen(&lookup_id_wire, &short, 1);
        assert_eq!(
            PresentedCredential::parse(&tampered),
            Err(PresentedCredentialParseError::InvalidLookupIdLength)
        );
    }

    #[test]
    fn invalid_secret_encoding_rejected() {
        let credential = PresentedCredential::generate(CredentialKind::Runtime);
        let secret_wire = credential.secret().to_wire();
        let wire = credential.to_wire_value();
        let tampered = wire.replacen(&secret_wire, "not!valid!base64", 1);
        assert_eq!(
            PresentedCredential::parse(&tampered),
            Err(PresentedCredentialParseError::InvalidSecretEncoding)
        );
    }

    #[test]
    fn invalid_secret_length_rejected() {
        let credential = PresentedCredential::generate(CredentialKind::Runtime);
        let secret_wire = credential.secret().to_wire();
        let wire = credential.to_wire_value();
        // Valid base64url, but decodes to fewer than 32 bytes.
        let short = URL_SAFE_NO_PAD.encode([0u8; 16]);
        let tampered = wire.replacen(&secret_wire, &short, 1);
        assert_eq!(
            PresentedCredential::parse(&tampered),
            Err(PresentedCredentialParseError::InvalidSecretLength)
        );
    }

    #[test]
    fn redacted_debug_does_not_contain_plaintext_secret() {
        let credential = PresentedCredential::generate(CredentialKind::Runtime);
        let secret_wire = credential.secret().to_wire();
        let debug_output = format!("{credential:?}");
        assert!(!debug_output.contains(&secret_wire));
        assert!(debug_output.contains("REDACTED"));

        let secret_debug_output = format!("{:?}", credential.secret());
        assert!(!secret_debug_output.contains(&secret_wire));
        assert!(secret_debug_output.contains("REDACTED"));
    }
}
