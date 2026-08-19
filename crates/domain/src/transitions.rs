//! Assembles the pure state-machine modules (`identity`, `credential`) into
//! the concrete durable transitions WP1 requires, each producing a
//! [`TransitionOutcome`] ready for atomic persistence
//! (ADR-0013 "PostgreSQL persistence backend baseline", carrying forward
//! ADR-0007's "Transactional consistency between domain state, domain
//! events, and audit records").
//!
//! Every [`TransitionOutcome`] returned here is constructed exclusively by
//! this module, so an Application-layer caller can never hand-assemble a
//! domain aggregate into an invariant-violating shape.

use chrono::{DateTime, Duration, Utc};
use uuid::Uuid;

use crate::credential::{self, AuthOutcome, CredentialChain, CredentialSecret};
use crate::endpoint::{EndpointAggregate, EndpointId};
use crate::events::{Actor, AuditRecord, DomainEvent, TransitionOutcome};
use crate::identity::{self, IdentityState, InvalidIdentityTransition};

/// Result of redeeming a credential in a fresh `AuthRequest`, whether for a
/// first-seen Endpoint or a known one.
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
        issued: CredentialSecret,
        issued_expires_at: DateTime<Utc>,
        /// True only when this redemption is the Endpoint's very first
        /// (i.e. this outcome also creates `PendingEnrollment`).
        first_contact: bool,
        successor_confirmed: bool,
    },
    Rejected,
}

/// `(no record) -> PendingEnrollment`, atomic with
/// `NoActiveCredential -> CredentialActive`
/// (`m0-endpoint-identity-lifecycle.md` "Transitions";
/// `docs/decisions/0012-...md` point 1).
///
/// The caller must have already independently verified `presented` as a
/// legitimate, unexpired enrollment credential (`credential::enrollment::verify`)
/// before calling this function — this function unconditionally establishes
/// the chain, exactly as `authenticate` unconditionally accepts a
/// verified-valid predecessor.
pub fn first_contact(
    inventory_signal: String,
    presented: CredentialSecret,
    now: DateTime<Utc>,
    ttl: Duration,
) -> RedeemOutcome {
    let id = EndpointId::new();
    let (chain, issued) = CredentialChain::establish(presented, now, ttl);
    let issued_expires_at = now + ttl;

    let aggregate = EndpointAggregate {
        id,
        inventory_signal,
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

    RedeemOutcome::Established {
        outcome: TransitionOutcome {
            endpoint: aggregate,
            events: vec![event],
            audit: None,
        },
        issued,
        issued_expires_at,
        first_contact: true,
        successor_confirmed: false,
    }
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
    presented: &CredentialSecret,
    now: DateTime<Utc>,
    ttl: Duration,
) -> RedeemOutcome {
    match credential::authenticate(&aggregate.credential, presented, now, ttl) {
        AuthOutcome::Accepted {
            chain,
            issued,
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
            issued,
            issued_expires_at,
            first_contact: false,
            successor_confirmed,
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
/// A fresh, valid boot-scoped enrollment credential establishes a brand-new
/// runtime-credential chain for the Endpoint's existing durable identity —
/// the old chain is superseded in full, exactly as `CredentialChain::establish`
/// already does for a first-seen Endpoint, since "a runtime credential does
/// not need to survive a genuine Agent reboot" (ADR-0012 point 1). Endpoint
/// identity/lifecycle state (`PendingEnrollment`/`Enrolled`/`Retired`) is
/// preserved unchanged: this is a credential-dimension transition only,
/// never a re-run of operator approval (ADR-0004 "Reconnect handling";
/// `m0-endpoint-identity-lifecycle.md` "Reconnect / credential renewal
/// handling"). No domain event is emitted, for the same reason routine
/// rotation emits none (ADR-0012 point 9) — the credential dimension does
/// not itself change value across a genuine reboot, it is simply
/// re-established.
///
/// The caller must have already independently verified `presented` as a
/// legitimate, unexpired enrollment credential (`credential::enrollment::verify`),
/// exactly as `first_contact` requires. The caller must also have already
/// confirmed the current chain is not `CredentialRevoked` — this function
/// does not check that itself and must not be called for a revoked chain:
/// `CredentialRevoked` is durable Endpoint-level state that survives a
/// genuine reboot, and a fresh `E2` does not clear it (owner-approved
/// policy, ADR-0012 point 8 / "Consequences"; restoring `CredentialActive`
/// requires a separate, explicit, authorized reactivation operation, not
/// designed here).
pub fn genuine_reboot(
    aggregate: &EndpointAggregate,
    presented: CredentialSecret,
    now: DateTime<Utc>,
    ttl: Duration,
) -> RedeemOutcome {
    let (chain, issued) = CredentialChain::establish(presented, now, ttl);
    let issued_expires_at = now + ttl;

    RedeemOutcome::Established {
        outcome: TransitionOutcome {
            endpoint: EndpointAggregate {
                credential: chain,
                updated_at: now,
                ..aggregate.clone()
            },
            events: vec![],
            audit: None,
        },
        issued,
        issued_expires_at,
        first_contact: false,
        successor_confirmed: false,
    }
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
    use crate::credential::DEFAULT_CREDENTIAL_TTL;

    fn now() -> DateTime<Utc> {
        Utc::now()
    }

    #[test]
    fn first_contact_creates_pending_enrollment_with_one_event() {
        let outcome = first_contact(
            "mac:AA:BB".into(),
            CredentialSecret("e1".into()),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        );
        match outcome {
            RedeemOutcome::Established {
                outcome,
                first_contact,
                ..
            } => {
                assert!(first_contact);
                assert_eq!(outcome.endpoint.identity, IdentityState::PendingEnrollment);
                assert_eq!(outcome.events.len(), 1);
                assert_eq!(outcome.events[0].event_type(), "EndpointPendingEnrollment");
                assert!(outcome.audit.is_none());
            }
            RedeemOutcome::Rejected => panic!("first contact must be established"),
        }
    }

    #[test]
    fn redeem_known_rotation_produces_no_events() {
        let RedeemOutcome::Established {
            outcome: first,
            issued: r1,
            ..
        } = first_contact(
            "mac:AA:BB".into(),
            CredentialSecret("e1".into()),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        else {
            panic!()
        };

        let outcome = redeem_known(&first.endpoint, &r1, now(), DEFAULT_CREDENTIAL_TTL);
        match outcome {
            RedeemOutcome::Established {
                outcome,
                successor_confirmed,
                ..
            } => {
                assert!(successor_confirmed);
                assert!(
                    outcome.events.is_empty(),
                    "routine rotation must not emit a domain event"
                );
                assert!(outcome.audit.is_none());
            }
            RedeemOutcome::Rejected => panic!("R1 must authenticate"),
        }
    }

    #[test]
    fn genuine_reboot_reestablishes_chain_preserving_identity_and_id() {
        let RedeemOutcome::Established { outcome: first, .. } = first_contact(
            "mac:AA:BB".into(),
            CredentialSecret("e1".into()),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        ) else {
            panic!()
        };
        let original_id = first.endpoint.id;

        let RedeemOutcome::Established {
            outcome: rebooted,
            issued: e2_issued,
            ..
        } = genuine_reboot(
            &first.endpoint,
            CredentialSecret("e2".into()),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
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
                &e2_issued,
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
                &CredentialSecret("e1".into()),
                now(),
                DEFAULT_CREDENTIAL_TTL
            ),
            AuthOutcome::Rejected
        );
    }

    #[test]
    fn rejected_redemption_yields_no_transition_to_persist() {
        let RedeemOutcome::Established { outcome: first, .. } = first_contact(
            "mac:AA:BB".into(),
            CredentialSecret("e1".into()),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        ) else {
            panic!()
        };

        let bogus = CredentialSecret("not-issued".into());
        assert!(matches!(
            redeem_known(&first.endpoint, &bogus, now(), DEFAULT_CREDENTIAL_TTL),
            RedeemOutcome::Rejected
        ));
    }

    #[test]
    fn approve_enrollment_emits_enrolled_and_operator_decision_with_audit() {
        let RedeemOutcome::Established { outcome: first, .. } = first_contact(
            "mac:AA:BB".into(),
            CredentialSecret("e1".into()),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        ) else {
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
        let RedeemOutcome::Established { outcome: first, .. } = first_contact(
            "mac:AA:BB".into(),
            CredentialSecret("e1".into()),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        ) else {
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
        let RedeemOutcome::Established {
            outcome: first,
            issued: r1,
            ..
        } = first_contact(
            "mac:AA:BB".into(),
            CredentialSecret("e1".into()),
            now(),
            DEFAULT_CREDENTIAL_TTL,
        )
        else {
            panic!()
        };

        let revoked = revoke_credential(&first.endpoint, now());
        assert!(revoked.events.is_empty());

        assert!(matches!(
            redeem_known(&revoked.endpoint, &r1, now(), DEFAULT_CREDENTIAL_TTL),
            RedeemOutcome::Rejected
        ));
    }
}
