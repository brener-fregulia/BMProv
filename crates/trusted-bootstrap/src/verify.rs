//! Strict Ed25519 assertion verification
//! (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
//! "(C) Authenticated and fresh bootstrap material", "Site bootstrap signing
//! key and SiteKeyId"): [`VerifiedBootstrapAssertion`] is constructible only
//! through [`BootstrapAssertion::verify`] — there is no public unchecked
//! constructor. Successful verification here proves only signature/trust-
//! anchor/schema validity, never that the assertion belongs to the caller's
//! current boot — that boot-context correlation remains the caller's
//! responsibility (Simulator now, Server/Domain later).

use ed25519_dalek::Signature;

use crate::assertion::BootstrapAssertion;
use crate::boot_nonce::BootNonce;
use crate::fingerprint::ServerCertFingerprint;
use crate::site_key::{AcceptedSiteKeys, SiteKeyId};

#[derive(Debug, thiserror::Error)]
pub enum VerificationError {
    #[error("assertion SiteKeyId does not select any already-accepted site key")]
    UnknownSiteKey,
    #[error("recomputed SiteKeyId of the selected accepted key does not match the assertion")]
    SiteKeyIdMismatch,
    #[error("Ed25519 signature verification failed")]
    SignatureInvalid,
}

/// Verified assertion contents. Boot-context-scoped freshness (`boot_nonce`)
/// against the caller's own current boot is not checked here — this type
/// proves only cryptographic/trust-anchor validity.
#[derive(Clone, Copy, Debug)]
pub struct VerifiedBootstrapAssertion {
    boot_nonce: BootNonce,
    expected_server_fingerprint: ServerCertFingerprint,
    site_key_id: SiteKeyId,
}

impl VerifiedBootstrapAssertion {
    pub fn boot_nonce(&self) -> BootNonce {
        self.boot_nonce
    }

    pub fn server_fingerprint(&self) -> ServerCertFingerprint {
        self.expected_server_fingerprint
    }

    pub fn site_key_id(&self) -> SiteKeyId {
        self.site_key_id
    }
}

impl BootstrapAssertion {
    /// Strict V1 verification ordering: the assertion must already be
    /// canonically parsed (guaranteed by [`BootstrapAssertion`] itself);
    /// `SiteKeyId` selects a key from `accepted`; that key's own recomputed
    /// `SiteKeyId` is required to match (defense-in-depth against a lookup
    /// bug); the exact 133-byte transcript is verified under
    /// `verify_strict`, which rejects non-canonical/weak-key signatures.
    pub fn verify(
        &self,
        accepted: &AcceptedSiteKeys,
    ) -> Result<VerifiedBootstrapAssertion, VerificationError> {
        let claimed_site_key_id = SiteKeyId::from_raw_bytes(self.site_key_id_bytes());
        let accepted_key = accepted
            .find(&claimed_site_key_id)
            .ok_or(VerificationError::UnknownSiteKey)?;

        if SiteKeyId::from_public_key(accepted_key) != claimed_site_key_id {
            return Err(VerificationError::SiteKeyIdMismatch);
        }

        let signature = Signature::from_bytes(&self.signature_bytes());
        accepted_key
            .verifying_key()
            .verify_strict(self.transcript_bytes(), &signature)
            .map_err(|_| VerificationError::SignatureInvalid)?;

        Ok(VerifiedBootstrapAssertion {
            boot_nonce: BootNonce::from_bytes(self.boot_nonce_bytes()),
            expected_server_fingerprint: ServerCertFingerprint::from_sha256_digest(
                self.server_fingerprint_bytes(),
            ),
            site_key_id: claimed_site_key_id,
        })
    }
}

#[cfg(test)]
mod tests {
    use ed25519_dalek::{Signer, SigningKey};

    use super::*;
    use crate::assertion::build_transcript_v1;
    use crate::site_key::SitePublicKey;

    fn signer_and_key(seed: [u8; 32]) -> (SigningKey, SitePublicKey) {
        let signing_key = SigningKey::from_bytes(&seed);
        let public_key = SitePublicKey::from_bytes(signing_key.verifying_key().to_bytes())
            .expect("valid Ed25519 public key");
        (signing_key, public_key)
    }

    fn sign_assertion(
        signing_key: &SigningKey,
        site_key_id: SiteKeyId,
        boot_nonce: [u8; 32],
        fingerprint: [u8; 32],
    ) -> BootstrapAssertion {
        let transcript = build_transcript_v1(&boot_nonce, &fingerprint, site_key_id.as_bytes());
        let signature = signing_key.sign(&transcript);
        BootstrapAssertion::from_signed_transcript(transcript, signature.to_bytes())
    }

    #[test]
    fn valid_ed25519_assertion_verifies() {
        let (signing_key, public_key) = signer_and_key([1u8; 32]);
        let site_key_id = SiteKeyId::from_public_key(&public_key);
        let assertion = sign_assertion(&signing_key, site_key_id, [2u8; 32], [3u8; 32]);
        let accepted = AcceptedSiteKeys::single(public_key);

        let verified = assertion
            .verify(&accepted)
            .expect("valid assertion verifies");
        assert_eq!(verified.boot_nonce(), BootNonce::from_bytes([2u8; 32]));
        assert_eq!(
            verified.server_fingerprint(),
            ServerCertFingerprint::from_sha256_digest([3u8; 32])
        );
        assert_eq!(verified.site_key_id(), site_key_id);
    }

    #[test]
    fn wrong_accepted_signer_is_rejected() {
        let (signing_key, _signer_public_key) = signer_and_key([1u8; 32]);
        let (_other_signing_key, unrelated_accepted_key) = signer_and_key([9u8; 32]);
        let site_key_id = SiteKeyId::from_public_key(&unrelated_accepted_key);

        // Signed by [1u8;32]'s key, but the transcript claims a SiteKeyId
        // belonging to the unrelated [9u8;32] key, and only that unrelated
        // key is accepted.
        let assertion = sign_assertion(&signing_key, site_key_id, [2u8; 32], [3u8; 32]);
        let accepted = AcceptedSiteKeys::single(unrelated_accepted_key);

        assert!(matches!(
            assertion.verify(&accepted),
            Err(VerificationError::SignatureInvalid)
        ));
    }

    #[test]
    fn unknown_site_key_id_is_rejected() {
        let (signing_key, public_key) = signer_and_key([1u8; 32]);
        let site_key_id = SiteKeyId::from_public_key(&public_key);
        let assertion = sign_assertion(&signing_key, site_key_id, [2u8; 32], [3u8; 32]);

        let (_unrelated_signing_key, unrelated_accepted_key) = signer_and_key([9u8; 32]);
        let accepted = AcceptedSiteKeys::single(unrelated_accepted_key);

        assert!(matches!(
            assertion.verify(&accepted),
            Err(VerificationError::UnknownSiteKey)
        ));
    }

    #[test]
    fn corrupted_signature_is_rejected() {
        let (signing_key, public_key) = signer_and_key([1u8; 32]);
        let site_key_id = SiteKeyId::from_public_key(&public_key);
        let assertion = sign_assertion(&signing_key, site_key_id, [2u8; 32], [3u8; 32]);
        let accepted = AcceptedSiteKeys::single(public_key);

        let mut corrupted_signature = assertion.signature_bytes();
        corrupted_signature[0] ^= 0xFF;
        let corrupted = BootstrapAssertion::from_signed_transcript(
            *assertion.transcript_bytes(),
            corrupted_signature,
        );

        assert!(matches!(
            corrupted.verify(&accepted),
            Err(VerificationError::SignatureInvalid)
        ));
    }
}
