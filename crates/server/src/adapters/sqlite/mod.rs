//! SQLite Adapter behind the `EndpointRepository` Port (ADR-0007 "SQLite is
//! accepted as the M0 persistence backend"). Domain and Application code
//! never reference `rusqlite` directly — only this module does.

use async_trait::async_trait;
use bamep_domain::{EndpointAggregate, EndpointId, TransitionOutcome};
use rusqlite::{params, Connection};
use tokio::sync::Mutex;

use crate::ports::{EndpointRepository, RepositoryError};

const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS endpoints (
    id TEXT PRIMARY KEY,
    inventory_signal TEXT NOT NULL UNIQUE,
    aggregate_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS domain_events (
    event_id TEXT PRIMARY KEY,
    event_type TEXT NOT NULL,
    endpoint_id TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    payload_json TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS audit_records (
    audit_id TEXT PRIMARY KEY,
    endpoint_id TEXT NOT NULL,
    occurred_at TEXT NOT NULL,
    payload_json TEXT NOT NULL
);
";

pub struct SqliteEndpointRepository {
    conn: Mutex<Connection>,
}

impl SqliteEndpointRepository {
    pub fn open(path: &str) -> rusqlite::Result<Self> {
        let conn = Connection::open(path)?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn open_in_memory() -> rusqlite::Result<Self> {
        let conn = Connection::open_in_memory()?;
        conn.execute_batch(SCHEMA)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    fn row_to_aggregate(json: String) -> Result<EndpointAggregate, RepositoryError> {
        serde_json::from_str(&json).map_err(|e| RepositoryError::Backend(e.to_string()))
    }
}

fn to_backend_err(e: rusqlite::Error) -> RepositoryError {
    RepositoryError::Backend(e.to_string())
}

fn commit_transaction(conn: &mut Connection, outcome: &TransitionOutcome) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;

    let agg_json = serde_json::to_string(&outcome.endpoint)
        .expect("EndpointAggregate serialization cannot fail");
    tx.execute(
        "INSERT INTO endpoints (id, inventory_signal, aggregate_json) VALUES (?1, ?2, ?3)
         ON CONFLICT(id) DO UPDATE SET aggregate_json = excluded.aggregate_json",
        params![
            outcome.endpoint.id.0.to_string(),
            outcome.endpoint.inventory_signal,
            agg_json
        ],
    )?;

    for event in &outcome.events {
        let payload = serde_json::to_string(event).expect("DomainEvent serialization cannot fail");
        tx.execute(
            "INSERT INTO domain_events (event_id, event_type, endpoint_id, occurred_at, payload_json)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                event.event_id().to_string(),
                event.event_type(),
                event.endpoint_id().0.to_string(),
                event.occurred_at().to_rfc3339(),
                payload
            ],
        )?;
    }

    if let Some(audit) = &outcome.audit {
        let payload = serde_json::to_string(audit).expect("AuditRecord serialization cannot fail");
        tx.execute(
            "INSERT INTO audit_records (audit_id, endpoint_id, occurred_at, payload_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![
                audit.audit_id.to_string(),
                audit.endpoint_id.0.to_string(),
                audit.occurred_at.to_rfc3339(),
                payload
            ],
        )?;
    }

    tx.commit()
}

#[async_trait]
impl EndpointRepository for SqliteEndpointRepository {
    async fn find_by_inventory_signal(
        &self,
        signal: &str,
    ) -> Result<Option<EndpointAggregate>, RepositoryError> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT aggregate_json FROM endpoints WHERE inventory_signal = ?1")
            .map_err(to_backend_err)?;
        let mut rows = stmt.query(params![signal]).map_err(to_backend_err)?;
        match rows.next().map_err(to_backend_err)? {
            Some(row) => {
                let json: String = row.get(0).map_err(to_backend_err)?;
                Ok(Some(Self::row_to_aggregate(json)?))
            }
            None => Ok(None),
        }
    }

    async fn find_by_id(
        &self,
        id: EndpointId,
    ) -> Result<Option<EndpointAggregate>, RepositoryError> {
        let conn = self.conn.lock().await;
        let mut stmt = conn
            .prepare("SELECT aggregate_json FROM endpoints WHERE id = ?1")
            .map_err(to_backend_err)?;
        let mut rows = stmt
            .query(params![id.0.to_string()])
            .map_err(to_backend_err)?;
        match rows.next().map_err(to_backend_err)? {
            Some(row) => {
                let json: String = row.get(0).map_err(to_backend_err)?;
                Ok(Some(Self::row_to_aggregate(json)?))
            }
            None => Ok(None),
        }
    }

    async fn commit(&self, outcome: TransitionOutcome) -> Result<(), RepositoryError> {
        let mut conn = self.conn.lock().await;
        commit_transaction(&mut conn, &outcome).map_err(to_backend_err)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bamep_domain::{transitions, Actor, CredentialSecret, DEFAULT_CREDENTIAL_TTL};
    use chrono::Utc;

    #[tokio::test]
    async fn commit_persists_endpoint_event_and_audit_atomically() {
        let repo = SqliteEndpointRepository::open_in_memory().unwrap();
        let now = Utc::now();

        let transitions::RedeemOutcome::Established { outcome, .. } = transitions::first_contact(
            "mac:AA:BB".into(),
            CredentialSecret("e1".into()),
            now,
            DEFAULT_CREDENTIAL_TTL,
        ) else {
            panic!()
        };
        let endpoint_id = outcome.endpoint.id;
        repo.commit(outcome).await.unwrap();

        let fetched = repo.find_by_id(endpoint_id).await.unwrap().unwrap();
        assert_eq!(fetched.inventory_signal, "mac:AA:BB");

        let conn = repo.conn.lock().await;
        let event_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM domain_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(event_count, 1);
        drop(conn);

        let approve_outcome = transitions::approve_enrollment(
            &fetched,
            Actor::Operator {
                label: "wp1-harness".into(),
            },
            now,
        )
        .unwrap();
        repo.commit(approve_outcome).await.unwrap();

        let conn = repo.conn.lock().await;
        let event_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM domain_events", [], |r| r.get(0))
            .unwrap();
        assert_eq!(event_count, 3, "2 more events from approval");
        let audit_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM audit_records", [], |r| r.get(0))
            .unwrap();
        assert_eq!(audit_count, 1);
    }

    #[tokio::test]
    async fn find_by_inventory_signal_matches_known_endpoint_on_reconnect() {
        let repo = SqliteEndpointRepository::open_in_memory().unwrap();
        let now = Utc::now();

        let transitions::RedeemOutcome::Established { outcome, .. } = transitions::first_contact(
            "mac:CC:DD".into(),
            CredentialSecret("e1".into()),
            now,
            DEFAULT_CREDENTIAL_TTL,
        ) else {
            panic!()
        };
        repo.commit(outcome).await.unwrap();

        let found = repo.find_by_inventory_signal("mac:CC:DD").await.unwrap();
        assert!(found.is_some());

        let missing = repo.find_by_inventory_signal("mac:00:00").await.unwrap();
        assert!(missing.is_none());
    }
}
