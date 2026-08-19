//! Application layer: orchestrates Domain transitions against the
//! `EndpointRepository` Port. Owns no business rules of its own — every
//! decision about whether a transition is legal, and what it produces, comes
//! from `bamep_domain`. This layer's job is sequencing (fetch, decide, one
//! atomic commit) and translating Domain outcomes into results the Runtime
//! Services (Agent Control Gateway, operator-approval harness) can act on.

use std::sync::Arc;

use bamep_domain::credential::enrollment::{self, SigningKey};
use bamep_domain::{
    transitions, Actor, CredentialSecret, EndpointId, InvalidIdentityTransition,
    DEFAULT_CREDENTIAL_TTL,
};
use chrono::{DateTime, Duration, Utc};

use crate::ports::{EndpointRepository, RepositoryError};

#[derive(Debug, thiserror::Error)]
pub enum ApplicationError {
    #[error("endpoint {0:?} not found")]
    EndpointNotFound(EndpointId),
    #[error(transparent)]
    InvalidTransition(#[from] InvalidIdentityTransition),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
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
/// enrollment credential (ADR-0004 point 2). For WP1, the real PXE/boot-chain
/// delivery of this credential to an endpoint is faked by the Simulator
/// fixture (`m0-simulator-contract-and-validation-strategy.md`); this
/// service's issuance logic itself is real.
pub struct BootOrchestrationService {
    signing_key: SigningKey,
    enrollment_ttl: Duration,
}

impl BootOrchestrationService {
    pub fn new(signing_key: SigningKey, enrollment_ttl: Duration) -> Self {
        Self {
            signing_key,
            enrollment_ttl,
        }
    }

    pub fn issue_enrollment_credential(&self, now: DateTime<Utc>) -> CredentialSecret {
        enrollment::issue(&self.signing_key, now, self.enrollment_ttl)
    }
}

/// Endpoint identity/credential enrollment operations
/// (`docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md`).
pub struct EnrollmentService<R: EndpointRepository> {
    repo: Arc<R>,
    enrollment_key: SigningKey,
    credential_ttl: Duration,
}

impl<R: EndpointRepository> EnrollmentService<R> {
    pub fn new(repo: Arc<R>, enrollment_key: SigningKey) -> Self {
        Self {
            repo,
            enrollment_key,
            credential_ttl: DEFAULT_CREDENTIAL_TTL,
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
    pub async fn redeem(
        &self,
        inventory_signal: &str,
        presented: CredentialSecret,
        now: DateTime<Utc>,
    ) -> Result<RedeemResult, ApplicationError> {
        match self.repo.find_by_inventory_signal(inventory_signal).await? {
            None => {
                // First-seen Endpoint: the presented value must itself be a
                // valid, unexpired enrollment credential — otherwise this is
                // a rejected AuthRequest, not a transition to persist.
                if !enrollment::verify(&self.enrollment_key, &presented, now) {
                    return Ok(RedeemResult::Rejected);
                }
                let outcome = transitions::first_contact(
                    inventory_signal.to_string(),
                    presented,
                    now,
                    self.credential_ttl,
                );
                self.apply(outcome).await
            }
            Some(aggregate) => {
                let outcome =
                    transitions::redeem_known(&aggregate, &presented, now, self.credential_ttl);
                self.apply(outcome).await
            }
        }
    }

    async fn apply(
        &self,
        outcome: transitions::RedeemOutcome,
    ) -> Result<RedeemResult, ApplicationError> {
        match outcome {
            transitions::RedeemOutcome::Established {
                outcome,
                issued,
                issued_expires_at,
                ..
            } => {
                let endpoint_id = outcome.endpoint.id;
                self.repo.commit(outcome).await?;
                Ok(RedeemResult::Established {
                    endpoint_id,
                    runtime_credential: issued,
                    credential_expires_at: issued_expires_at,
                })
            }
            transitions::RedeemOutcome::Rejected => Ok(RedeemResult::Rejected),
        }
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
        let aggregate = self
            .repo
            .find_by_id(endpoint_id)
            .await?
            .ok_or(ApplicationError::EndpointNotFound(endpoint_id))?;
        let outcome = transitions::approve_enrollment(&aggregate, operator, now)?;
        self.repo.commit(outcome).await?;
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
        let aggregate = self
            .repo
            .find_by_id(endpoint_id)
            .await?
            .ok_or(ApplicationError::EndpointNotFound(endpoint_id))?;
        let outcome = transitions::revoke_credential(&aggregate, now);
        self.repo.commit(outcome).await?;
        Ok(())
    }
}
