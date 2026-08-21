//! Repository Port (`m0-stack-and-boundaries-baseline.md` "Component
//! responsibilities and boundaries" — Ports: repositories). Application and
//! Domain depend only on this trait; concrete persistence lives entirely in
//! `adapters::postgres` (ADR-0013 "PostgreSQL persistence backend
//! baseline").
//!
//! `redeem` and `update_endpoint` each bound one durable transaction: the
//! Adapter is responsible for locking the affected Endpoint row (or, for a
//! not-yet-existing first-contact row, an equivalent serialization
//! mechanism) *before* invoking the supplied `decide` closure, and for
//! committing the closure's result atomically within that same lock/
//! transaction scope. This is the mechanism that satisfies ADR-0012's
//! commit-time concurrency requirement ("the credential presented needs to
//! remain valid at the commit that accepts the redemption") — the previous
//! WP1 checkpoint's separate `find` then `commit` calls could not express
//! this, since nothing held the row locked between them. `decide` itself
//! never touches the database: it only calls into `bamep_domain::transitions`,
//! so the Domain remains the sole owner of transition/business-rule
//! decisions; the Adapter never reimplements them in SQL.

use async_trait::async_trait;
use bamep_domain::{
    BootContext, EndpointAggregate, EndpointId, InvalidIdentityTransition, RedeemOutcome,
    TransitionOutcome,
};

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("persistence backend error: {0}")]
    Backend(String),
}

/// A pure decision over the (possibly absent) current state of an Endpoint
/// matched by inventory signal, producing what `redeem` must persist.
/// Boxed rather than a bare generic so the Port trait stays object-safety-
/// neutral and callers do not need to name a closure type.
pub type RedeemDecision = Box<dyn FnOnce(Option<EndpointAggregate>) -> RedeemOutcome + Send>;

/// A pure decision over a known Endpoint's current state, producing the
/// transition to persist or the reason it is illegal.
pub type UpdateDecision = Box<
    dyn FnOnce(EndpointAggregate) -> Result<TransitionOutcome, InvalidIdentityTransition> + Send,
>;

#[derive(Debug, thiserror::Error)]
pub enum EndpointUpdateError {
    #[error("endpoint {0:?} not found")]
    NotFound(EndpointId),
    #[error(transparent)]
    InvalidTransition(#[from] InvalidIdentityTransition),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

#[async_trait]
pub trait EndpointRepository: Send + Sync {
    /// Read-only lookup, for verification/reporting only. Never used to
    /// drive a transition decision — `redeem`/`update_endpoint` own that
    /// path so the read and the eventual commit share one lock/transaction.
    async fn find_by_id(
        &self,
        id: EndpointId,
    ) -> Result<Option<EndpointAggregate>, RepositoryError>;

    /// Locks (or otherwise serializes, for the not-yet-existing case) the
    /// Endpoint identified by `inventory_signal`, invokes `decide` with its
    /// current state, and — only for [`RedeemOutcome::Established`] —
    /// atomically persists the returned [`TransitionOutcome`] (state, domain
    /// events, audit record) in the same transaction before releasing the
    /// lock. [`RedeemOutcome::Rejected`] persists nothing.
    ///
    /// Two concurrent callers presenting the same still-valid predecessor,
    /// or the same first-seen `inventory_signal`, are serialized by this
    /// method: the second caller's `decide` observes the first caller's
    /// already-committed result (ADR-0012 point 7; `m0-endpoint-identity-lifecycle.md`
    /// "Concurrent redemption").
    async fn redeem(
        &self,
        inventory_signal: &str,
        decide: RedeemDecision,
    ) -> Result<RedeemOutcome, RepositoryError>;

    /// Locks the Endpoint identified by `id`, invokes `decide` with its
    /// current state, and atomically persists the returned
    /// [`TransitionOutcome`] in the same transaction. Used by operator
    /// approval and credential revocation — operations that require the
    /// Endpoint to already exist.
    async fn update_endpoint(
        &self,
        id: EndpointId,
        decide: UpdateDecision,
    ) -> Result<TransitionOutcome, EndpointUpdateError>;
}

/// Persistence for newly issued `BootContext`s (ADR-0014 point 11
/// "Persist-before-deliver issuance ordering"). Kept as its own narrow Port,
/// separate from [`EndpointRepository`]: BootContext issuance is not an
/// Endpoint aggregate operation — no Endpoint exists yet for an unresolved
/// enrollment credential. Redemption/routing against a `BootContext` (ADR-0014
/// points 5, 7, 8) is not covered by this Port and remains a later
/// checkpoint.
#[async_trait]
pub trait BootContextRepository: Send + Sync {
    /// Durably inserts a newly issued, not-yet-persisted `BootContext`. A
    /// successful return means the row has committed — the caller (a future
    /// Application issuance sequence) may then deliver the enrollment
    /// credential to the booting Endpoint, per the persist-before-deliver
    /// ordering this Port exists to support.
    ///
    /// Rejects, rather than overwrites, a `boot_context_id` that already
    /// exists: `boot_context_id` is randomly generated per issuance, so a
    /// collision indicates a caller error, not a legitimate re-issuance.
    async fn insert_boot_context(&self, context: &BootContext) -> Result<(), RepositoryError>;
}
