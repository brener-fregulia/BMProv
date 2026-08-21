//! Site bootstrap trust-anchor representation
//! (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
//! "Site bootstrap signing key and SiteKeyId"): the raw 32-byte Ed25519
//! public key ([`SitePublicKey`]), its derived identifier ([`SiteKeyId`] =
//! SHA-256 of the exact raw public-key bytes), and the smallest
//! configuration-oriented already-accepted-key set ([`AcceptedSiteKeys`]) V1
//! verification needs. No PEM/SPKI/DER wrapper, no rotation protocol, no
//! TOFU: an assertion's `SiteKeyId` never causes a new public key to become
//! trusted — it only selects among keys already accepted here.

use ed25519_dalek::VerifyingKey;
use sha2::{Digest, Sha256};

#[derive(Clone, Copy, Debug)]
pub struct SitePublicKey {
    verifying_key: VerifyingKey,
}

#[derive(Debug, thiserror::Error)]
pub enum SitePublicKeyError {
    #[error("bytes are not a valid Ed25519 public key")]
    InvalidKey(#[source] ed25519_dalek::SignatureError),
}

impl SitePublicKey {
    /// Validates `bytes` as an Ed25519 public key according to the
    /// underlying implementation.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, SitePublicKeyError> {
        let verifying_key =
            VerifyingKey::from_bytes(&bytes).map_err(SitePublicKeyError::InvalidKey)?;
        Ok(Self { verifying_key })
    }

    pub fn as_bytes(&self) -> [u8; 32] {
        self.verifying_key.to_bytes()
    }

    pub(crate) fn verifying_key(&self) -> &VerifyingKey {
        &self.verifying_key
    }
}

impl PartialEq for SitePublicKey {
    fn eq(&self, other: &Self) -> bool {
        self.verifying_key.as_bytes() == other.verifying_key.as_bytes()
    }
}

impl Eq for SitePublicKey {}

/// `SHA-256(exact raw 32-byte Ed25519 public-key value)`. Covered by the V1
/// signature itself; selects among already-accepted keys, never causes one
/// to become trusted.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub struct SiteKeyId([u8; 32]);

impl SiteKeyId {
    pub fn from_public_key(key: &SitePublicKey) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(key.as_bytes());
        let mut bytes = [0u8; 32];
        bytes.copy_from_slice(hasher.finalize().as_slice());
        Self(bytes)
    }

    /// Parses a raw 32-byte candidate identifier, e.g. from an assertion's
    /// `site_key_id` field. Does not itself claim the identifier is trusted
    /// or corresponds to any accepted key.
    pub fn from_raw_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// The smallest immutable already-accepted-key set V1 verification needs
/// (`m0-trusted-bootstrap-and-server-fingerprint-contract.md` "Site
/// bootstrap signing key and SiteKeyId"). WP1 configures one accepted site
/// key; the representation does not prevent more than one later. No
/// persistence, asynchronous repository, rotation protocol, or pairing
/// ceremony — those remain out of scope for this contract-implementation
/// boundary.
pub struct AcceptedSiteKeys {
    keys: Vec<(SiteKeyId, SitePublicKey)>,
}

impl AcceptedSiteKeys {
    pub fn new(keys: impl IntoIterator<Item = SitePublicKey>) -> Self {
        let keys = keys
            .into_iter()
            .map(|key| (SiteKeyId::from_public_key(&key), key))
            .collect();
        Self { keys }
    }

    pub fn single(key: SitePublicKey) -> Self {
        Self::new([key])
    }

    pub(crate) fn find(&self, id: &SiteKeyId) -> Option<&SitePublicKey> {
        self.keys
            .iter()
            .find(|(candidate_id, _)| candidate_id == id)
            .map(|(_, key)| key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_site_key_id_from_exact_raw_public_key() {
        // A `SitePublicKey` is derived from an `ed25519_dalek::SigningKey`
        // seed here, matching how every other caller in this crate obtains
        // one.
        let signing_key = ed25519_dalek::SigningKey::from_bytes(&[3u8; 32]);
        let key = SitePublicKey::from_bytes(signing_key.verifying_key().to_bytes())
            .expect("SigningKey always derives a valid VerifyingKey");
        let id_a = SiteKeyId::from_public_key(&key);
        let id_b = SiteKeyId::from_public_key(&key);
        assert_eq!(id_a, id_b);
    }

    #[test]
    fn different_public_keys_produce_different_ids() {
        let key_a = ed25519_dalek::SigningKey::from_bytes(&[3u8; 32]);
        let key_b = ed25519_dalek::SigningKey::from_bytes(&[4u8; 32]);
        let a = SitePublicKey::from_bytes(key_a.verifying_key().to_bytes()).unwrap();
        let b = SitePublicKey::from_bytes(key_b.verifying_key().to_bytes()).unwrap();
        assert_ne!(
            SiteKeyId::from_public_key(&a),
            SiteKeyId::from_public_key(&b)
        );
    }

    #[test]
    fn accepted_site_keys_finds_by_id() {
        let key = ed25519_dalek::SigningKey::from_bytes(&[5u8; 32]);
        let public = SitePublicKey::from_bytes(key.verifying_key().to_bytes()).unwrap();
        let id = SiteKeyId::from_public_key(&public);
        let accepted = AcceptedSiteKeys::single(public);
        assert!(accepted.find(&id).is_some());
    }

    #[test]
    fn accepted_site_keys_rejects_unknown_id() {
        let key = ed25519_dalek::SigningKey::from_bytes(&[6u8; 32]);
        let public = SitePublicKey::from_bytes(key.verifying_key().to_bytes()).unwrap();
        let accepted = AcceptedSiteKeys::single(public);
        let unrelated_id = SiteKeyId::from_raw_bytes([0xAA; 32]);
        assert!(accepted.find(&unrelated_id).is_none());
    }
}
