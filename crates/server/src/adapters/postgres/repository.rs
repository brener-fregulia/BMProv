//! `EndpointRepository` Port implementation against real PostgreSQL.
//!
//! Every write path locks the affected `endpoints` row (or, for a
//! not-yet-existing first-contact row, a `pg_advisory_xact_lock` keyed by
//! the inventory signal) before invoking the caller-supplied decision
//! closure, and commits the closure's result in the same transaction that
//! held the lock. This is what satisfies ADR-0012's commit-time concurrency
//! requirement and the "duplicate first contact" race requirement — see
//! `crate::ports` module docs for the full rationale.

use async_trait::async_trait;
use bamep_domain::credential::{CredentialChain, CredentialHash, CredentialSlot};
use bamep_domain::{
    Actor, DomainEvent, EndpointAggregate, EndpointId, IdentityState, TransitionOutcome,
};
use sqlx::{PgPool, Postgres, Row, Transaction};

use crate::ports::{
    EndpointRepository, EndpointUpdateError, RedeemDecision, RepositoryError, UpdateDecision,
};

pub struct PostgresEndpointRepository {
    pool: PgPool,
}

impl PostgresEndpointRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn to_backend_err(e: sqlx::Error) -> RepositoryError {
    RepositoryError::Backend(e.to_string())
}

fn identity_state_to_str(state: IdentityState) -> &'static str {
    match state {
        IdentityState::PendingEnrollment => "PendingEnrollment",
        IdentityState::Enrolled => "Enrolled",
        IdentityState::Retired => "Retired",
    }
}

fn identity_state_from_str(s: &str) -> Result<IdentityState, RepositoryError> {
    match s {
        "PendingEnrollment" => Ok(IdentityState::PendingEnrollment),
        "Enrolled" => Ok(IdentityState::Enrolled),
        "Retired" => Ok(IdentityState::Retired),
        other => Err(RepositoryError::Backend(format!(
            "unrecognized identity_state {other:?} in durable storage"
        ))),
    }
}

fn verifier_from_bytes(bytes: Vec<u8>) -> Result<CredentialHash, RepositoryError> {
    let array: [u8; 48] = bytes.try_into().map_err(|bytes: Vec<u8>| {
        RepositoryError::Backend(format!(
            "credential verifier has unexpected length {} (expected 48)",
            bytes.len()
        ))
    })?;
    Ok(CredentialHash::from_bytes(array))
}

fn actor_columns(actor: &Actor) -> (&'static str, Option<&str>) {
    match actor {
        Actor::Operator { label } => ("operator", Some(label.as_str())),
        Actor::System => ("system", None),
    }
}

/// Event-type-specific remainder only — envelope fields (`event_id`,
/// `event_type`, `endpoint_id`, `occurred_at`) are already real columns
/// (ADR-0013 Sec.6: JSONB is for genuinely variable payload, not for hiding
/// queryable lifecycle/state).
fn event_payload(event: &DomainEvent) -> serde_json::Value {
    match event {
        DomainEvent::EndpointPendingEnrollment { .. } | DomainEvent::EndpointEnrolled { .. } => {
            serde_json::json!({})
        }
        DomainEvent::OperatorDecisionRecorded {
            decision, actor, ..
        } => {
            let (actor_kind, actor_label) = actor_columns(actor);
            serde_json::json!({
                "decision": decision,
                "actor_kind": actor_kind,
                "actor_label": actor_label,
            })
        }
    }
}

fn row_to_aggregate(row: &sqlx::postgres::PgRow) -> Result<EndpointAggregate, RepositoryError> {
    let id: uuid::Uuid = row.try_get("id").map_err(to_backend_err)?;
    let inventory_signal: String = row.try_get("inventory_signal").map_err(to_backend_err)?;
    let identity_state: String = row.try_get("identity_state").map_err(to_backend_err)?;
    let created_at = row.try_get("created_at").map_err(to_backend_err)?;
    let updated_at = row.try_get("updated_at").map_err(to_backend_err)?;

    let predecessor_verifier: Vec<u8> = row
        .try_get("predecessor_verifier")
        .map_err(to_backend_err)?;
    let predecessor = CredentialSlot {
        hash: verifier_from_bytes(predecessor_verifier)?,
        issued_at: row
            .try_get("predecessor_issued_at")
            .map_err(to_backend_err)?,
        expires_at: row
            .try_get("predecessor_expires_at")
            .map_err(to_backend_err)?,
    };

    let successor_verifier: Option<Vec<u8>> =
        row.try_get("successor_verifier").map_err(to_backend_err)?;
    let successor = match successor_verifier {
        Some(bytes) => Some(CredentialSlot {
            hash: verifier_from_bytes(bytes)?,
            issued_at: row.try_get("successor_issued_at").map_err(to_backend_err)?,
            expires_at: row
                .try_get("successor_expires_at")
                .map_err(to_backend_err)?,
        }),
        None => None,
    };
    let revoked: bool = row.try_get("revoked").map_err(to_backend_err)?;

    Ok(EndpointAggregate {
        id: EndpointId(id),
        inventory_signal,
        identity: identity_state_from_str(&identity_state)?,
        credential: CredentialChain::from_parts(predecessor, successor, revoked),
        created_at,
        updated_at,
    })
}

async fn load_by_signal_for_update(
    tx: &mut Transaction<'_, Postgres>,
    signal: &str,
) -> Result<Option<EndpointAggregate>, RepositoryError> {
    let row = sqlx::query(
        r#"
        SELECT e.id, e.inventory_signal, e.identity_state, e.created_at, e.updated_at,
               c.predecessor_verifier, c.predecessor_issued_at, c.predecessor_expires_at,
               c.successor_verifier, c.successor_issued_at, c.successor_expires_at, c.revoked
        FROM endpoints e
        JOIN endpoint_credentials c ON c.endpoint_id = e.id
        WHERE e.inventory_signal = $1
        FOR UPDATE OF e, c
        "#,
    )
    .bind(signal)
    .fetch_optional(&mut **tx)
    .await
    .map_err(to_backend_err)?;

    row.as_ref().map(row_to_aggregate).transpose()
}

async fn load_by_id_for_update(
    tx: &mut Transaction<'_, Postgres>,
    id: EndpointId,
) -> Result<Option<EndpointAggregate>, RepositoryError> {
    let row = sqlx::query(
        r#"
        SELECT e.id, e.inventory_signal, e.identity_state, e.created_at, e.updated_at,
               c.predecessor_verifier, c.predecessor_issued_at, c.predecessor_expires_at,
               c.successor_verifier, c.successor_issued_at, c.successor_expires_at, c.revoked
        FROM endpoints e
        JOIN endpoint_credentials c ON c.endpoint_id = e.id
        WHERE e.id = $1
        FOR UPDATE OF e, c
        "#,
    )
    .bind(id.0)
    .fetch_optional(&mut **tx)
    .await
    .map_err(to_backend_err)?;

    row.as_ref().map(row_to_aggregate).transpose()
}

/// Persists a [`TransitionOutcome`] (state + domain events + audit record)
/// within `tx`, without committing it — the caller decides commit/rollback.
async fn persist_transition(
    tx: &mut Transaction<'_, Postgres>,
    outcome: &TransitionOutcome,
) -> Result<(), RepositoryError> {
    let endpoint = &outcome.endpoint;

    sqlx::query(
        r#"
        INSERT INTO endpoints (id, inventory_signal, identity_state, created_at, updated_at)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (id) DO UPDATE SET
            identity_state = EXCLUDED.identity_state,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(endpoint.id.0)
    .bind(&endpoint.inventory_signal)
    .bind(identity_state_to_str(endpoint.identity))
    .bind(endpoint.created_at)
    .bind(endpoint.updated_at)
    .execute(&mut **tx)
    .await
    .map_err(to_backend_err)?;

    let predecessor = endpoint.credential.predecessor();
    let successor = endpoint.credential.successor();

    sqlx::query(
        r#"
        INSERT INTO endpoint_credentials (
            endpoint_id, predecessor_verifier, predecessor_issued_at, predecessor_expires_at,
            successor_verifier, successor_issued_at, successor_expires_at, revoked
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (endpoint_id) DO UPDATE SET
            predecessor_verifier = EXCLUDED.predecessor_verifier,
            predecessor_issued_at = EXCLUDED.predecessor_issued_at,
            predecessor_expires_at = EXCLUDED.predecessor_expires_at,
            successor_verifier = EXCLUDED.successor_verifier,
            successor_issued_at = EXCLUDED.successor_issued_at,
            successor_expires_at = EXCLUDED.successor_expires_at,
            revoked = EXCLUDED.revoked
        "#,
    )
    .bind(endpoint.id.0)
    .bind(predecessor.hash.to_bytes().to_vec())
    .bind(predecessor.issued_at)
    .bind(predecessor.expires_at)
    .bind(successor.as_ref().map(|s| s.hash.to_bytes().to_vec()))
    .bind(successor.as_ref().map(|s| s.issued_at))
    .bind(successor.as_ref().map(|s| s.expires_at))
    .bind(endpoint.credential.is_revoked())
    .execute(&mut **tx)
    .await
    .map_err(to_backend_err)?;

    for event in &outcome.events {
        sqlx::query(
            r#"
            INSERT INTO domain_events (event_id, event_type, event_version, endpoint_id, occurred_at, payload)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(event.event_id())
        .bind(event.event_type())
        .bind(1i32)
        .bind(event.endpoint_id().0)
        .bind(event.occurred_at())
        .bind(event_payload(event))
        .execute(&mut **tx)
        .await
        .map_err(to_backend_err)?;
    }

    if let Some(audit) = &outcome.audit {
        let (actor_kind, actor_label) = actor_columns(&audit.actor);
        sqlx::query(
            r#"
            INSERT INTO audit_records (audit_id, endpoint_id, actor_kind, actor_label, occurred_at, detail)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(audit.audit_id)
        .bind(audit.endpoint_id.0)
        .bind(actor_kind)
        .bind(actor_label)
        .bind(audit.occurred_at)
        .bind(&audit.detail)
        .execute(&mut **tx)
        .await
        .map_err(to_backend_err)?;
    }

    Ok(())
}

#[async_trait]
impl EndpointRepository for PostgresEndpointRepository {
    async fn find_by_id(
        &self,
        id: EndpointId,
    ) -> Result<Option<EndpointAggregate>, RepositoryError> {
        let row = sqlx::query(
            r#"
            SELECT e.id, e.inventory_signal, e.identity_state, e.created_at, e.updated_at,
                   c.predecessor_verifier, c.predecessor_issued_at, c.predecessor_expires_at,
                   c.successor_verifier, c.successor_issued_at, c.successor_expires_at, c.revoked
            FROM endpoints e
            JOIN endpoint_credentials c ON c.endpoint_id = e.id
            WHERE e.id = $1
            "#,
        )
        .bind(id.0)
        .fetch_optional(&self.pool)
        .await
        .map_err(to_backend_err)?;

        row.as_ref().map(row_to_aggregate).transpose()
    }

    async fn redeem(
        &self,
        inventory_signal: &str,
        decide: RedeemDecision,
    ) -> Result<bamep_domain::RedeemOutcome, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(to_backend_err)?;

        // Serializes every redemption (first-contact or reconnect) for this
        // inventory_signal within the transaction — released automatically
        // on commit/rollback, including on crash/abort. Handles the "no row
        // yet" first-contact race, which a row-level lock cannot express
        // since there is no row to lock.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
            .bind(inventory_signal)
            .execute(&mut *tx)
            .await
            .map_err(to_backend_err)?;

        let existing = load_by_signal_for_update(&mut tx, inventory_signal).await?;
        let outcome = decide(existing);

        match &outcome {
            bamep_domain::RedeemOutcome::Established { outcome, .. } => {
                persist_transition(&mut tx, outcome).await?;
                tx.commit().await.map_err(to_backend_err)?;
            }
            bamep_domain::RedeemOutcome::Rejected => {
                tx.rollback().await.map_err(to_backend_err)?;
            }
        }

        Ok(outcome)
    }

    async fn update_endpoint(
        &self,
        id: EndpointId,
        decide: UpdateDecision,
    ) -> Result<TransitionOutcome, EndpointUpdateError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| EndpointUpdateError::Repository(to_backend_err(e)))?;

        let existing = load_by_id_for_update(&mut tx, id)
            .await
            .map_err(EndpointUpdateError::Repository)?;

        let Some(aggregate) = existing else {
            tx.rollback()
                .await
                .map_err(|e| EndpointUpdateError::Repository(to_backend_err(e)))?;
            return Err(EndpointUpdateError::NotFound(id));
        };

        match decide(aggregate) {
            Ok(outcome) => {
                persist_transition(&mut tx, &outcome)
                    .await
                    .map_err(EndpointUpdateError::Repository)?;
                tx.commit()
                    .await
                    .map_err(|e| EndpointUpdateError::Repository(to_backend_err(e)))?;
                Ok(outcome)
            }
            Err(invalid) => {
                tx.rollback()
                    .await
                    .map_err(|e| EndpointUpdateError::Repository(to_backend_err(e)))?;
                Err(EndpointUpdateError::InvalidTransition(invalid))
            }
        }
    }
}
