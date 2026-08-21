//! Runtime Agent credential issuance, rotation, and reconnect recovery.
//!
//! Implements the model accepted in ADR-0012
//! (`docs/decisions/0012-runtime-agent-credential-issuance-rotation-and-reconnect-recovery.md`)
//! and normatively defined in
//! `docs/specifications/m0-endpoint-identity-lifecycle.md` "Credential chain,
//! rotation, and revocation".
//!
//! This module is pure: it never touches the clock or randomness directly.
//! Callers pass `now` explicitly and supply freshly generated secrets, so the
//! state-transition logic stays deterministic and unit-testable.

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

/// A plaintext credential secret, held only transiently: generated for the
/// Agent, sent over the wire, and never persisted in this form (ADR-0012
/// point 10, "No recoverable-secret requirement").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CredentialSecret(pub String);

impl CredentialSecret {
    fn generate() -> Self {
        let mut bytes = [0u8; 32];
        rand::rngs::OsRng.fill_bytes(&mut bytes);
        Self(URL_SAFE_NO_PAD.encode(bytes))
    }
}

/// The durable, salted-hash representation of a credential secret
/// (ADR-0012 point 10: "a salted hash verified against a presented value").
/// Never reversible to the original secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialHash {
    salt: [u8; 16],
    digest: [u8; 32],
}

impl CredentialHash {
    fn of(secret: &CredentialSecret) -> Self {
        Self::of_bytes(secret.0.as_bytes())
    }

    /// Constant-time verification, so timing does not leak how many bytes of
    /// a guessed secret matched.
    fn verify(&self, secret: &CredentialSecret) -> bool {
        self.verify_bytes(secret.0.as_bytes())
    }

    /// Byte-oriented one-way verifier construction (ADR-0014 point 4): hashes
    /// the actual secret bytes directly, with no intermediate text encoding.
    /// For a secret representation that is not itself text (e.g.
    /// `PresentedCredentialSecret`'s raw bytes), this avoids the pointless
    /// round-trip through base64 that wrapping it in the transitional
    /// [`CredentialSecret`] would require.
    pub fn of_bytes(secret_bytes: &[u8]) -> Self {
        let mut salt = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut salt);
        Self::of_bytes_with_salt(secret_bytes, salt)
    }

    fn of_bytes_with_salt(secret_bytes: &[u8], salt: [u8; 16]) -> Self {
        use sha2::Digest;
        let mut hasher = Sha256::new();
        hasher.update(salt);
        hasher.update(secret_bytes);
        let digest: [u8; 32] = hasher.finalize().into();
        Self { salt, digest }
    }

    /// Constant-time verification against raw secret bytes, so timing does
    /// not leak how many bytes of a guessed secret matched.
    pub fn verify_bytes(&self, secret_bytes: &[u8]) -> bool {
        let candidate = Self::of_bytes_with_salt(secret_bytes, self.salt);
        candidate.digest.ct_eq(&self.digest).into()
    }

    /// Opaque byte representation for relational persistence (ADR-0013 Sec.6:
    /// an "opaque credential/assertion blob" is an accepted non-relational
    /// column even under a relational-first schema). Never reversible to the
    /// original secret — salt and digest are already one-way outputs.
    pub fn to_bytes(&self) -> [u8; 48] {
        let mut out = [0u8; 48];
        out[..16].copy_from_slice(&self.salt);
        out[16..].copy_from_slice(&self.digest);
        out
    }

    /// Reconstructs a hash from its durable byte representation
    /// (`to_bytes`). Adapter-facing only: never called from any transition
    /// or authentication logic in this module.
    pub fn from_bytes(bytes: [u8; 48]) -> Self {
        let mut salt = [0u8; 16];
        let mut digest = [0u8; 32];
        salt.copy_from_slice(&bytes[..16]);
        digest.copy_from_slice(&bytes[16..]);
        Self { salt, digest }
    }
}

/// One credential in the chain: its durable hash plus its validity window
/// (ADR-0012 point 4, "Bounded valid set").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialSlot {
    pub hash: CredentialHash,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl CredentialSlot {
    fn valid_at(&self, now: DateTime<Utc>) -> bool {
        now < self.expires_at
    }
}

/// Default runtime-credential validity window. The exact numeric grace/expiry
/// duration is explicitly implementation-time
/// (`m0-endpoint-identity-lifecycle.md` "Open questions" item 2; ADR-0012
/// "Consequences"). One hour is a conservative placeholder proportionate to
/// Bamep's controlled-LAN, infrequent-reconnect V1 threat model; it is not an
/// architectural commitment and may be tuned freely without revisiting
/// ADR-0012.
pub const DEFAULT_CREDENTIAL_TTL: Duration = Duration::hours(1);

/// The durable credential chain for one Endpoint: at most one predecessor in
/// grace, plus at most one current unconfirmed successor (ADR-0012 point 4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialChain {
    predecessor: CredentialSlot,
    successor: Option<CredentialSlot>,
    revoked: bool,
}

/// The credential/session lifecycle dimension
/// (`m0-endpoint-identity-lifecycle.md` "Credential/session lifecycle
/// (independent of identity lifecycle)"), derived from the chain rather than
/// stored independently, so it can never drift out of sync with the chain it
/// summarizes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CredentialDimension {
    NoActiveCredential,
    CredentialActive,
    CredentialExpired,
    CredentialRevoked,
}

impl CredentialChain {
    /// Establishes a fresh chain from a first-contact credential redemption
    /// (either the boot-scoped enrollment credential, or — for a reconnecting
    /// known Endpoint whose chain was somehow lost — a fresh restart of the
    /// chain). The presented credential becomes the predecessor; a fresh
    /// successor is minted immediately, since "every successful `AuthRequest`
    /// ... issues a fresh runtime credential" (ADR-0012 point 1) with no
    /// exception for the first exchange.
    pub fn establish(
        presented: CredentialSecret,
        now: DateTime<Utc>,
        ttl: Duration,
    ) -> (Self, CredentialSecret) {
        let predecessor = CredentialSlot {
            hash: CredentialHash::of(&presented),
            issued_at: now,
            expires_at: now + ttl,
        };
        let issued_secret = CredentialSecret::generate();
        let successor = CredentialSlot {
            hash: CredentialHash::of(&issued_secret),
            issued_at: now,
            expires_at: now + ttl,
        };
        (
            Self {
                predecessor,
                successor: Some(successor),
                revoked: false,
            },
            issued_secret,
        )
    }

    pub fn dimension(&self, now: DateTime<Utc>) -> CredentialDimension {
        if self.revoked {
            return CredentialDimension::CredentialRevoked;
        }
        let predecessor_valid = self.predecessor.valid_at(now);
        let successor_valid = self.successor.as_ref().is_some_and(|s| s.valid_at(now));
        if predecessor_valid || successor_valid {
            CredentialDimension::CredentialActive
        } else {
            CredentialDimension::CredentialExpired
        }
    }

    /// Explicit `CredentialRevoked`: invalidates every credential still valid
    /// in the chain at that instant (ADR-0012 point 8).
    pub fn revoke(&self) -> Self {
        Self {
            predecessor: self.predecessor.clone(),
            successor: self.successor.clone(),
            revoked: true,
        }
    }

    /// Read-only access to the chain's parts, for a relational Adapter to
    /// decompose a chain into its durable columns. No transition or
    /// authentication decision may be reimplemented from these accessors
    /// outside this module (`AGENTS.md` "Architecture and dependencies").
    pub fn predecessor(&self) -> &CredentialSlot {
        &self.predecessor
    }

    pub fn successor(&self) -> Option<&CredentialSlot> {
        self.successor.as_ref()
    }

    pub fn is_revoked(&self) -> bool {
        self.revoked
    }

    /// Reconstructs a chain from its durable parts. Adapter-facing only: a
    /// relational Adapter reloading a previously committed row is the only
    /// legitimate caller — this constructor trusts the caller to hand back
    /// exactly what a prior `establish`/`authenticate`/`revoke` call
    /// produced, and enforces no invariant of its own beyond the type
    /// signature.
    pub fn from_parts(
        predecessor: CredentialSlot,
        successor: Option<CredentialSlot>,
        revoked: bool,
    ) -> Self {
        Self {
            predecessor,
            successor,
            revoked,
        }
    }
}

/// Outcome of presenting a credential in a fresh `AuthRequest` against an
/// existing chain (ADR-0012 points 2-7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthOutcome {
    /// The presented credential matched the predecessor (superseding any
    /// prior unconfirmed successor) or matched the unconfirmed successor
    /// (confirming it). Either way, a fresh successor is issued and must be
    /// delivered via `SessionEstablished`.
    Accepted {
        chain: CredentialChain,
        issued: CredentialSecret,
        issued_expires_at: DateTime<Utc>,
        /// True only when the presented credential was the previously
        /// unconfirmed successor authenticating for the first time
        /// (ADR-0012 point 5, "Confirmation semantics").
        successor_confirmed: bool,
    },
    /// Generic rejection. Deliberately does not distinguish "expired
    /// predecessor" from "already-superseded successor" from "unknown value"
    /// from "revoked chain" (ADR-0012 point 6; `AuthError` minimal-disclosure
    /// precedent).
    Rejected,
}

/// Presents `presented` against `chain` in a fresh `AuthRequest`.
pub fn authenticate(
    chain: &CredentialChain,
    presented: &CredentialSecret,
    now: DateTime<Utc>,
    ttl: Duration,
) -> AuthOutcome {
    if chain.revoked {
        return AuthOutcome::Rejected;
    }

    if chain.predecessor.valid_at(now) && chain.predecessor.hash.verify(presented) {
        // Predecessor re-presented: supersede any unconfirmed successor
        // (ADR-0012 point 3) and mint a fresh one. Never reconstruct or
        // redeliver the old successor.
        let issued_secret = CredentialSecret::generate();
        let issued_slot = CredentialSlot {
            hash: CredentialHash::of(&issued_secret),
            issued_at: now,
            expires_at: now + ttl,
        };
        let new_chain = CredentialChain {
            predecessor: chain.predecessor.clone(),
            successor: Some(issued_slot.clone()),
            revoked: false,
        };
        return AuthOutcome::Accepted {
            chain: new_chain,
            issued: issued_secret,
            issued_expires_at: issued_slot.expires_at,
            successor_confirmed: false,
        };
    }

    if let Some(successor) = &chain.successor {
        if successor.valid_at(now) && successor.hash.verify(presented) {
            // Successor confirmed: it becomes the new predecessor, the old
            // predecessor is retired, and a fresh successor is minted for the
            // next rotation (ADR-0012 point 5).
            let issued_secret = CredentialSecret::generate();
            let issued_slot = CredentialSlot {
                hash: CredentialHash::of(&issued_secret),
                issued_at: now,
                expires_at: now + ttl,
            };
            let new_chain = CredentialChain {
                predecessor: successor.clone(),
                successor: Some(issued_slot.clone()),
                revoked: false,
            };
            return AuthOutcome::Accepted {
                chain: new_chain,
                issued: issued_secret,
                issued_expires_at: issued_slot.expires_at,
                successor_confirmed: true,
            };
        }
    }

    AuthOutcome::Rejected
}

/// Boot-scoped enrollment credential issuance and redemption
/// (`docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md` point
/// 2; ADR-0012 point 1). Represented as a self-verifying signed token so the
/// Boot Orchestrator can issue it before any Endpoint record exists, without
/// requiring the Server to pre-persist a row for a device it has not yet
/// heard from.
pub mod enrollment {
    use super::*;

    /// Server-held HMAC key backing enrollment-token issuance and
    /// verification. Boot Orchestration is an Application-level Server
    /// responsibility (`m0-stack-and-boundaries-baseline.md` "Component
    /// responsibilities and boundaries"), so the Server legitimately holds
    /// this key; it is never shared with an Endpoint.
    #[derive(Clone)]
    pub struct SigningKey(pub [u8; 32]);

    impl SigningKey {
        pub fn generate() -> Self {
            let mut bytes = [0u8; 32];
            rand::rngs::OsRng.fill_bytes(&mut bytes);
            Self(bytes)
        }
    }

    /// Issues a fresh boot-scoped enrollment credential, standing in for the
    /// (Simulator-faked, in production PXE-delivered) Boot Orchestrator
    /// handing a booting endpoint its short-lived credential.
    pub fn issue(key: &SigningKey, now: DateTime<Utc>, ttl: Duration) -> CredentialSecret {
        let mut nonce = [0u8; 16];
        rand::rngs::OsRng.fill_bytes(&mut nonce);
        let expires_at = (now + ttl).timestamp();

        let mut payload = Vec::with_capacity(24);
        payload.extend_from_slice(&nonce);
        payload.extend_from_slice(&expires_at.to_be_bytes());

        let mut mac = HmacSha256::new_from_slice(&key.0).expect("HMAC accepts any key length");
        mac.update(&payload);
        let tag = mac.finalize().into_bytes();

        let token = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(payload),
            URL_SAFE_NO_PAD.encode(tag)
        );
        CredentialSecret(token)
    }

    /// Verifies a presented value is a still-valid, correctly-signed
    /// enrollment token. Used only on first contact for a given
    /// `inventory_signal` — subsequent redemptions are validated against the
    /// persisted chain via [`authenticate`] instead.
    pub fn verify(key: &SigningKey, presented: &CredentialSecret, now: DateTime<Utc>) -> bool {
        let Some((payload_b64, tag_b64)) = presented.0.split_once('.') else {
            return false;
        };
        let Ok(payload) = URL_SAFE_NO_PAD.decode(payload_b64) else {
            return false;
        };
        let Ok(tag) = URL_SAFE_NO_PAD.decode(tag_b64) else {
            return false;
        };
        if payload.len() != 24 {
            return false;
        }

        let mut mac = HmacSha256::new_from_slice(&key.0).expect("HMAC accepts any key length");
        mac.update(&payload);
        if mac.verify_slice(&tag).is_err() {
            return false;
        }

        let expires_at_bytes: [u8; 8] = payload[16..24].try_into().expect("checked length above");
        let expires_at = i64::from_be_bytes(expires_at_bytes);
        now.timestamp() < expires_at
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    #[test]
    fn establish_creates_predecessor_and_fresh_successor() {
        let presented = CredentialSecret("enrollment-e1".into());
        let (chain, issued) = CredentialChain::establish(presented, now(), DEFAULT_CREDENTIAL_TTL);
        assert_eq!(
            chain.dimension(now()),
            CredentialDimension::CredentialActive
        );
        assert_ne!(issued.0, "enrollment-e1");
    }

    #[test]
    fn presenting_predecessor_again_supersedes_unconfirmed_successor() {
        let e1 = CredentialSecret("e1".into());
        let (chain, r1) = CredentialChain::establish(e1.clone(), now(), DEFAULT_CREDENTIAL_TTL);

        // E1 reappears before R1 ever authenticated (e.g. dropped connection
        // between commit and delivery).
        let outcome = authenticate(&chain, &e1, now(), DEFAULT_CREDENTIAL_TTL);
        match outcome {
            AuthOutcome::Accepted {
                chain: new_chain,
                issued: r1_prime,
                successor_confirmed,
                ..
            } => {
                assert!(!successor_confirmed);
                assert_ne!(r1_prime, r1, "must never redeliver the original successor");
                // The superseded R1 must no longer authenticate.
                assert_eq!(
                    authenticate(&new_chain, &r1, now(), DEFAULT_CREDENTIAL_TTL),
                    AuthOutcome::Rejected
                );
                // The fresh successor does authenticate (confirming it).
                assert!(matches!(
                    authenticate(&new_chain, &r1_prime, now(), DEFAULT_CREDENTIAL_TTL),
                    AuthOutcome::Accepted {
                        successor_confirmed: true,
                        ..
                    }
                ));
            }
            AuthOutcome::Rejected => panic!("predecessor must still authenticate"),
        }
    }

    #[test]
    fn successor_confirmation_retires_predecessor_and_rotates() {
        let e1 = CredentialSecret("e1".into());
        let (chain, r1) = CredentialChain::establish(e1.clone(), now(), DEFAULT_CREDENTIAL_TTL);

        let outcome = authenticate(&chain, &r1, now(), DEFAULT_CREDENTIAL_TTL);
        let (new_chain, r2) = match outcome {
            AuthOutcome::Accepted {
                chain,
                issued,
                successor_confirmed,
                ..
            } => {
                assert!(successor_confirmed);
                (chain, issued)
            }
            AuthOutcome::Rejected => panic!("successor must authenticate"),
        };

        // E1 (the old predecessor) must now be rejected: it was retired on
        // confirmation.
        assert_eq!(
            authenticate(&new_chain, &e1, now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Rejected
        );
        // R1 is now the predecessor and still authenticates (retry path).
        assert!(matches!(
            authenticate(&new_chain, &r1, now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Accepted {
                successor_confirmed: false,
                ..
            }
        ));
        // R2 (the freshly minted successor) confirms in turn.
        assert!(matches!(
            authenticate(&new_chain, &r2, now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Accepted {
                successor_confirmed: true,
                ..
            }
        ));
    }

    #[test]
    fn recovery_by_retrying_still_valid_predecessor_after_superseded_successor() {
        let e1 = CredentialSecret("e1".into());
        let (chain, r1) = CredentialChain::establish(e1.clone(), now(), DEFAULT_CREDENTIAL_TTL);

        // E1 retried (as if R1 delivery was lost), producing R1'.
        let (chain2, r1_prime) = match authenticate(&chain, &e1, now(), DEFAULT_CREDENTIAL_TTL) {
            AuthOutcome::Accepted { chain, issued, .. } => (chain, issued),
            AuthOutcome::Rejected => panic!(),
        };

        // Agent still holds the now-superseded R1 and tries it: generic
        // rejection, not a distinguishing error.
        assert_eq!(
            authenticate(&chain2, &r1, now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Rejected
        );

        // Agent falls back to E1 (still valid, within grace): recovers.
        assert!(matches!(
            authenticate(&chain2, &e1, now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Accepted { .. }
        ));

        let _ = r1_prime;
    }

    #[test]
    fn expired_predecessor_and_successor_are_rejected() {
        let e1 = CredentialSecret("e1".into());
        let past = now() - Duration::hours(2);
        let (chain, r1) = CredentialChain::establish(e1.clone(), past, DEFAULT_CREDENTIAL_TTL);

        assert_eq!(
            chain.dimension(now()),
            CredentialDimension::CredentialExpired
        );
        assert_eq!(
            authenticate(&chain, &e1, now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Rejected
        );
        assert_eq!(
            authenticate(&chain, &r1, now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Rejected
        );
    }

    #[test]
    fn revocation_invalidates_every_credential_in_the_chain() {
        let e1 = CredentialSecret("e1".into());
        let (chain, r1) = CredentialChain::establish(e1.clone(), now(), DEFAULT_CREDENTIAL_TTL);
        let revoked = chain.revoke();

        assert_eq!(
            revoked.dimension(now()),
            CredentialDimension::CredentialRevoked
        );
        assert_eq!(
            authenticate(&revoked, &e1, now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Rejected
        );
        assert_eq!(
            authenticate(&revoked, &r1, now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Rejected
        );
    }

    #[test]
    fn unrelated_garbage_credential_is_rejected() {
        let e1 = CredentialSecret("e1".into());
        let (chain, _r1) = CredentialChain::establish(e1, now(), DEFAULT_CREDENTIAL_TTL);
        let garbage = CredentialSecret("not-a-real-credential".into());
        assert_eq!(
            authenticate(&chain, &garbage, now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Rejected
        );
    }

    #[test]
    fn concurrent_redemption_last_commit_wins_and_prior_session_not_retroactively_invalidated() {
        // ADR-0012 point 7: an already-established WSS session is not
        // retroactively invalidated merely because the credential issued to
        // it for a future reconnect was superseded by a concurrent
        // redemption. This is modeled here at the chain level: two
        // authentications both accepted against the same starting chain
        // (representing two concurrent connections racing on the same
        // predecessor); only the last persisted chain matters going forward.
        let e1 = CredentialSecret("e1".into());
        let (chain, _r1) = CredentialChain::establish(e1.clone(), now(), DEFAULT_CREDENTIAL_TTL);

        let outcome_a = authenticate(&chain, &e1, now(), DEFAULT_CREDENTIAL_TTL);
        let outcome_b = authenticate(&chain, &e1, now(), DEFAULT_CREDENTIAL_TTL);

        // Both concurrent AuthRequests against the same durable snapshot are
        // individually accepted (each session did authenticate successfully)...
        assert!(matches!(outcome_a, AuthOutcome::Accepted { .. }));
        assert!(matches!(outcome_b, AuthOutcome::Accepted { .. }));

        // ...but only the chain from whichever commits last durably wins;
        // the repository/transaction layer serializes the actual commit
        // (see server-crate persistence tests), not this pure function.
    }

    #[test]
    fn enrollment_token_round_trips_and_expires() {
        let key = enrollment::SigningKey::generate();
        let token = enrollment::issue(&key, now(), Duration::minutes(5));
        assert!(enrollment::verify(&key, &token, now()));

        let later = now() + Duration::minutes(10);
        assert!(!enrollment::verify(&key, &token, later));
    }

    #[test]
    fn enrollment_token_wrong_signer_is_rejected() {
        let key = enrollment::SigningKey::generate();
        let wrong_key = enrollment::SigningKey::generate();
        let token = enrollment::issue(&key, now(), Duration::minutes(5));
        assert!(!enrollment::verify(&wrong_key, &token, now()));
    }

    #[test]
    fn tampered_enrollment_token_is_rejected() {
        let key = enrollment::SigningKey::generate();
        let token = enrollment::issue(&key, now(), Duration::minutes(5));
        let tampered = CredentialSecret(format!("{}x", token.0));
        assert!(!enrollment::verify(&key, &tampered, now()));
    }
}
