//! `BootContextRepository` Port implementation against real PostgreSQL
//! (ADR-0014 point 11 "Persist-before-deliver issuance ordering"). A single
//! `INSERT` executed directly against the pool: PostgreSQL's own statement
//! autocommit already gives an unambiguous "committed or not" outcome for
//! one statement, so no explicit transaction is needed here.
//!
//! `boot_context_id` is the table's `PRIMARY KEY`
//! (`../../../migrations/0003_boot_context_and_credential_lookup.sql`); this
//! Adapter issues a plain `INSERT` with no `ON CONFLICT` clause, so a
//! colliding `boot_context_id` fails the statement instead of overwriting
//! the existing row.

use async_trait::async_trait;
use bamep_domain::BootContext;
use sqlx::PgPool;

use crate::ports::{BootContextRepository, RepositoryError};

pub struct PostgresBootContextRepository {
    pool: PgPool,
}

impl PostgresBootContextRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BootContextRepository for PostgresBootContextRepository {
    async fn insert_boot_context(&self, context: &BootContext) -> Result<(), RepositoryError> {
        sqlx::query(
            r#"
            INSERT INTO boot_contexts (
                boot_context_id, verifier, issued_at, expires_at, inventory_signal, resolved_endpoint_id
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            "#,
        )
        .bind(context.boot_context_id().to_bytes().to_vec())
        .bind(context.verifier().to_bytes().to_vec())
        .bind(context.issued_at())
        .bind(context.expires_at())
        .bind(context.inventory_signal())
        .bind(context.resolved_endpoint_id().map(|id| id.0))
        .execute(&self.pool)
        .await
        .map_err(|e| RepositoryError::Backend(e.to_string()))?;

        Ok(())
    }
}
