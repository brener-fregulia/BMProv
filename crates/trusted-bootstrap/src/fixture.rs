//! Fixture-only Ed25519 assertion signing
//! (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
//! "Simulator contract" — "Key separation": the fixture issuer owns the
//! private Ed25519 signing key; the Simulated Agent and its paired-trust
//! state receive only accepted site public key(s)). Gated behind the
//! `fixture-signing` Cargo feature so ordinary consumers of this crate never
//! link signing-key material or a signing implementation into a production
//! code path.
//!
//! This is WP1 fixture/test support, not a production Server
//! assertion-signing service and not the real ADR-0011 pairing ceremony.

use std::fmt;

use ed25519_dalek::{Signer, SigningKey};

use crate::assertion::{build_transcript_v1, BootstrapAssertion};
use crate::boot_nonce::BootNonce;
use crate::fingerprint::ServerCertFingerprint;
use crate::site_key::{SiteKeyId, SitePublicKey};

#[derive(Debug, thiserror::Error)]
pub enum FixtureSignerError {
    #[error("failed to obtain secure randomness for fixture signing-key generation")]
    Entropy(#[source] getrandom::Error),
}

/// Owns the fixture Ed25519 private signing key. `Debug` is implemented
/// manually and never exposes the key bytes.
pub struct FixtureAssertionSigner {
    signing_key: SigningKey,
    site_key_id: SiteKeyId,
}

impl fmt::Debug for FixtureAssertionSigner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("FixtureAssertionSigner")
            .finish_non_exhaustive()
    }
}

impl FixtureAssertionSigner {
    /// Fresh CSPRNG-generated fixture signing key.
    pub fn generate() -> Result<Self, FixtureSignerError> {
        let mut seed = [0u8; 32];
        getrandom::getrandom(&mut seed).map_err(FixtureSignerError::Entropy)?;
        Ok(Self::from_seed(seed))
    }

    /// Deterministic construction from a 32-byte fixture seed, for
    /// reproducible fixtures and golden test vectors. Does not itself claim
    /// randomness.
    pub fn from_seed(seed: [u8; 32]) -> Self {
        let signing_key = SigningKey::from_bytes(&seed);
        let public_key = SitePublicKey::from_bytes(signing_key.verifying_key().to_bytes())
            .expect("an Ed25519 SigningKey always derives a valid VerifyingKey");
        let site_key_id = SiteKeyId::from_public_key(&public_key);
        Self {
            signing_key,
            site_key_id,
        }
    }

    pub fn public_key(&self) -> SitePublicKey {
        SitePublicKey::from_bytes(self.signing_key.verifying_key().to_bytes())
            .expect("an Ed25519 SigningKey always derives a valid VerifyingKey")
    }

    /// Signs the exact V1 transcript for `boot_nonce`/`server_fingerprint`.
    /// `SiteKeyId` is always derived from this signer's own public key —
    /// callers cannot supply a disagreeing `SiteKeyId`.
    pub fn sign_v1(
        &self,
        boot_nonce: BootNonce,
        server_fingerprint: ServerCertFingerprint,
    ) -> BootstrapAssertion {
        let transcript = build_transcript_v1(
            boot_nonce.as_bytes(),
            server_fingerprint.as_bytes(),
            self.site_key_id.as_bytes(),
        );
        let signature = self.signing_key.sign(&transcript);
        BootstrapAssertion::from_signed_transcript(transcript, signature.to_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_does_not_contain_signing_key_bytes() {
        let signer = FixtureAssertionSigner::from_seed([0x42; 32]);
        let debug_output = format!("{signer:?}");
        assert!(!debug_output.contains("0x42"));
        assert_eq!(debug_output, "FixtureAssertionSigner { .. }");
    }

    #[test]
    fn site_key_id_matches_the_signer_public_key() {
        let signer = FixtureAssertionSigner::from_seed([7u8; 32]);
        let assertion = signer.sign_v1(
            BootNonce::from_bytes([1u8; 32]),
            ServerCertFingerprint::from_sha256_digest([2u8; 32]),
        );
        let expected_site_key_id = SiteKeyId::from_public_key(&signer.public_key());
        assert_eq!(
            assertion.site_key_id_bytes(),
            *expected_site_key_id.as_bytes()
        );
    }
}
