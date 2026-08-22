//! Assertion schema V1 exact wire representation
//! (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
//! "Exact assertion-schema-V1 signed representation", "Agent Protocol
//! assertion carrier"). [`BootstrapAssertion`] retains the exact 197
//! canonical assertion bytes and exposes only parsed contract fields through
//! safe accessors; parsing is strict/canonical, never best-effort. Unknown
//! schema versions and wrong domain discriminators are rejected explicitly.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

/// Exact 33-byte ASCII domain/contract discriminator.
pub const DOMAIN_DISCRIMINATOR: &[u8; 33] = b"bamep.trusted-bootstrap.assertion";
pub const SCHEMA_VERSION: u16 = 1;

/// `u16be(33) || domain[33] || u16be(1) || boot_nonce[32] ||
/// expected_server_fingerprint[32] || site_key_id[32]`.
pub const TRANSCRIPT_LEN: usize = 133;
pub const SIGNATURE_LEN: usize = 64;
/// `signed_payload_v1[133] || ed25519_signature[64]`.
pub const ASSERTION_LEN: usize = TRANSCRIPT_LEN + SIGNATURE_LEN;
/// Canonical RFC 4648 base64url-without-padding encoding of `ASSERTION_LEN`
/// bytes.
pub const CARRIER_LEN: usize = 263;

const NONCE_OFFSET: usize = 37;
const FINGERPRINT_OFFSET: usize = 69;
const SITE_KEY_ID_OFFSET: usize = 101;

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct BootstrapAssertion {
    bytes: [u8; ASSERTION_LEN],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AssertionParseError {
    #[error("assertion carrier has the wrong length")]
    CarrierLength,
    #[error(
        "assertion carrier contains characters outside the canonical base64url-no-pad alphabet"
    )]
    CarrierCharacters,
    #[error("assertion carrier failed base64url decoding")]
    CarrierEncoding,
    #[error("assertion carrier is not the canonical re-encoding of its own bytes")]
    CarrierNonCanonical,
    #[error("decoded assertion has the wrong length")]
    AssertionLength,
    #[error("assertion domain-discriminator length prefix is not 33")]
    DomainLength,
    #[error("assertion domain discriminator does not match the expected contract discriminator")]
    DomainDiscriminator,
    #[error("assertion schema version is not supported")]
    UnsupportedSchemaVersion,
}

impl BootstrapAssertion {
    /// Strict carrier parsing (`m0-trusted-bootstrap-and-server-fingerprint-
    /// contract.md` "Agent Protocol assertion carrier"): exact 263-character
    /// canonical base64url-no-pad carrier, canonical re-encode-and-compare,
    /// exact V1 length/layout, domain discriminator, and schema version.
    /// Never best-effort.
    pub fn parse_wire_value(carrier: &str) -> Result<Self, AssertionParseError> {
        if carrier.len() != CARRIER_LEN {
            return Err(AssertionParseError::CarrierLength);
        }
        if !carrier.bytes().all(is_canonical_base64url_char) {
            return Err(AssertionParseError::CarrierCharacters);
        }
        let decoded = URL_SAFE_NO_PAD
            .decode(carrier)
            .map_err(|_| AssertionParseError::CarrierEncoding)?;
        let bytes: [u8; ASSERTION_LEN] = decoded
            .try_into()
            .map_err(|_| AssertionParseError::AssertionLength)?;
        let candidate = Self { bytes };
        if candidate.to_wire_value() != carrier {
            return Err(AssertionParseError::CarrierNonCanonical);
        }
        candidate.validate_header()?;
        Ok(candidate)
    }

    fn validate_header(&self) -> Result<(), AssertionParseError> {
        let domain_len = u16::from_be_bytes([self.bytes[0], self.bytes[1]]);
        if domain_len != DOMAIN_DISCRIMINATOR.len() as u16 {
            return Err(AssertionParseError::DomainLength);
        }
        if &self.bytes[2..35] != DOMAIN_DISCRIMINATOR.as_slice() {
            return Err(AssertionParseError::DomainDiscriminator);
        }
        let schema_version = u16::from_be_bytes([self.bytes[35], self.bytes[36]]);
        if schema_version != SCHEMA_VERSION {
            return Err(AssertionParseError::UnsupportedSchemaVersion);
        }
        Ok(())
    }

    pub fn to_wire_value(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.bytes)
    }

    pub fn boot_nonce_bytes(&self) -> [u8; 32] {
        self.bytes[NONCE_OFFSET..NONCE_OFFSET + 32]
            .try_into()
            .expect("fixed 32-byte slice")
    }

    pub fn server_fingerprint_bytes(&self) -> [u8; 32] {
        self.bytes[FINGERPRINT_OFFSET..FINGERPRINT_OFFSET + 32]
            .try_into()
            .expect("fixed 32-byte slice")
    }

    pub fn site_key_id_bytes(&self) -> [u8; 32] {
        self.bytes[SITE_KEY_ID_OFFSET..SITE_KEY_ID_OFFSET + 32]
            .try_into()
            .expect("fixed 32-byte slice")
    }

    pub fn signature_bytes(&self) -> [u8; SIGNATURE_LEN] {
        self.bytes[TRANSCRIPT_LEN..]
            .try_into()
            .expect("fixed 64-byte slice")
    }

    pub fn transcript_bytes(&self) -> &[u8; TRANSCRIPT_LEN] {
        (&self.bytes[..TRANSCRIPT_LEN])
            .try_into()
            .expect("fixed 133-byte slice")
    }

    /// Builds the complete assertion from an already-computed exact V1
    /// transcript and its Ed25519 signature over that transcript. Crate-
    /// internal only: the fixture signer is the sole caller, so an
    /// externally-supplied unchecked assertion can never bypass
    /// [`BootstrapAssertion::parse_wire_value`]'s strict parsing.
    #[cfg_attr(not(any(test, feature = "fixture-signing")), allow(dead_code))]
    pub(crate) fn from_signed_transcript(
        transcript: [u8; TRANSCRIPT_LEN],
        signature: [u8; SIGNATURE_LEN],
    ) -> Self {
        let mut bytes = [0u8; ASSERTION_LEN];
        bytes[..TRANSCRIPT_LEN].copy_from_slice(&transcript);
        bytes[TRANSCRIPT_LEN..].copy_from_slice(&signature);
        Self { bytes }
    }
}

/// Builds the exact 133-byte V1 signed transcript. Shared by the fixture
/// signer (to sign it) and by parsing (to reconstruct it for verification).
#[cfg_attr(not(any(test, feature = "fixture-signing")), allow(dead_code))]
pub(crate) fn build_transcript_v1(
    boot_nonce: &[u8; 32],
    server_fingerprint: &[u8; 32],
    site_key_id: &[u8; 32],
) -> [u8; TRANSCRIPT_LEN] {
    let mut buf = [0u8; TRANSCRIPT_LEN];
    buf[0..2].copy_from_slice(&(DOMAIN_DISCRIMINATOR.len() as u16).to_be_bytes());
    buf[2..35].copy_from_slice(DOMAIN_DISCRIMINATOR.as_slice());
    buf[35..37].copy_from_slice(&SCHEMA_VERSION.to_be_bytes());
    buf[NONCE_OFFSET..NONCE_OFFSET + 32].copy_from_slice(boot_nonce);
    buf[FINGERPRINT_OFFSET..FINGERPRINT_OFFSET + 32].copy_from_slice(server_fingerprint);
    buf[SITE_KEY_ID_OFFSET..SITE_KEY_ID_OFFSET + 32].copy_from_slice(site_key_id);
    buf
}

fn is_canonical_base64url_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_transcript() -> [u8; TRANSCRIPT_LEN] {
        build_transcript_v1(&[1u8; 32], &[2u8; 32], &[3u8; 32])
    }

    #[test]
    fn transcript_length_is_exactly_133() {
        assert_eq!(sample_transcript().len(), TRANSCRIPT_LEN);
        assert_eq!(TRANSCRIPT_LEN, 133);
    }

    #[test]
    fn transcript_field_offsets_are_exact() {
        let transcript = sample_transcript();
        assert_eq!(u16::from_be_bytes([transcript[0], transcript[1]]), 33);
        assert_eq!(&transcript[2..35], DOMAIN_DISCRIMINATOR.as_slice());
        assert_eq!(u16::from_be_bytes([transcript[35], transcript[36]]), 1);
        assert_eq!(&transcript[37..69], &[1u8; 32]);
        assert_eq!(&transcript[69..101], &[2u8; 32]);
        assert_eq!(&transcript[101..133], &[3u8; 32]);
    }

    #[test]
    fn assertion_and_carrier_lengths_are_exact() {
        assert_eq!(ASSERTION_LEN, 197);
        assert_eq!(CARRIER_LEN, 263);
        let assertion =
            BootstrapAssertion::from_signed_transcript(sample_transcript(), [9u8; SIGNATURE_LEN]);
        assert_eq!(assertion.to_wire_value().len(), CARRIER_LEN);
    }

    #[test]
    fn strict_carrier_round_trips() {
        let assertion =
            BootstrapAssertion::from_signed_transcript(sample_transcript(), [9u8; SIGNATURE_LEN]);
        let carrier = assertion.to_wire_value();
        let parsed = BootstrapAssertion::parse_wire_value(&carrier).expect("round trip parses");
        assert_eq!(parsed, assertion);
    }

    #[test]
    fn trailing_bytes_are_rejected() {
        let assertion =
            BootstrapAssertion::from_signed_transcript(sample_transcript(), [9u8; SIGNATURE_LEN]);
        let mut carrier = assertion.to_wire_value();
        carrier.push_str("AAAA");
        assert_eq!(
            BootstrapAssertion::parse_wire_value(&carrier),
            Err(AssertionParseError::CarrierLength)
        );
    }

    #[test]
    fn truncated_assertion_is_rejected() {
        let assertion =
            BootstrapAssertion::from_signed_transcript(sample_transcript(), [9u8; SIGNATURE_LEN]);
        let carrier = assertion.to_wire_value();
        let truncated = &carrier[..carrier.len() - 4];
        assert_eq!(
            BootstrapAssertion::parse_wire_value(truncated),
            Err(AssertionParseError::CarrierLength)
        );
    }

    #[test]
    fn wrong_domain_discriminator_is_rejected() {
        let wrong_transcript = {
            let mut t = sample_transcript();
            t[2] = b'X';
            t
        };
        let assertion =
            BootstrapAssertion::from_signed_transcript(wrong_transcript, [9u8; SIGNATURE_LEN]);
        let carrier = assertion.to_wire_value();
        assert_eq!(
            BootstrapAssertion::parse_wire_value(&carrier),
            Err(AssertionParseError::DomainDiscriminator)
        );
    }

    #[test]
    fn unsupported_schema_version_is_rejected() {
        let wrong_transcript = {
            let mut t = sample_transcript();
            t[35..37].copy_from_slice(&2u16.to_be_bytes());
            t
        };
        let assertion =
            BootstrapAssertion::from_signed_transcript(wrong_transcript, [9u8; SIGNATURE_LEN]);
        let carrier = assertion.to_wire_value();
        assert_eq!(
            BootstrapAssertion::parse_wire_value(&carrier),
            Err(AssertionParseError::UnsupportedSchemaVersion)
        );
    }

    #[test]
    fn malformed_carrier_is_rejected() {
        assert_eq!(
            BootstrapAssertion::parse_wire_value("not base64url at all!!"),
            Err(AssertionParseError::CarrierLength)
        );
    }

    #[test]
    fn padding_in_carrier_is_rejected() {
        let assertion =
            BootstrapAssertion::from_signed_transcript(sample_transcript(), [9u8; SIGNATURE_LEN]);
        let mut carrier = assertion.to_wire_value();
        carrier.replace_range(carrier.len() - 1..carrier.len(), "=");
        assert_eq!(
            BootstrapAssertion::parse_wire_value(&carrier),
            Err(AssertionParseError::CarrierCharacters)
        );
    }
}
