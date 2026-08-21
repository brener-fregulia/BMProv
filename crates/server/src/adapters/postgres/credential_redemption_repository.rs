//! `CredentialRedemptionRepository` Port implementation against real
//! PostgreSQL (ADR-0014 "Routing/locking algorithm").
//!
//! One transaction per `redeem` call. Lock order (never violated, so no
//! path that already holds an Endpoint lock later waits on a `BootContext`):
//!
//! ```text
//! runtime/persisted mapping:      Endpoint
//! unresolved Enrollment:          BootContext -> inventory_signal advisory lock -> candidate Endpoint
//! already-resolved BootContext:   BootContext -> resolved Endpoint
//! ```
//!
//! Routing:
//!
//! 1. `SELECT endpoint_id FROM endpoint_credential_lookups WHERE lookup_id =
//!    $1` — an unlocked read; this mapping alone never authenticates
//!    anything (ADR-0014 point 7), so no lock is taken on it before the
//!    Endpoint lock below.
//! 2. If found: lock/reconstruct the owning Endpoint and route to
//!    [`RedemptionTarget::Endpoint`]. The Domain re-check of `lookup_id`
//!    against the now-locked, freshly re-read chain protects against a
//!    mapping that went stale while this transaction waited for the lock.
//! 3. If not found and `kind` is `Runtime`: route to
//!    [`RedemptionTarget::UnknownCredential`] without ever touching
//!    `boot_contexts` — a Runtime credential must never fall through to
//!    `BootContext` (ADR-0014 point 7).
//! 4. If not found and `kind` is `Enrollment`: lock/re-read the
//!    `BootContext` by `boot_context_id`. Absent ->
//!    [`RedemptionTarget::UnknownBootContext`]. Already resolved -> lock/load
//!    the resolved Endpoint and route to `RedemptionTarget::Endpoint`
//!    (never re-running first-contact resolution). Unresolved -> acquire the
//!    `inventory_signal` advisory lock, load a candidate Endpoint under lock
//!    if one exists, and route to `RedemptionTarget::UnresolvedBootContext`.
//!
//! On an accepted decision, this Adapter persists state + domain events +
//! audit record (`shared::persist_transition`), the final credential-lookup
//! projection (`shared::persist_lookup_projection`), and — when the outcome
//! resolved a `BootContext` — its `resolved_endpoint_id`
//! (`shared::persist_boot_context_resolution`), all in the one transaction,
//! before committing.

use async_trait::async_trait;
use bamep_domain::presented_credential::{CredentialKind, CredentialLookupId};
use bamep_domain::{EndpointId, RedeemOutcome};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use super::shared::{
    load_boot_context_for_update, load_by_id_for_update, load_by_signal_for_update,
    persist_boot_context_resolution, persist_lookup_projection, persist_transition, to_backend_err,
};
use crate::ports::{
    CredentialRedemptionRepository, RedemptionDecision, RedemptionTarget, RepositoryError,
};

pub struct PostgresCredentialRedemptionRepository {
    pool: PgPool,
}

impl PostgresCredentialRedemptionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

/// Routes `(kind, lookup_id)` to its [`RedemptionTarget`], acquiring every
/// lock that target's decision may need — see this module's doc comment for
/// the full algorithm and lock order.
async fn route(
    tx: &mut Transaction<'_, Postgres>,
    kind: CredentialKind,
    lookup_id: &CredentialLookupId,
) -> Result<RedemptionTarget, RepositoryError> {
    let lookup_bytes = lookup_id.to_bytes().to_vec();

    let mapped_endpoint_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT endpoint_id FROM endpoint_credential_lookups WHERE lookup_id = $1",
    )
    .bind(&lookup_bytes)
    .fetch_optional(&mut **tx)
    .await
    .map_err(to_backend_err)?;

    if let Some(endpoint_id) = mapped_endpoint_id {
        let aggregate = load_by_id_for_update(tx, EndpointId(endpoint_id))
            .await?
            .ok_or_else(|| {
                RepositoryError::Backend(format!(
                    "credential lookup mapping referenced missing endpoint {endpoint_id}"
                ))
            })?;
        return Ok(RedemptionTarget::Endpoint(aggregate));
    }

    if kind == CredentialKind::Runtime {
        // A Runtime credential never falls through to a BootContext lookup
        // (ADR-0014 point 7) — no boot_contexts query at all here.
        return Ok(RedemptionTarget::UnknownCredential);
    }

    let Some(context) = load_boot_context_for_update(tx, &lookup_bytes).await? else {
        return Ok(RedemptionTarget::UnknownBootContext);
    };

    if let Some(resolved_id) = context.resolved_endpoint_id() {
        let aggregate = load_by_id_for_update(tx, resolved_id)
            .await?
            .ok_or_else(|| {
                RepositoryError::Backend(format!(
                    "resolved boot context referenced missing endpoint {resolved_id:?}"
                ))
            })?;
        return Ok(RedemptionTarget::Endpoint(aggregate));
    }

    // Unresolved: the BootContext row is already locked above. Acquire the
    // transaction-scoped correlation advisory lock before looking for a
    // candidate Endpoint by inventory_signal (ADR-0014 point 8 concurrency
    // correction; the same advisory-lock mechanism the pre-ADR-0014
    // transitional repository already used, moved to this stage).
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(context.inventory_signal())
        .execute(&mut **tx)
        .await
        .map_err(to_backend_err)?;

    let candidate_endpoint = load_by_signal_for_update(tx, context.inventory_signal()).await?;

    Ok(RedemptionTarget::UnresolvedBootContext {
        context,
        candidate_endpoint,
    })
}

#[async_trait]
impl CredentialRedemptionRepository for PostgresCredentialRedemptionRepository {
    async fn redeem(
        &self,
        kind: CredentialKind,
        lookup_id: &CredentialLookupId,
        decide: RedemptionDecision,
    ) -> Result<RedeemOutcome, RepositoryError> {
        let mut tx = self.pool.begin().await.map_err(to_backend_err)?;

        let target = route(&mut tx, kind, lookup_id).await?;
        let is_unknown = matches!(
            target,
            RedemptionTarget::UnknownBootContext | RedemptionTarget::UnknownCredential
        );

        match decide(target) {
            Ok(RedeemOutcome::Established {
                outcome,
                issued,
                issued_expires_at,
                first_contact,
                successor_confirmed,
                resolved_boot_context,
            }) => {
                if is_unknown {
                    tx.rollback().await.map_err(to_backend_err)?;
                    return Err(RepositoryError::Backend(
                        "internal contract violation: decision accepted a redemption for an \
                         unknown target"
                            .to_string(),
                    ));
                }

                persist_transition(&mut tx, &outcome).await?;
                persist_lookup_projection(
                    &mut tx,
                    outcome.endpoint.id,
                    &outcome.endpoint.credential,
                )
                .await?;
                if let Some(context) = &resolved_boot_context {
                    persist_boot_context_resolution(&mut tx, context).await?;
                }

                tx.commit().await.map_err(to_backend_err)?;

                Ok(RedeemOutcome::Established {
                    outcome,
                    issued,
                    issued_expires_at,
                    first_contact,
                    successor_confirmed,
                    resolved_boot_context,
                })
            }
            Ok(RedeemOutcome::Rejected) => {
                tx.rollback().await.map_err(to_backend_err)?;
                Ok(RedeemOutcome::Rejected)
            }
            Err(resolve_err) => {
                tx.rollback().await.map_err(to_backend_err)?;
                Err(RepositoryError::Backend(resolve_err.to_string()))
            }
        }
    }
}
