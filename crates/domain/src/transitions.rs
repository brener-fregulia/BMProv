//! Assembles the pure state-machine modules (`identity`, `credential`,
//! `boot_context`) into the concrete durable transitions WP1 requires, each
//! producing a [`RedeemOutcome`]/[`TransitionOutcome`] ready for atomic
//! persistence (ADR-0013 "PostgreSQL persistence backend baseline", carrying
//! forward ADR-0007's "Transactional consistency between domain state,
//! domain events, and audit records").
//!
//! Every [`TransitionOutcome`] returned here is constructed exclusively by
//! this module, so an Application-layer caller can never hand-assemble a
//! domain aggregate into an invariant-violating shape.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::boot_context::{BootContext, BootContextResolveError};
use crate::credential::{self, AuthOutcome, CredentialChain};
use crate::endpoint::{EndpointAggregate, EndpointId};
use crate::events::{Actor, AuditRecord, DomainEvent, TransitionOutcome};
use crate::identity::{self, IdentityState, InvalidIdentityTransition};
use crate::presented_credential::{CredentialKind, PresentedCredential};

/// Result of redeeming a credential in a fresh `AuthRequest`, whether for a
/// first-seen Endpoint, a genuine reboot, or a known one.
///
/// `Established` intentionally carries `TransitionOutcome` by value rather
/// than boxed: this type crosses one `AuthRequest` at a time, never a hot
/// per-message path (`ActionProgress`-style traffic stays out of this type
/// entirely), so the size difference clippy flags is not worth the
/// indirection at WP1's scale.
#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum RedeemOutcome {
    Established {
        outcome: TransitionOutcome,
        /// The fresh Runtime credential the caller generated and this
        /// redemption installed as the new successor — the caller already
        /// holds this value; it is echoed back so a single match on
        /// `RedeemOutcome` gives the Application everything it needs to
        /// answer the `AuthRequest`.
        issued: PresentedCredential,
        issued_expires_at: DateTime<Utc>,
        /// True only when this redemption is the Endpoint's very first
        /// (i.e. this outcome also creates `PendingEnrollment`).
        first_contact: bool,
        successor_confirmed: bool,
        /// The `BootContext` this redemption resolved, when it resolved one
        /// (first contact or genuine reboot) — `None` for an ordinary
        /// persisted-chain rotation (ADR-0014 point 5). The Adapter persists
        /// `resolved_endpoint_id` atomically with the rest of this outcome.
        resolved_boot_context: Option<BootContext>,
    },
    Rejected,
}

/// Verifies `presented` against `context` the way both `first_contact` and
/// `genuine_reboot` require (ADR-0014 point 7): correct kind, correct
/// lookup id, the `BootContext` still admits a first redemption at `now`,
/// and the secret verifies against the stored one-way verifier.
fn boot_context_credential_is_valid(
    context: &BootContext,
    presented: &PresentedCredential,
    now: DateTime<Utc>,
) -> bool {
    presented.kind() == CredentialKind::Enrollment
        && presented.lookup_id() == context.boot_context_id()
        && context.valid_at(now)
        && context.verify_secret(presented.secret())
}

/// `(no record) -> PendingEnrollment`, atomic with
/// `NoActiveCredential -> CredentialActive`
/// (`m0-endpoint-identity-lifecycle.md` "Transitions";
/// `docs/decisions/0012-...md` point 1; ADR-0014 point 5).
///
/// `context` must be unresolved — callers only reach this function through
/// `RedemptionTarget::UnresolvedBootContext { candidate_endpoint: None, .. }`
/// routing, but this function still verifies it explicitly rather than
/// trusting the caller. Every other credential check (kind, lookup id,
/// expiry, secret) is performed here too — this function does not trust an
/// already-verified `presented` the way the pre-ADR-0014 version did.
///
/// Returns `Err` only if [`BootContext::resolve`] itself reports a conflict
/// (a different Endpoint already resolved this `BootContext`) — a Domain
/// invariant failure the Adapter must roll back on, never collapsed into a
/// generic `Rejected` (ADR-0014 checkpoint "BootContext resolution error").
pub fn first_contact(
    context: &BootContext,
    presented: &PresentedCredential,
    fresh_successor: &PresentedCredential,
    now: DateTime<Utc>,
    ttl: Duration,
) -> Result<RedeemOutcome, BootContextResolveError> {
    if context.resolved_endpoint_id().is_some() {
        return Ok(RedeemOutcome::Rejected);
    }
    if !boot_context_credential_is_valid(context, presented, now) {
        return Ok(RedeemOutcome::Rejected);
    }

    let Some(chain) = CredentialChain::establish(
        presented.lookup_id().clone(),
        context.verifier().clone(),
        fresh_successor,
        now,
        ttl,
    ) else {
        return Ok(RedeemOutcome::Rejected);
    };

    let id = EndpointId::new();
    let aggregate = EndpointAggregate {
        id,
        inventory_signal: context.inventory_signal().to_string(),
        identity: IdentityState::PendingEnrollment,
        credential: chain,
        created_at: now,
        updated_at: now,
    };

    let event = DomainEvent::EndpointPendingEnrollment {
        event_id: Uuid::new_v4(),
        endpoint_id: id,
        occurred_at: now,
    };

    let resolved_context = context.resolve(id)?;

    Ok(RedeemOutcome::Established {
        outcome: TransitionOutcome {
            endpoint: aggregate,
            events: vec![event],
            audit: None,
        },
        issued: fresh_successor.clone(),
        issued_expires_at: now + ttl,
        first_contact: true,
        successor_confirmed: false,
        resolved_boot_context: Some(resolved_context),
    })
}

/// Redeems a credential against a known Endpoint's existing chain. Routine
/// rotation while remaining `CredentialActive` is durable bookkeeping, not a
/// new domain event (ADR-0012 point 9) — the returned [`TransitionOutcome`]
/// therefore carries no events on `Accepted`.
///
/// A rejected `AuthRequest` is explicitly not a domain-state transition
/// requiring persistence (`m0-endpoint-identity-lifecycle.md`; WP1 acceptance
/// criteria) — callers must not persist anything for [`RedeemOutcome::Rejected`].
pub fn redeem_known(
    aggregate: &EndpointAggregate,
    presented: &PresentedCredential,
    fresh_successor: &PresentedCredential,
    now: DateTime<Utc>,
    ttl: Duration,
) -> RedeemOutcome {
    match credential::authenticate(&aggregate.credential, presented, fresh_successor, now, ttl) {
        AuthOutcome::Accepted {
            chain,
            issued_expires_at,
            successor_confirmed,
        } => RedeemOutcome::Established {
            outcome: TransitionOutcome {
                endpoint: EndpointAggregate {
                    credential: chain,
                    updated_at: now,
                    ..aggregate.clone()
                },
                events: vec![],
                audit: None,
            },
            issued: fresh_successor.clone(),
            issued_expires_at,
            first_contact: false,
            successor_confirmed,
            resolved_boot_context: None,
        },
        AuthOutcome::Rejected => RedeemOutcome::Rejected,
    }
}

/// Genuine reboot of an already-known Endpoint (ADR-0012 point 1):
///
/// ```text
/// same boot:      E1 -> R1 -> R2 -> ...
/// genuine reboot: E2 (new enrollment credential) -> fresh runtime credential -> ...
/// ```
///
/// A fresh, valid boot-scoped enrollment credential, correlated by the
/// Adapter to `candidate` via `BootContext.inventory_signal`, establishes a
/// brand-new runtime-credential chain for the Endpoint's existing durable
/// identity — the old chain is superseded in full, exactly as
/// `CredentialChain::establish` already does for a first-seen Endpoint,
/// since "a runtime credential does not need to survive a genuine Agent
/// reboot" (ADR-0012 point 1). Endpoint identity/lifecycle state
/// (`PendingEnrollment`/`Enrolled`/`Retired`) is preserved unchanged: this is
/// a credential-dimension transition only, never a re-run of operator
/// approval (ADR-0004 "Reconnect handling"; `m0-endpoint-identity-lifecycle.md`
/// "Reconnect / credential renewal handling"). No domain event is emitted,
/// for the same reason routine rotation emits none (ADR-0012 point 9).
///
/// Rejects outright — without attempting to establish anything — when
/// `candidate`'s current chain is `CredentialRevoked`: `CredentialRevoked` is
/// durable Endpoint-level state that survives a genuine reboot, and a fresh
/// `E2` does not clear it (owner-approved policy, ADR-0012 point 8 /
/// "Consequences"; restoring `CredentialActive` requires a separate,
/// explicit, authorized reactivation operation, not designed here).
///
/// Returns `Err` only if [`BootContext::resolve`] itself reports a conflict,
/// exactly like [`first_contact`].
pub fn genuine_reboot(
    context: &BootContext,
    candidate: &EndpointAggregate,
    presented: &PresentedCredential,
    fresh_successor: &PresentedCredential,
    now: DateTime<Utc>,
    ttl: Duration,
) -> Result<RedeemOutcome, BootContextResolveError> {
    if !boot_context_credential_is_valid(context, presented, now) {
        return Ok(RedeemOutcome::Rejected);
    }
    if candidate.credential.is_revoked() {
        return Ok(RedeemOutcome::Rejected);
    }

    let Some(chain) = CredentialChain::establish(
        presented.lookup_id().clone(),
        context.verifier().clone(),
        fresh_successor,
        now,
        ttl,
    ) else {
        return Ok(RedeemOutcome::Rejected);
    };

    let resolved_context = context.resolve(candidate.id)?;

    Ok(RedeemOutcome::Established {
        outcome: TransitionOutcome {
            endpoint: EndpointAggregate {
                credential: chain,
                updated_at: now,
                ..candidate.clone()
            },
            events: vec![],
            audit: None,
        },
        issued: fresh_successor.clone(),
        issued_expires_at: now + ttl,
        first_contact: false,
        successor_confirmed: false,
        resolved_boot_context: Some(resolved_context),
    })
}

/// `PendingEnrollment -> Enrolled`, atomic with `EndpointEnrolled` and
/// `OperatorDecisionRecorded`, plus the required immutable audit record
/// (`m0-persistence-observability-and-domain-events.md` "Auditability";
/// ADR-0004 "Decision: operator-approval-gated first enrollment").
pub fn approve_enrollment(
    aggregate: &EndpointAggregate,
    operator: Actor,
    now: DateTime<Utc>,
) -> Result<TransitionOutcome, InvalidIdentityTransition> {
    let new_identity = identity::approve(aggregate.identity)?;

    let enrolled_event = DomainEvent::EndpointEnrolled {
        event_id: Uuid::new_v4(),
        endpoint_id: aggregate.id,
        occurred_at: now,
    };
    let decision_event = DomainEvent::OperatorDecisionRecorded {
        event_id: Uuid::new_v4(),
        endpoint_id: aggregate.id,
        decision: "EnrollmentApproved".to_string(),
        actor: operator.clone(),
        occurred_at: now,
    };
    let audit = AuditRecord {
        audit_id: Uuid::new_v4(),
        endpoint_id: aggregate.id,
        actor: operator,
        occurred_at: now,
        detail: "EnrollmentApproved".to_string(),
    };

    Ok(TransitionOutcome {
        endpoint: EndpointAggregate {
            identity: new_identity,
            updated_at: now,
            ..aggregate.clone()
        },
        events: vec![enrolled_event, decision_event],
        audit: Some(audit),
    })
}

/// Explicit `CredentialRevoked`: invalidates every credential still valid in
/// the chain (ADR-0012 point 8). WP1 exercises this at the domain/persistence
/// layer directly (Issue #17 "Safety constraints"), with no operator-facing
/// revocation control path introduced — accordingly no domain event or audit
/// record is attributed here; a future Work Package that introduces a real
/// revocation control path owns deciding whether one is required.
pub fn revoke_credential(aggregate: &EndpointAggregate, now: DateTime<Utc>) -> TransitionOutcome {
    TransitionOutcome {
        endpoint: EndpointAggregate {
            credential: aggregate.credential.revoke(),
            updated_at: now,
            ..aggregate.clone()
        },
        events: vec![],
        audit: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credential::{CredentialHash, DEFAULT_CREDENTIAL_TTL};

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    fn fresh_runtime() -> PresentedCredential {
        PresentedCredential::generate(CredentialKind::Runtime)
    }

    /// Builds an unresolved `BootContext` plus the matching enrollment
    /// `PresentedCredential` that verifies against it — the pure-Domain
    /// stand-in for what `BootOrchestrationService::issue_enrollment_credential`
    /// produces end to end.
    fn issue_boot_context(
        inventory_signal: &str,
        now: DateTime<Utc>,
        ttl: Duration,
    ) -> (BootContext, PresentedCredential) {
        let credential = PresentedCredential::generate(CredentialKind::Enrollment);
        let verifier = CredentialHash::of_bytes(credential.secret().expose_secret_bytes());
        let context = BootContext::new(
            credential.lookup_id().clone(),
            verifier,
            now,
            now + ttl,
            inventory_signal.to_string(),
        );
        (context, credential)
    }

    #[test]
    fn first_contact_creates_pending_enrollment_with_one_event() {
        let (context, e1) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        let outcome = first_contact(
            &context,
            &e1,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .expect("fresh BootContext resolves cleanly");
        match outcome {
            RedeemOutcome::Established {
                outcome,
                first_contact,
                resolved_boot_context,
                ..
            } => {
                assert!(first_contact);
                assert_eq!(outcome.endpoint.identity, IdentityState::PendingEnrollment);
                assert_eq!(outcome.events.len(), 1);
                assert_eq!(outcome.events[0].event_type(), "EndpointPendingEnrollment");
                assert!(outcome.audit.is_none());
                assert_eq!(
                    resolved_boot_context.unwrap().resolved_endpoint_id(),
                    Some(outcome.endpoint.id)
                );
            }
            RedeemOutcome::Rejected => panic!("first contact must be established"),
        }
    }

    #[test]
    fn first_contact_rejects_wrong_kind_presented_credential() {
        let (context, e1) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        // Same lookup id and secret, but wrong kind on the wire.
        let wrong_kind_wire = e1.to_wire_value().replacen(".enrollment.", ".runtime.", 1);
        let wrong_kind = PresentedCredential::parse(&wrong_kind_wire).unwrap();
        let outcome = first_contact(
            &context,
            &wrong_kind,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .unwrap();
        assert!(matches!(outcome, RedeemOutcome::Rejected));
    }

    #[test]
    fn first_contact_rejects_wrong_secret() {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};

        let (context, e1) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        // Different secret, but forged to carry E1's real lookup id.
        let other = PresentedCredential::generate(CredentialKind::Enrollment);
        let wire = format!(
            "v1.enrollment.{}.{}",
            URL_SAFE_NO_PAD.encode(e1.lookup_id().to_bytes()),
            URL_SAFE_NO_PAD.encode(other.secret().expose_secret_bytes())
        );
        let forged = PresentedCredential::parse(&wire).unwrap();
        let outcome = first_contact(
            &context,
            &forged,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .unwrap();
        assert!(matches!(outcome, RedeemOutcome::Rejected));
    }

    #[test]
    fn first_contact_rejects_an_expired_boot_context() {
        let past = now() - Duration::minutes(10);
        let (context, e1) = issue_boot_context("mac:AA:BB", past, Duration::minutes(5));
        let outcome = first_contact(
            &context,
            &e1,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .unwrap();
        assert!(matches!(outcome, RedeemOutcome::Rejected));
    }

    #[test]
    fn first_contact_rejects_an_already_resolved_boot_context() {
        let (context, e1) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        let resolved = context.resolve(EndpointId::new()).unwrap();
        let outcome = first_contact(
            &resolved,
            &e1,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .unwrap();
        assert!(matches!(outcome, RedeemOutcome::Rejected));
    }

    #[test]
    fn redeem_known_rotation_produces_no_events() {
        let (context, e1) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        let r1 = fresh_runtime();
        let RedeemOutcome::Established { outcome: first, .. } =
            first_contact(&context, &e1, &r1, now(), DEFAULT_CREDENTIAL_TTL).unwrap()
        else {
            panic!()
        };

        let outcome = redeem_known(
            &first.endpoint,
            &r1,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        );
        match outcome {
            RedeemOutcome::Established {
                outcome,
                successor_confirmed,
                resolved_boot_context,
                ..
            } => {
                assert!(successor_confirmed);
                assert!(
                    outcome.events.is_empty(),
                    "routine rotation must not emit a domain event"
                );
                assert!(outcome.audit.is_none());
                assert!(resolved_boot_context.is_none());
            }
            RedeemOutcome::Rejected => panic!("R1 must authenticate"),
        }
    }

    #[test]
    fn genuine_reboot_reestablishes_chain_preserving_identity_and_id() {
        let (context1, e1) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        let RedeemOutcome::Established { outcome: first, .. } = first_contact(
            &context1,
            &e1,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .unwrap() else {
            panic!()
        };
        let original_id = first.endpoint.id;

        let (context2, e2) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        let r_after_reboot = fresh_runtime();
        let RedeemOutcome::Established {
            outcome: rebooted, ..
        } = genuine_reboot(
            &context2,
            &first.endpoint,
            &e2,
            &r_after_reboot,
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .unwrap()
        else {
            panic!("genuine reboot must establish a fresh chain")
        };

        assert_eq!(
            rebooted.endpoint.id, original_id,
            "identity must be preserved across reboot"
        );
        assert_eq!(
            rebooted.endpoint.identity, first.endpoint.identity,
            "identity lifecycle state must not change on genuine reboot"
        );
        assert!(
            rebooted.events.is_empty(),
            "genuine reboot must not emit a domain event"
        );
        assert!(rebooted.audit.is_none());

        // The fresh successor from the reboot authenticates against the new
        // chain...
        assert!(matches!(
            credential::authenticate(
                &rebooted.endpoint.credential,
                &r_after_reboot,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Accepted { .. }
        ));
        // ...but nothing from the pre-reboot chain does: it was fully
        // superseded, not merged.
        assert_eq!(
            credential::authenticate(
                &rebooted.endpoint.credential,
                &e1,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Rejected
        );
    }

    #[test]
    fn genuine_reboot_rejects_when_current_chain_is_revoked() {
        let (context1, e1) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        let RedeemOutcome::Established { outcome: first, .. } = first_contact(
            &context1,
            &e1,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .unwrap() else {
            panic!()
        };
        let revoked = revoke_credential(&first.endpoint, now());

        let (context2, e2) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        let outcome = genuine_reboot(
            &context2,
            &revoked.endpoint,
            &e2,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .unwrap();
        assert!(matches!(outcome, RedeemOutcome::Rejected));
    }

    #[test]
    fn rejected_redemption_yields_no_transition_to_persist() {
        let (context, e1) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        let RedeemOutcome::Established { outcome: first, .. } = first_contact(
            &context,
            &e1,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .unwrap() else {
            panic!()
        };

        let bogus = PresentedCredential::generate(CredentialKind::Runtime);
        assert!(matches!(
            redeem_known(
                &first.endpoint,
                &bogus,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            RedeemOutcome::Rejected
        ));
    }

    #[test]
    fn approve_enrollment_emits_enrolled_and_operator_decision_with_audit() {
        let (context, e1) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        let RedeemOutcome::Established { outcome: first, .. } = first_contact(
            &context,
            &e1,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .unwrap() else {
            panic!()
        };

        let operator = Actor::Operator {
            label: "wp1-harness".into(),
        };
        let outcome =
            approve_enrollment(&first.endpoint, operator, now()).expect("valid transition");

        assert_eq!(outcome.endpoint.identity, IdentityState::Enrolled);
        assert_eq!(outcome.events.len(), 2);
        assert!(outcome
            .events
            .iter()
            .any(|e| e.event_type() == "EndpointEnrolled"));
        assert!(outcome
            .events
            .iter()
            .any(|e| e.event_type() == "OperatorDecisionRecorded"));
        assert!(outcome.audit.is_some());
    }

    #[test]
    fn approve_enrollment_rejects_already_enrolled() {
        let (context, e1) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        let RedeemOutcome::Established { outcome: first, .. } = first_contact(
            &context,
            &e1,
            &fresh_runtime(),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        .unwrap() else {
            panic!()
        };
        let operator = Actor::Operator {
            label: "wp1-harness".into(),
        };
        let enrolled = approve_enrollment(&first.endpoint, operator.clone(), now()).unwrap();

        assert!(approve_enrollment(&enrolled.endpoint, operator, now()).is_err());
    }

    #[test]
    fn revoke_invalidates_every_credential_in_chain_with_no_new_event() {
        let (context, e1) = issue_boot_context("mac:AA:BB", now(), Duration::minutes(5));
        let r1 = fresh_runtime();
        let RedeemOutcome::Established { outcome: first, .. } =
            first_contact(&context, &e1, &r1, now(), DEFAULT_CREDENTIAL_TTL).unwrap()
        else {
            panic!()
        };

        let revoked = revoke_credential(&first.endpoint, now());
        assert!(revoked.events.is_empty());

        assert!(matches!(
            redeem_known(
                &revoked.endpoint,
                &r1,
                &fresh_runtime(),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            RedeemOutcome::Rejected
        ));
    }

    #[test]
    fn promoted_predecessor_survives_boot_context_expiry() {
        // ADR-0014 point 6: after promotion, the predecessor is governed by
        // its own CredentialSlot expiry, not by BootContext.expires_at.
        let short_ttl = Duration::minutes(1);
        let (context, e1) = issue_boot_context("mac:AA:BB", now(), short_ttl);
        let long_credential_ttl = Duration::hours(1);
        let RedeemOutcome::Established { outcome: first, .. } =
            first_contact(&context, &e1, &fresh_runtime(), now(), long_credential_ttl).unwrap()
        else {
            panic!()
        };

        // Well past the BootContext's own short TTL, but within the
        // promoted predecessor's much longer credential TTL.
        let later = now() + Duration::minutes(30);
        let retry_successor = fresh_runtime();
        let outcome = redeem_known(
            &first.endpoint,
            &e1,
            &retry_successor,
            later,
            long_credential_ttl,
        );
        assert!(matches!(outcome, RedeemOutcome::Established { .. }));
    }
}
