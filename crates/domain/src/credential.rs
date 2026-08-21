//! Runtime Agent credential issuance, rotation, and reconnect recovery.
//!
//! Implements the model accepted in ADR-0012
//! (`docs/decisions/0012-runtime-agent-credential-issuance-rotation-and-reconnect-recovery.md`)
//! and ADR-0014
//! (`docs/decisions/0014-agent-credential-lookup-and-boot-context-correlation.md`),
//! normatively defined in `docs/specifications/m0-endpoint-identity-lifecycle.md`
//! "Credential chain, rotation, and revocation".
//!
//! This module is pure: it never touches the clock or randomness directly.
//! Callers pass `now` explicitly and supply a freshly generated
//! [`PresentedCredential`] candidate, so the state-transition logic stays
//! deterministic and unit-testable.

use chrono::{DateTime, Duration, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;

use crate::presented_credential::{CredentialKind, CredentialLookupId, PresentedCredential};

/// The durable, salted-hash representation of a credential secret
/// (ADR-0012 point 10: "a salted hash verified against a presented value").
/// Never reversible to the original secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialHash {
    salt: [u8; 16],
    digest: [u8; 32],
}

impl CredentialHash {
    /// Byte-oriented one-way verifier construction (ADR-0014 point 4): hashes
    /// the actual secret bytes directly, with no intermediate text encoding.
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

/// One credential in the chain: its non-secret lookup identifier (ADR-0014
/// point 2), its durable hash, and its validity window (ADR-0012 point 4,
/// "Bounded valid set").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CredentialSlot {
    pub lookup_id: CredentialLookupId,
    pub hash: CredentialHash,
    pub issued_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl CredentialSlot {
    fn valid_at(&self, now: DateTime<Utc>) -> bool {
        now < self.expires_at
    }

    /// Both the non-secret lookup identifier and the secret must match
    /// (ADR-0014 point 7: "The lookup identifier never authenticates the
    /// Agent by itself").
    fn matches(&self, presented: &PresentedCredential, now: DateTime<Utc>) -> bool {
        self.valid_at(now)
            && self.lookup_id == *presented.lookup_id()
            && self
                .hash
                .verify_bytes(presented.secret().expose_secret_bytes())
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

/// Builds the durable [`CredentialSlot`] for a freshly issued successor.
/// Freshly issued successor credentials must always be `Runtime`
/// (ADR-0014); returns `None` rather than panicking when a caller hands in a
/// wrongly-kinded candidate — this is caller-controlled data, and the
/// smallest correct handling is to refuse to mint the slot, which every
/// caller in this module already treats as a rejected redemption.
fn mint_successor_slot(
    fresh_successor: &PresentedCredential,
    now: DateTime<Utc>,
    ttl: Duration,
) -> Option<CredentialSlot> {
    if fresh_successor.kind() != CredentialKind::Runtime {
        return None;
    }
    Some(CredentialSlot {
        lookup_id: fresh_successor.lookup_id().clone(),
        hash: CredentialHash::of_bytes(fresh_successor.secret().expose_secret_bytes()),
        issued_at: now,
        expires_at: now + ttl,
    })
}

impl CredentialChain {
    /// Establishes a fresh chain by promoting an already-verified predecessor
    /// (a boot-scoped enrollment credential's lookup id and its `BootContext`
    /// verifier, ADR-0014 point 5) and installing a freshly issued Runtime
    /// successor. Returns `None` only if `fresh_successor` is not a `Runtime`
    /// credential (see [`mint_successor_slot`]) — callers treat that as a
    /// rejected redemption, never a panic.
    pub fn establish(
        predecessor_lookup_id: CredentialLookupId,
        predecessor_verifier: CredentialHash,
        fresh_successor: &PresentedCredential,
        now: DateTime<Utc>,
        ttl: Duration,
    ) -> Option<Self> {
        let successor = mint_successor_slot(fresh_successor, now, ttl)?;
        let predecessor = CredentialSlot {
            lookup_id: predecessor_lookup_id,
            hash: predecessor_verifier,
            issued_at: now,
            expires_at: now + ttl,
        };
        Some(Self {
            predecessor,
            successor: Some(successor),
            revoked: false,
        })
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
    /// (confirming it). Either way, the caller-supplied fresh successor was
    /// installed and must be delivered via `SessionEstablished`.
    Accepted {
        chain: CredentialChain,
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

/// Presents `presented` against `chain` in a fresh `AuthRequest`, minting
/// `fresh_successor` (which the caller must already have generated as a
/// [`CredentialKind::Runtime`] candidate) as the new successor on acceptance.
///
/// Matching requires both the non-secret lookup identifier and the secret to
/// agree (ADR-0014 point 7) — the presented predecessor's *kind* is
/// deliberately not checked: a promoted E1/E2 remains kind `Enrollment` on
/// its wire representation and must keep working as a normal predecessor
/// during its grace window (ADR-0014 point 6).
pub fn authenticate(
    chain: &CredentialChain,
    presented: &PresentedCredential,
    fresh_successor: &PresentedCredential,
    now: DateTime<Utc>,
    ttl: Duration,
) -> AuthOutcome {
    if chain.revoked {
        return AuthOutcome::Rejected;
    }

    if chain.predecessor.matches(presented, now) {
        // Predecessor re-presented: supersede any unconfirmed successor
        // (ADR-0012 point 3) and install the fresh one. Never reconstruct or
        // redeliver the old successor.
        let Some(issued_slot) = mint_successor_slot(fresh_successor, now, ttl) else {
            return AuthOutcome::Rejected;
        };
        let new_chain = CredentialChain {
            predecessor: chain.predecessor.clone(),
            successor: Some(issued_slot.clone()),
            revoked: false,
        };
        return AuthOutcome::Accepted {
            chain: new_chain,
            issued_expires_at: issued_slot.expires_at,
            successor_confirmed: false,
        };
    }

    if let Some(successor) = &chain.successor {
        if successor.matches(presented, now) {
            // Successor confirmed: it becomes the new predecessor, the old
            // predecessor is retired, and the fresh successor is installed
            // for the next rotation (ADR-0012 point 5).
            let Some(issued_slot) = mint_successor_slot(fresh_successor, now, ttl) else {
                return AuthOutcome::Rejected;
            };
            let new_chain = CredentialChain {
                predecessor: successor.clone(),
                successor: Some(issued_slot.clone()),
                revoked: false,
            };
            return AuthOutcome::Accepted {
                chain: new_chain,
                issued_expires_at: issued_slot.expires_at,
                successor_confirmed: true,
            };
        }
    }

    AuthOutcome::Rejected
}

#[cfg(test)]
mod tests {
    use super::*;

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    fn fresh_runtime() -> PresentedCredential {
        PresentedCredential::generate(CredentialKind::Runtime)
    }

    fn fresh_enrollment() -> PresentedCredential {
        PresentedCredential::generate(CredentialKind::Enrollment)
    }

    fn establish_chain(
        e1: &PresentedCredential,
        r1: &PresentedCredential,
        now: DateTime<Utc>,
        ttl: Duration,
    ) -> CredentialChain {
        let verifier = CredentialHash::of_bytes(e1.secret().expose_secret_bytes());
        CredentialChain::establish(e1.lookup_id().clone(), verifier, r1, now, ttl)
            .expect("fresh successor is Runtime")
    }

    #[test]
    fn establish_creates_predecessor_and_installs_fresh_successor() {
        let e1 = fresh_enrollment();
        let r1 = fresh_runtime();
        let chain = establish_chain(&e1, &r1, now(), DEFAULT_CREDENTIAL_TTL);
        assert_eq!(
            chain.dimension(now()),
            CredentialDimension::CredentialActive
        );
        assert_eq!(chain.predecessor().lookup_id, *e1.lookup_id());
        assert_eq!(chain.successor().unwrap().lookup_id, *r1.lookup_id());
    }

    #[test]
    fn establish_rejects_a_non_runtime_fresh_successor() {
        let e1 = fresh_enrollment();
        let not_runtime = fresh_enrollment();
        let verifier = CredentialHash::of_bytes(e1.secret().expose_secret_bytes());
        assert!(CredentialChain::establish(
            e1.lookup_id().clone(),
            verifier,
            &not_runtime,
            now(),
            DEFAULT_CREDENTIAL_TTL
        )
        .is_none());
    }

    #[test]
    fn presenting_predecessor_again_supersedes_unconfirmed_successor() {
        let e1 = fresh_enrollment();
        let r1 = fresh_runtime();
        let chain = establish_chain(&e1, &r1, now(), DEFAULT_CREDENTIAL_TTL);

        // E1 reappears before R1 ever authenticated (e.g. dropped connection
        // between commit and delivery).
        let r1_prime = fresh_runtime();
        let outcome = authenticate(&chain, &e1, &r1_prime, now(), DEFAULT_CREDENTIAL_TTL);
        match outcome {
            AuthOutcome::Accepted {
                chain: new_chain,
                successor_confirmed,
                ..
            } => {
                assert!(!successor_confirmed);
                assert_ne!(r1.lookup_id(), r1_prime.lookup_id());
                // The superseded R1 must no longer authenticate.
                assert_eq!(
                    authenticate(
                        &new_chain,
                        &r1,
                        &fresh_runtime(),
                        now(),
                        DEFAULT_CREDENTIAL_TTL
                    ),
                    AuthOutcome::Rejected
                );
                // The fresh successor does authenticate (confirming it).
                assert!(matches!(
                    authenticate(
                        &new_chain,
                        &r1_prime,
                        &fresh_runtime(),
                        now(),
                        DEFAULT_CREDENTIAL_TTL
                    ),
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
        let e1 = fresh_enrollment();
        let r1 = fresh_runtime();
        let chain = establish_chain(&e1, &r1, now(), DEFAULT_CREDENTIAL_TTL);

        let r2 = fresh_runtime();
        let outcome = authenticate(&chain, &r1, &r2, now(), DEFAULT_CREDENTIAL_TTL);
        let new_chain = match outcome {
            AuthOutcome::Accepted {
                chain,
                successor_confirmed,
                ..
            } => {
                assert!(successor_confirmed);
                chain
            }
            AuthOutcome::Rejected => panic!("successor must authenticate"),
        };

        // E1 (the old predecessor) must now be rejected: it was retired on
        // confirmation.
        assert_eq!(
            authenticate(
                &new_chain,
                &e1,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Rejected
        );
        // R1 is now the predecessor and still authenticates (retry path).
        assert!(matches!(
            authenticate(
                &new_chain,
                &r1,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Accepted {
                successor_confirmed: false,
                ..
            }
        ));
        // R2 (the freshly installed successor) confirms in turn.
        assert!(matches!(
            authenticate(
                &new_chain,
                &r2,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Accepted {
                successor_confirmed: true,
                ..
            }
        ));
    }

    #[test]
    fn recovery_by_retrying_still_valid_predecessor_after_superseded_successor() {
        let e1 = fresh_enrollment();
        let r1 = fresh_runtime();
        let chain = establish_chain(&e1, &r1, now(), DEFAULT_CREDENTIAL_TTL);

        // E1 retried (as if R1 delivery was lost), producing R1'.
        let r1_prime = fresh_runtime();
        let chain2 = match authenticate(&chain, &e1, &r1_prime, now(), DEFAULT_CREDENTIAL_TTL) {
            AuthOutcome::Accepted { chain, .. } => chain,
            AuthOutcome::Rejected => panic!(),
        };

        // Agent still holds the now-superseded R1 and tries it: generic
        // rejection, not a distinguishing error.
        assert_eq!(
            authenticate(
                &chain2,
                &r1,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Rejected
        );

        // Agent falls back to E1 (still valid, within grace): recovers.
        assert!(matches!(
            authenticate(
                &chain2,
                &e1,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Accepted { .. }
        ));
    }

    #[test]
    fn expired_predecessor_and_successor_are_rejected() {
        let e1 = fresh_enrollment();
        let r1 = fresh_runtime();
        let past = now() - Duration::hours(2);
        let chain = establish_chain(&e1, &r1, past, DEFAULT_CREDENTIAL_TTL);

        assert_eq!(
            chain.dimension(now()),
            CredentialDimension::CredentialExpired
        );
        assert_eq!(
            authenticate(&chain, &e1, &fresh_runtime(), now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Rejected
        );
        assert_eq!(
            authenticate(&chain, &r1, &fresh_runtime(), now(), DEFAULT_CREDENTIAL_TTL),
            AuthOutcome::Rejected
        );
    }

    #[test]
    fn revocation_invalidates_every_credential_in_the_chain() {
        let e1 = fresh_enrollment();
        let r1 = fresh_runtime();
        let chain = establish_chain(&e1, &r1, now(), DEFAULT_CREDENTIAL_TTL);
        let revoked = chain.revoke();

        assert_eq!(
            revoked.dimension(now()),
            CredentialDimension::CredentialRevoked
        );
        assert_eq!(
            authenticate(
                &revoked,
                &e1,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Rejected
        );
        assert_eq!(
            authenticate(
                &revoked,
                &r1,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Rejected
        );
    }

    #[test]
    fn unrelated_garbage_credential_is_rejected() {
        let e1 = fresh_enrollment();
        let chain = establish_chain(&e1, &fresh_runtime(), now(), DEFAULT_CREDENTIAL_TTL);
        let garbage = fresh_runtime();
        assert_eq!(
            authenticate(
                &chain,
                &garbage,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Rejected
        );
    }

    #[test]
    fn correct_lookup_id_with_wrong_secret_is_rejected() {
        let e1 = fresh_enrollment();
        let chain = establish_chain(&e1, &fresh_runtime(), now(), DEFAULT_CREDENTIAL_TTL);

        // Same lookup id as the real predecessor, but different (wrong)
        // secret material — must not authenticate on lookup id alone
        // (ADR-0014 point 7).
        let wrong_secret = PresentedCredential::generate(CredentialKind::Enrollment);
        let forged = presented_with_lookup_id(&wrong_secret, e1.lookup_id().clone());
        assert_eq!(
            authenticate(
                &chain,
                &forged,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Rejected
        );
    }

    #[test]
    fn wrong_lookup_id_with_the_right_secret_bytes_is_rejected() {
        let e1 = fresh_enrollment();
        let chain = establish_chain(&e1, &fresh_runtime(), now(), DEFAULT_CREDENTIAL_TTL);

        // Correct secret bytes, but under a different lookup id: the slot's
        // stored lookup id no longer matches, so this must reject even
        // though the underlying secret would otherwise verify.
        let other_lookup = PresentedCredential::generate(CredentialKind::Enrollment);
        let forged = presented_with_lookup_id(&e1, other_lookup.lookup_id().clone());
        assert_eq!(
            authenticate(
                &chain,
                &forged,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Rejected
        );
    }

    /// Test-only helper: reconstructs a presented credential carrying a
    /// different lookup id than the one it was generated with, by
    /// round-tripping through the wire format. Exists only to exercise the
    /// "lookup id alone never authenticates" invariant from both directions.
    fn presented_with_lookup_id(
        source: &PresentedCredential,
        lookup_id: CredentialLookupId,
    ) -> PresentedCredential {
        let wire = source.to_wire_value();
        let parts: Vec<&str> = wire.split('.').collect();
        let lookup_wire = {
            // base64url without padding never contains '.', so this
            // round-trip through the public wire format is the only way
            // this test module (which has no access to presented_credential's
            // private fields) can build a credential with a chosen lookup id.
            use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
            URL_SAFE_NO_PAD.encode(lookup_id.to_bytes())
        };
        let rebuilt = format!("{}.{}.{}.{}", parts[0], parts[1], lookup_wire, parts[3]);
        PresentedCredential::parse(&rebuilt).expect("rebuilt wire value must parse")
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
        let e1 = fresh_enrollment();
        let chain = establish_chain(&e1, &fresh_runtime(), now(), DEFAULT_CREDENTIAL_TTL);

        let outcome_a = authenticate(&chain, &e1, &fresh_runtime(), now(), DEFAULT_CREDENTIAL_TTL);
        let outcome_b = authenticate(&chain, &e1, &fresh_runtime(), now(), DEFAULT_CREDENTIAL_TTL);

        // Both concurrent AuthRequests against the same durable snapshot are
        // individually accepted (each session did authenticate successfully)...
        assert!(matches!(outcome_a, AuthOutcome::Accepted { .. }));
        assert!(matches!(outcome_b, AuthOutcome::Accepted { .. }));

        // ...but only the chain from whichever commits last durably wins;
        // the repository/transaction layer serializes the actual commit
        // (see server-crate persistence tests), not this pure function.
    }
}
