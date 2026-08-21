//! `BootContext`: durable state backing an unresolved boot-scoped enrollment
//! credential (`docs/decisions/0014-agent-credential-lookup-and-boot-context-correlation.md`
//! point 4).
//!
//! Before its first successful redemption, an enrollment credential's
//! non-secret lookup identifier (`boot_context_id`) resolves through this
//! record rather than through a persisted Endpoint credential chain, since no
//! Endpoint exists yet to own one. `BootContext` holds a one-way verifier of
//! the enrollment secret, an expiry, Server/Boot-Orchestration-observed
//! correlation evidence, and — once redemption succeeds — the Endpoint it
//! resolved to.
//!
//! This module is pure, like [`crate::credential`] and
//! [`crate::transitions`]: it never touches the clock or randomness
//! directly, and knows nothing about PostgreSQL, locking, or how it is
//! persisted. Those remain later checkpoints.

use chrono::{DateTime, Utc};

use crate::credential::CredentialHash;
use crate::endpoint::EndpointId;
use crate::presented_credential::{CredentialLookupId, PresentedCredentialSecret};

/// Durable BootContext state for one unresolved (or since-resolved)
/// boot-scoped enrollment credential.
///
/// `inventory_signal` is correlation evidence only (ADR-0014 point 4;
/// ADR-0004) — never authentication, never durable Endpoint identity, and
/// never a trust anchor by itself. It is the current WP1 stand-in for the
/// real hardware-correlation algorithm, mirroring
/// [`crate::endpoint::EndpointAggregate::inventory_signal`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootContext {
    boot_context_id: CredentialLookupId,
    verifier: CredentialHash,
    issued_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    inventory_signal: String,
    resolved_endpoint_id: Option<EndpointId>,
}

impl BootContext {
    /// Constructs a fresh, unresolved `BootContext` at issuance time. The
    /// caller derives `verifier` beforehand (e.g. via
    /// [`CredentialHash::of_bytes`] over a [`PresentedCredentialSecret`]'s
    /// raw bytes) — this constructor never sees or stores the plaintext
    /// secret itself.
    pub fn new(
        boot_context_id: CredentialLookupId,
        verifier: CredentialHash,
        issued_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        inventory_signal: String,
    ) -> Self {
        Self {
            boot_context_id,
            verifier,
            issued_at,
            expires_at,
            inventory_signal,
            resolved_endpoint_id: None,
        }
    }

    /// Reconstructs a `BootContext` from its durable parts. Adapter-facing
    /// only: a relational Adapter reloading a previously committed row is
    /// the only legitimate caller — this constructor trusts the caller to
    /// hand back exactly what a prior `new`/commit produced, and enforces no
    /// invariant of its own beyond the type signature.
    pub fn from_parts(
        boot_context_id: CredentialLookupId,
        verifier: CredentialHash,
        issued_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        inventory_signal: String,
        resolved_endpoint_id: Option<EndpointId>,
    ) -> Self {
        Self {
            boot_context_id,
            verifier,
            issued_at,
            expires_at,
            inventory_signal,
            resolved_endpoint_id,
        }
    }

    pub fn boot_context_id(&self) -> &CredentialLookupId {
        &self.boot_context_id
    }

    /// Read-only access to the stored verifier, for a relational Adapter to
    /// decompose it into its durable byte representation
    /// ([`CredentialHash::to_bytes`]). Never usable to recover the original
    /// secret.
    pub fn verifier(&self) -> &CredentialHash {
        &self.verifier
    }

    pub fn issued_at(&self) -> DateTime<Utc> {
        self.issued_at
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    /// Correlation evidence only — see this type's documentation. Never an
    /// authentication input.
    pub fn inventory_signal(&self) -> &str {
        &self.inventory_signal
    }

    pub fn resolved_endpoint_id(&self) -> Option<EndpointId> {
        self.resolved_endpoint_id
    }

    /// Whether this BootContext may still admit its first successful
    /// redemption at `now` (ADR-0014 point 6: `BootContext.expires_at`
    /// governs only the unresolved credential's first successful
    /// redemption, with no grace period). Callers must supply `now`
    /// explicitly, evaluated only after acquiring whatever lock the Adapter
    /// requires for commit-time evaluation (ADR-0014 point 8) — this method
    /// performs no locking or clock access of its own.
    pub fn valid_at(&self, now: DateTime<Utc>) -> bool {
        now < self.expires_at
    }

    /// Verifies a presented enrollment secret against the stored one-way
    /// verifier, via the same constant-time mechanism as every other
    /// credential comparison in this Domain
    /// ([`CredentialHash::verify_bytes`]). Possession of `boot_context_id`
    /// alone never reaches this method's success path without this check
    /// also succeeding.
    pub fn verify_secret(&self, presented: &PresentedCredentialSecret) -> bool {
        self.verifier.verify_bytes(presented.expose_secret_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    fn fresh_context(secret: &PresentedCredentialSecret, ttl: Duration) -> BootContext {
        BootContext::new(
            CredentialLookupId::generate(),
            CredentialHash::of_bytes(secret.expose_secret_bytes()),
            now(),
            now() + ttl,
            "inventory-signal-e1".into(),
        )
    }

    #[test]
    fn stores_lookup_id_without_treating_it_as_authentication() {
        let secret = PresentedCredentialSecret::generate();
        let lookup_id = CredentialLookupId::generate();
        let context = BootContext::new(
            lookup_id.clone(),
            CredentialHash::of_bytes(secret.expose_secret_bytes()),
            now(),
            now() + Duration::minutes(5),
            "inventory-signal-e1".into(),
        );

        // The lookup id round-trips through the accessor exactly...
        assert_eq!(context.boot_context_id(), &lookup_id);
        // ...but knowing it is not sufficient to authenticate: a wrong
        // secret is still rejected even though the caller supplied the
        // correct boot_context_id to reach this BootContext at all.
        let wrong = PresentedCredentialSecret::generate();
        assert!(!context.verify_secret(&wrong));
    }

    #[test]
    fn correct_enrollment_secret_verifies() {
        let secret = PresentedCredentialSecret::generate();
        let context = fresh_context(&secret, Duration::minutes(5));
        assert!(context.verify_secret(&secret));
    }

    #[test]
    fn different_secret_does_not_verify() {
        let secret = PresentedCredentialSecret::generate();
        let context = fresh_context(&secret, Duration::minutes(5));
        let other = PresentedCredentialSecret::generate();
        assert!(!context.verify_secret(&other));
    }

    #[test]
    fn verifier_uses_the_one_way_credential_hash_mechanism() {
        let secret = PresentedCredentialSecret::generate();
        let hash = CredentialHash::of_bytes(secret.expose_secret_bytes());
        let context = BootContext::from_parts(
            CredentialLookupId::generate(),
            hash.clone(),
            now(),
            now() + Duration::minutes(5),
            "inventory-signal-e1".into(),
            None,
        );

        // Reconstructing the same verifier from its durable bytes and
        // verifying independently agrees with BootContext's own check —
        // proving BootContext delegates to CredentialHash rather than
        // implementing its own comparison.
        let reconstructed_hash = CredentialHash::from_bytes(hash.to_bytes());
        assert!(reconstructed_hash.verify_bytes(secret.expose_secret_bytes()));
        assert!(context.verify_secret(&secret));
    }

    #[test]
    fn context_valid_before_expires_at() {
        let secret = PresentedCredentialSecret::generate();
        let context = fresh_context(&secret, Duration::minutes(5));
        assert!(context.valid_at(context.expires_at() - Duration::seconds(1)));
    }

    #[test]
    fn context_invalid_at_expires_at() {
        let secret = PresentedCredentialSecret::generate();
        let context = fresh_context(&secret, Duration::minutes(5));
        assert!(!context.valid_at(context.expires_at()));
    }

    #[test]
    fn context_invalid_after_expires_at() {
        let secret = PresentedCredentialSecret::generate();
        let context = fresh_context(&secret, Duration::minutes(5));
        assert!(!context.valid_at(context.expires_at() + Duration::seconds(1)));
    }

    #[test]
    fn unresolved_context_has_no_resolved_endpoint_id() {
        let secret = PresentedCredentialSecret::generate();
        let context = fresh_context(&secret, Duration::minutes(5));
        assert_eq!(context.resolved_endpoint_id(), None);
    }

    #[test]
    fn from_parts_round_trips_a_resolved_endpoint_id() {
        let secret = PresentedCredentialSecret::generate();
        let endpoint_id = EndpointId::new();
        let context = BootContext::from_parts(
            CredentialLookupId::generate(),
            CredentialHash::of_bytes(secret.expose_secret_bytes()),
            now(),
            now() + Duration::minutes(5),
            "inventory-signal-e1".into(),
            Some(endpoint_id),
        );
        assert_eq!(context.resolved_endpoint_id(), Some(endpoint_id));
    }
}
