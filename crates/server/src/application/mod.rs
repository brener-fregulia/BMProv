//! Application layer: orchestrates Domain transitions against the
//! `EndpointRepository` Port. Owns no business rules of its own — every
//! decision about whether a transition is legal, and what it produces, comes
//! from `bamep_domain`. This layer's job is sequencing (fetch, decide, one
//! atomic commit) and translating Domain outcomes into results the Runtime
//! Services (Agent Control Gateway, operator-approval harness) can act on.

use std::sync::Arc;

use bamep_domain::credential::enrollment::{self, SigningKey};
use bamep_domain::credential::CredentialHash;
use bamep_domain::presented_credential::{CredentialKind, PresentedCredential};
use bamep_domain::{
    transitions, Actor, BootContext, CredentialSecret, EndpointId, InvalidIdentityTransition,
    DEFAULT_CREDENTIAL_TTL,
};
use chrono::{DateTime, Duration, Utc};

use crate::ports::{
    BootContextRepository, EndpointRepository, EndpointUpdateError, RepositoryError,
};

#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("endpoint {0:?} not found")]
    EndpointNotFound(EndpointId),
    #[error(transparent)]
    InvalidTransition(#[from] InvalidIdentityTransition),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
}

impl From<EndpointUpdateError> for ApplicationError {
    fn from(err: EndpointUpdateError) -> Self {
        match err {
            EndpointUpdateError::NotFound(id) => ApplicationError::EndpointNotFound(id),
            EndpointUpdateError::InvalidTransition(e) => ApplicationError::InvalidTransition(e),
            EndpointUpdateError::Repository(e) => ApplicationError::Repository(e),
        }
    }
}

/// Wall-clock abstraction so [`EnrollmentService::redeem`] can obtain "now"
/// at *decision time* — inside `EndpointRepository::redeem`'s lock/
/// transaction scope, after it has serialized against concurrent
/// redemptions for the same inventory signal — rather than at *call time*,
/// before any lock is even requested. ADR-0012 requires that "the
/// credential presented needs to remain valid at the commit that accepts
/// the redemption"; a `now` captured before a lock wait and carried through
/// unchanged cannot satisfy that if the wait is long enough for the
/// credential to expire in between. Deliberately adapter-neutral and
/// PostgreSQL-free — this is a pure Application-level concern, not a Port/
/// Adapter one, and Domain functions are unaffected: they still take an
/// explicit `now: DateTime<Utc>` parameter, preserving Domain purity and
/// deterministic unit testing.
pub trait Clock: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
}

/// Real wall-clock time — the production default.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> DateTime<Utc> {
        Utc::now()
    }
}

/// Outcome of redeeming a presented credential in a fresh `AuthRequest`,
/// shaped for the eventual Agent Control Gateway adapter to translate
/// directly into `SessionEstablished` / `AuthError`
/// (`m0-agent-protocol-contract.md` "Transport and handshake").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RedeemResult {
    Established {
        endpoint_id: EndpointId,
        runtime_credential: CredentialSecret,
        credential_expires_at: DateTime<Utc>,
    },
    Rejected,
}

/// Boot Orchestration's Application-level responsibility
/// (`m0-stack-and-boundaries-baseline.md` "Component responsibilities and
/// boundaries" — Application: Boot Orchestration): issuing the boot-scoped
/// enrollment credential (ADR-0004 point 2) as a durable, self-locating
/// ADR-0014 credential, following the mandatory persist-before-deliver
/// ordering (ADR-0014 point 11). For WP1, the real PXE/boot-chain delivery of
/// this credential to an endpoint is faked by the Simulator fixture
/// (`m0-simulator-contract-and-validation-strategy.md`); this service's
/// issuance logic itself is real.
pub struct BootOrchestrationService<R: BootContextRepository> {
    repo: Arc<R>,
    enrollment_ttl: Duration,
}

impl<R: BootContextRepository> BootOrchestrationService<R> {
    pub fn new(repo: Arc<R>, enrollment_ttl: Duration) -> Self {
        Self {
            repo,
            enrollment_ttl,
        }
    }

    /// Issues a fresh boot-scoped enrollment credential: generates a
    /// self-locating `PresentedCredential::Enrollment`, derives its one-way
    /// verifier, and durably persists the backing `BootContext` — only after
    /// that persistence succeeds does this method return the credential
    /// (ADR-0014 point 11). A persistence failure returns an
    /// `ApplicationError` and never returns the generated credential; this
    /// method does not retry with a fresh credential of its own.
    ///
    /// `inventory_signal` is the current WP1 correlation-evidence stand-in
    /// stored on `BootContext` — evidence only, never authentication and
    /// never Endpoint identity (ADR-0004; ADR-0014 point 4).
    pub async fn issue_enrollment_credential(
        &self,
        inventory_signal: &str,
        now: DateTime<Utc>,
    ) -> Result<PresentedCredential, ApplicationError> {
        let credential = PresentedCredential::generate(CredentialKind::Enrollment);
        let verifier = CredentialHash::of_bytes(credential.secret().expose_secret_bytes());
        let context = BootContext::new(
            credential.lookup_id().clone(),
            verifier,
            now,
            now + self.enrollment_ttl,
            inventory_signal.to_string(),
        );
        self.repo.insert_boot_context(&context).await?;
        Ok(credential)
    }
}

/// Endpoint identity/credential enrollment operations
/// (`docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md`).
pub struct EnrollmentService<R: EndpointRepository> {
    repo: Arc<R>,
    enrollment_key: SigningKey,
    credential_ttl: Duration,
    clock: Arc<dyn Clock>,
}

impl<R: EndpointRepository> EnrollmentService<R> {
    /// Uses [`SystemClock`] — real wall-clock time, evaluated at decision
    /// time by [`redeem`](Self::redeem). Use [`with_clock`](Self::with_clock)
    /// to inject a deterministic clock (e.g. for tests that must control
    /// simulated time precisely).
    pub fn new(repo: Arc<R>, enrollment_key: SigningKey) -> Self {
        Self::with_clock(repo, enrollment_key, Arc::new(SystemClock))
    }

    pub fn with_clock(repo: Arc<R>, enrollment_key: SigningKey, clock: Arc<dyn Clock>) -> Self {
        Self {
            repo,
            enrollment_key,
            credential_ttl: DEFAULT_CREDENTIAL_TTL,
            clock,
        }
    }

    pub fn with_credential_ttl(mut self, ttl: Duration) -> Self {
        self.credential_ttl = ttl;
        self
    }

    /// Redeems a presented credential in a fresh `AuthRequest`. Called by the
    /// (not-yet-implemented, future round) Agent Control Gateway on every
    /// connection attempt, after the Server's own TLS layer has already
    /// completed — this method has no notion of TLS/WSS itself.
    ///
    /// The decision (first-contact-vs-known-Endpoint branching, enrollment
    /// verification, chain authentication, genuine-reboot fallback) is
    /// handed to the repository as a closure so it executes *inside* the
    /// Adapter's lock/transaction scope on the Endpoint's current state —
    /// never on a state read before that lock was acquired (ADR-0012 point 7
    /// commit-time concurrency; `crate::ports::EndpointRepository::redeem`).
    /// `now` is deliberately not a parameter here: the closure reads
    /// `self.clock.now()` itself, at the moment the Adapter actually invokes
    /// it (i.e. after the lock), so credential-validity decisions are never
    /// made against a timestamp captured before a lock wait of unknown
    /// duration (ADR-0012: "the credential presented needs to remain valid
    /// at the commit that accepts the redemption").
    pub async fn redeem(
        &self,
        inventory_signal: &str,
        presented: CredentialSecret,
    ) -> Result<RedeemResult, ApplicationError> {
        let signal = inventory_signal.to_string();
        let ttl = self.credential_ttl;
        let key = self.enrollment_key.clone();
        let clock = Arc::clone(&self.clock);
        let decide: crate::ports::RedeemDecision = Box::new(move |existing| {
            // Read here, not before — this closure body only ever runs
            // after the Adapter has acquired its lock for this
            // inventory_signal/Endpoint.
            let now = clock.now();
            match existing {
                None => {
                    // First-seen Endpoint: the presented value must itself
                    // be a valid, unexpired enrollment credential —
                    // otherwise this is a rejected AuthRequest, not a
                    // transition to persist.
                    if !enrollment::verify(&key, &presented, now) {
                        return transitions::RedeemOutcome::Rejected;
                    }
                    transitions::first_contact(signal, presented, now, ttl)
                }
                Some(aggregate) => {
                    match transitions::redeem_known(&aggregate, &presented, now, ttl) {
                        established @ transitions::RedeemOutcome::Established { .. } => established,
                        transitions::RedeemOutcome::Rejected => {
                            // The presented value did not match this known
                            // Endpoint's current chain. A fresh, valid
                            // boot-scoped enrollment credential still
                            // legitimately re-establishes a brand-new chain
                            // — "genuine reboot" (ADR-0012 point 1: E2 ->
                            // fresh runtime credential). Identity continuity
                            // is preserved and operator approval is not
                            // re-run (ADR-0004 "Reconnect handling").
                            //
                            // Deliberately NOT attempted when the current
                            // chain is `CredentialRevoked`: revocation is
                            // durable Endpoint-level state that survives a
                            // genuine reboot, and a fresh, independently
                            // valid E2 does not clear it (owner-approved
                            // policy, ADR-0012 point 8 / "Consequences").
                            // Restoring `CredentialActive` requires a
                            // separate, explicit, authorized reactivation
                            // operation, not implemented in WP1.
                            if aggregate.credential.is_revoked() {
                                transitions::RedeemOutcome::Rejected
                            } else if enrollment::verify(&key, &presented, now) {
                                transitions::genuine_reboot(&aggregate, presented, now, ttl)
                            } else {
                                transitions::RedeemOutcome::Rejected
                            }
                        }
                    }
                }
            }
        });

        let outcome = self.repo.redeem(inventory_signal, decide).await?;
        Ok(match outcome {
            transitions::RedeemOutcome::Established {
                outcome,
                issued,
                issued_expires_at,
                ..
            } => RedeemResult::Established {
                endpoint_id: outcome.endpoint.id,
                runtime_credential: issued,
                credential_expires_at: issued_expires_at,
            },
            transitions::RedeemOutcome::Rejected => RedeemResult::Rejected,
        })
    }

    /// The operator-approval control path
    /// (`docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md`
    /// "Decision: operator-approval-gated first enrollment"; Issue #17
    /// "Safety constraints"). Callers of this method must be structurally
    /// separate from the Simulated Agent participant — an in-process
    /// test/development harness, a future Administrative API handler, or a
    /// CLI, never Agent Protocol message handling.
    pub async fn approve_enrollment(
        &self,
        endpoint_id: EndpointId,
        operator: Actor,
        now: DateTime<Utc>,
    ) -> Result<(), ApplicationError> {
        let decide: crate::ports::UpdateDecision =
            Box::new(move |aggregate| transitions::approve_enrollment(&aggregate, operator, now));
        self.repo.update_endpoint(endpoint_id, decide).await?;
        Ok(())
    }

    /// Exercises `CredentialRevoked` at the domain/persistence layer directly
    /// (Issue #17 "Safety constraints": no new operator-facing revocation API
    /// is introduced merely to demonstrate this for WP1).
    pub async fn revoke_credential(
        &self,
        endpoint_id: EndpointId,
        now: DateTime<Utc>,
    ) -> Result<(), ApplicationError> {
        let decide: crate::ports::UpdateDecision =
            Box::new(move |aggregate| Ok(transitions::revoke_credential(&aggregate, now)));
        self.repo.update_endpoint(endpoint_id, decide).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Minimal in-memory `BootContextRepository` fake for Application-level
    /// unit tests that need precise, DB-free control over persistence
    /// success/failure and immediate visibility into what was persisted
    /// (`docs/development/testing.md` "Fakes and test boundaries"). The real
    /// PostgreSQL persistence path is covered separately by
    /// `crates/server/tests/boot_orchestration_service.rs`.
    #[derive(Default)]
    struct FakeBootContextRepository {
        contexts: Mutex<Vec<BootContext>>,
        fail: bool,
    }

    impl FakeBootContextRepository {
        fn new() -> Self {
            Self::default()
        }

        fn failing() -> Self {
            Self {
                contexts: Mutex::new(Vec::new()),
                fail: true,
            }
        }

        fn persisted(&self) -> Vec<BootContext> {
            self.contexts.lock().unwrap().clone()
        }
    }

    #[async_trait]
    impl BootContextRepository for FakeBootContextRepository {
        async fn insert_boot_context(&self, context: &BootContext) -> Result<(), RepositoryError> {
            if self.fail {
                return Err(RepositoryError::Backend(
                    "simulated persistence failure".into(),
                ));
            }
            self.contexts.lock().unwrap().push(context.clone());
            Ok(())
        }
    }

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    #[tokio::test]
    async fn issuance_returns_a_valid_self_locating_enrollment_credential() {
        let repo = Arc::new(FakeBootContextRepository::new());
        let service = BootOrchestrationService::new(repo, Duration::minutes(5));

        let credential = service
            .issue_enrollment_credential("sim-boot-orch-01", now())
            .await
            .expect("issuance must succeed");

        assert_eq!(credential.kind(), CredentialKind::Enrollment);
        // Self-locating: round-trips through the wire encoding cleanly.
        let wire = credential.to_wire_value();
        let parsed = PresentedCredential::parse(&wire).expect("must parse");
        assert_eq!(parsed.lookup_id(), credential.lookup_id());
    }

    #[tokio::test]
    async fn boot_context_is_durably_persisted_before_the_credential_is_returned() {
        let repo = Arc::new(FakeBootContextRepository::new());
        let service = BootOrchestrationService::new(Arc::clone(&repo), Duration::minutes(5));

        assert!(repo.persisted().is_empty());
        let credential = service
            .issue_enrollment_credential("sim-boot-orch-02", now())
            .await
            .expect("issuance must succeed");

        let persisted = repo.persisted();
        assert_eq!(
            persisted.len(),
            1,
            "BootContext must be durably persisted exactly once by the time issuance returns"
        );
        assert_eq!(persisted[0].boot_context_id(), credential.lookup_id());
    }

    #[tokio::test]
    async fn persisted_boot_context_matches_the_returned_credential() {
        let repo = Arc::new(FakeBootContextRepository::new());
        let ttl = Duration::minutes(5);
        let service = BootOrchestrationService::new(Arc::clone(&repo), ttl);
        let issued_at = now();

        let credential = service
            .issue_enrollment_credential("sim-boot-orch-03", issued_at)
            .await
            .expect("issuance must succeed");

        let persisted = repo.persisted();
        let context = &persisted[0];

        assert_eq!(context.boot_context_id(), credential.lookup_id());
        assert!(context.verify_secret(credential.secret()));
        assert_eq!(context.issued_at(), issued_at);
        assert_eq!(context.expires_at(), issued_at + ttl);
        assert_eq!(context.inventory_signal(), "sim-boot-orch-03");
        assert_eq!(context.resolved_endpoint_id(), None);
    }

    #[tokio::test]
    async fn two_issuances_generate_distinct_lookup_ids_and_secrets() {
        let repo = Arc::new(FakeBootContextRepository::new());
        let service = BootOrchestrationService::new(Arc::clone(&repo), Duration::minutes(5));

        let a = service
            .issue_enrollment_credential("sim-boot-orch-04", now())
            .await
            .unwrap();
        let b = service
            .issue_enrollment_credential("sim-boot-orch-04", now())
            .await
            .unwrap();

        assert_ne!(a.lookup_id(), b.lookup_id());
        assert_ne!(
            a.secret().expose_secret_bytes(),
            b.secret().expose_secret_bytes()
        );
    }

    #[tokio::test]
    async fn persistence_failure_yields_an_application_error_and_no_credential() {
        let repo = Arc::new(FakeBootContextRepository::failing());
        let service = BootOrchestrationService::new(Arc::clone(&repo), Duration::minutes(5));

        let err = service
            .issue_enrollment_credential("sim-boot-orch-05", now())
            .await
            .unwrap_err();

        assert!(matches!(err, ApplicationError::Repository(_)));
        assert!(repo.persisted().is_empty());
    }
}
