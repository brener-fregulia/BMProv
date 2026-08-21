//! `ServerCertFingerprint` — the authenticated expected Server TLS identity
//! for Agent Protocol v1
//! (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
//! "Bootstrap material"): SHA-256 over the exact DER bytes of the
//! leaf/end-entity certificate the Server presents during the TLS
//! handshake — a 32-byte digest. This is exact-certificate pinning: not an
//! SPKI/public-key pin, not a CA/chain fingerprint, and not a
//! hostname-derived identity.
//!
//! A fingerprint is not a secret, so ordinary equality is sufficient — no
//! algorithm enum, and no textual (hex/base64) internal representation.

use ring::digest::{digest, SHA256};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ServerCertFingerprint {
    digest: [u8; 32],
}

impl ServerCertFingerprint {
    /// Computes the fingerprint from the exact DER bytes of a leaf/
    /// end-entity certificate.
    pub fn from_leaf_der(der: &[u8]) -> Self {
        let computed = digest(&SHA256, der);
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(computed.as_ref());
        Self { digest: bytes }
    }

    /// The raw 32-byte SHA-256 digest.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.digest
    }

    /// Whether `der` (an exact leaf/end-entity certificate) hashes to this
    /// fingerprint.
    pub fn matches_leaf_der(&self, der: &[u8]) -> bool {
        Self::from_leaf_der(der) == *self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_der_bytes_match() {
        let der = b"not a real certificate, just bytes for the digest";
        let fingerprint = ServerCertFingerprint::from_leaf_der(der);
        assert!(fingerprint.matches_leaf_der(der));
    }

    #[test]
    fn different_der_bytes_do_not_match() {
        let a = ServerCertFingerprint::from_leaf_der(b"certificate A");
        assert!(!a.matches_leaf_der(b"certificate B"));
    }
}
