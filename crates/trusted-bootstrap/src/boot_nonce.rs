//! `BootNonce` V1 — the trusted-bootstrap freshness primitive
//! (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
//! "(C) Authenticated and fresh bootstrap material" / "BootNonce V1"): exactly
//! 32 random bytes generated fresh for every genuine new boot context from
//! the operating-system CSPRNG, never a UUID. Its only valid wire
//! representation is RFC 4648 base64url without padding, exactly 43 ASCII
//! characters, parsed strictly — no silent normalization.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

pub const BOOT_NONCE_BYTES: usize = 32;
pub const BOOT_NONCE_WIRE_LEN: usize = 43;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct BootNonce([u8; BOOT_NONCE_BYTES]);

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum BootNonceError {
    #[error("failed to obtain secure randomness for BootNonce generation")]
    Entropy(#[source] getrandom::Error),
    #[error("BootNonce wire value has the wrong length")]
    WireLength,
    #[error(
        "BootNonce wire value contains characters outside the canonical base64url-no-pad alphabet"
    )]
    InvalidCharacters,
    #[error("BootNonce wire value failed base64url decoding")]
    InvalidEncoding,
    #[error("BootNonce wire value did not decode to exactly 32 bytes")]
    InvalidDecodedLength,
    #[error("BootNonce wire value is not the canonical re-encoding of its own bytes")]
    NonCanonical,
}

impl BootNonce {
    /// Fresh CSPRNG-generated nonce for a genuine new boot context. Failure
    /// to obtain secure randomness propagates — there is no insecure PRNG
    /// fallback.
    pub fn generate() -> Result<Self, BootNonceError> {
        let mut bytes = [0u8; BOOT_NONCE_BYTES];
        getrandom::getrandom(&mut bytes).map_err(BootNonceError::Entropy)?;
        Ok(Self(bytes))
    }

    /// Deterministic construction for fixtures/tests. Does not itself claim
    /// randomness.
    pub fn from_bytes(bytes: [u8; BOOT_NONCE_BYTES]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; BOOT_NONCE_BYTES] {
        &self.0
    }

    pub fn to_wire_value(&self) -> String {
        URL_SAFE_NO_PAD.encode(self.0)
    }

    /// Strict parsing: exact 43-character length, canonical base64url
    /// alphabet only (no `=`, `+`, `/`, whitespace), and a canonical
    /// re-encode-and-compare against the input before accepting.
    pub fn parse_wire_value(value: &str) -> Result<Self, BootNonceError> {
        if value.len() != BOOT_NONCE_WIRE_LEN {
            return Err(BootNonceError::WireLength);
        }
        if !value.bytes().all(is_canonical_base64url_char) {
            return Err(BootNonceError::InvalidCharacters);
        }
        let decoded = URL_SAFE_NO_PAD
            .decode(value)
            .map_err(|_| BootNonceError::InvalidEncoding)?;
        let bytes: [u8; BOOT_NONCE_BYTES] = decoded
            .try_into()
            .map_err(|_| BootNonceError::InvalidDecodedLength)?;
        let candidate = Self(bytes);
        if candidate.to_wire_value() != value {
            return Err(BootNonceError::NonCanonical);
        }
        Ok(candidate)
    }
}

fn is_canonical_base64url_char(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn thirty_two_bytes_are_preserved_exactly() {
        let bytes = [7u8; BOOT_NONCE_BYTES];
        let nonce = BootNonce::from_bytes(bytes);
        assert_eq!(nonce.as_bytes(), &bytes);
    }

    #[test]
    fn canonical_wire_round_trips() {
        let nonce = BootNonce::from_bytes([9u8; BOOT_NONCE_BYTES]);
        let wire = nonce.to_wire_value();
        assert_eq!(wire.len(), BOOT_NONCE_WIRE_LEN);
        let parsed = BootNonce::parse_wire_value(&wire).expect("canonical wire value parses");
        assert_eq!(parsed, nonce);
    }

    #[test]
    fn generate_produces_a_parseable_canonical_wire_value() {
        let nonce = BootNonce::generate().expect("OS CSPRNG must be available in tests");
        let wire = nonce.to_wire_value();
        assert_eq!(BootNonce::parse_wire_value(&wire), Ok(nonce));
    }

    #[test]
    fn padding_is_rejected() {
        let nonce = BootNonce::from_bytes([1u8; BOOT_NONCE_BYTES]);
        let mut wire = nonce.to_wire_value();
        wire.push('=');
        assert_eq!(
            BootNonce::parse_wire_value(&wire),
            Err(BootNonceError::WireLength)
        );
    }

    #[test]
    fn standard_base64_alphabet_is_rejected() {
        // 43 canonical-length characters, but containing '+' and '/'.
        let mut wire: Vec<u8> = vec![b'A'; BOOT_NONCE_WIRE_LEN];
        wire[0] = b'+';
        wire[1] = b'/';
        let wire = String::from_utf8(wire).unwrap();
        assert_eq!(
            BootNonce::parse_wire_value(&wire),
            Err(BootNonceError::InvalidCharacters)
        );
    }

    #[test]
    fn whitespace_is_rejected() {
        let nonce = BootNonce::from_bytes([2u8; BOOT_NONCE_BYTES]);
        let mut wire = nonce.to_wire_value();
        wire.replace_range(0..1, " ");
        assert_eq!(
            BootNonce::parse_wire_value(&wire),
            Err(BootNonceError::InvalidCharacters)
        );
    }

    #[test]
    fn wrong_length_is_rejected() {
        assert_eq!(
            BootNonce::parse_wire_value("short"),
            Err(BootNonceError::WireLength)
        );
    }

    #[test]
    fn non_canonical_trailing_bits_are_rejected() {
        // A 43-char base64url string whose last character encodes non-zero
        // trailing bits beyond the 32 real payload bytes is not the
        // canonical re-encoding of any 32-byte value. The `base64` decoder
        // itself already rejects this at decode time (`InvalidEncoding`);
        // the crate's own re-encode-and-compare check is a defense-in-depth
        // backstop that would independently catch it as `NonCanonical` if
        // the decoder ever became lenient.
        let nonce = BootNonce::from_bytes([0u8; BOOT_NONCE_BYTES]);
        let mut wire = nonce.to_wire_value();
        assert_eq!(wire.pop(), Some('A'));
        wire.push('B');
        assert_eq!(
            BootNonce::parse_wire_value(&wire),
            Err(BootNonceError::InvalidEncoding)
        );
    }
}
