//! Repository Port (`m0-stack-and-boundaries-baseline.md` "Component
//! responsibilities and boundaries" — Ports: repositories). Application and
//! Domain depend only on this trait; concrete persistence lives entirely in
//! `adapters::sqlite` (ADR-0007 "Repository port/adapter boundary preserved
//! for a future PostgreSQL adapter").

use async_trait::async_trait;
use bamep_domain::{EndpointAggregate, EndpointId, TransitionOutcome};

#[derive(Debug, thiserror::Error)]
pub enum RepositoryError {
    #[error("persistence backend error: {0}")]
    Backend(String),
}

#[async_trait]
pub trait EndpointRepository: Send + Sync {
    async fn find_by_inventory_signal(
        &self,
        signal: &str,
    ) -> Result<Option<EndpointAggregate>, RepositoryError>;

    async fn find_by_id(
        &self,
        id: EndpointId,
    ) -> Result<Option<EndpointAggregate>, RepositoryError>;

    /// Persists a [`TransitionOutcome`] — the endpoint's new state, any
    /// domain events, and any audit record — atomically in a single
    /// transaction (ADR-0007 "Transactional consistency between domain
    /// state, domain events, and audit records"). Never called for a
    /// rejected redemption, which produces no transition to persist.
    async fn commit(&self, outcome: TransitionOutcome) -> Result<(), RepositoryError>;
}
