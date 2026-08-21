//! `EndpointRepository` Port implementation against real PostgreSQL.
//!
//! `update_endpoint` locks the affected `endpoints` row before invoking the
//! caller-supplied decision closure, and commits the closure's result in the
//! same transaction that held the lock — see `crate::ports` module docs for
//! the full rationale. Credential redemption (`AuthRequest` routing/locking)
//! is not this Adapter's responsibility; see
//! `super::credential_redemption_repository`.

use async_trait::async_trait;
use bamep_domain::{EndpointAggregate, EndpointId, TransitionOutcome};
use sqlx::PgPool;

use super::shared::{find_by_id, load_by_id_for_update, persist_transition, to_backend_err};
use crate::ports::{EndpointRepository, EndpointUpdateError, RepositoryError, UpdateDecision};

pub struct PostgresEndpointRepository {
    pool: PgPool,
}

impl PostgresEndpointRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl EndpointRepository for PostgresEndpointRepository {
    async fn find_by_id(
        &self,
        id: EndpointId,
    ) -> Result<Option<EndpointAggregate>, RepositoryError> {
        find_by_id(&self.pool, id).await
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
