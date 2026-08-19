-- WP1 (Issue #17) relational-first schema for Endpoint identity lifecycle and
-- ADR-0012 runtime-credential chain, per ADR-0013 "Modeling: relational-first,
-- JSONB selective". Only the identity/credential/event/audit surface WP1
-- actually implements is modeled here; WP2-WP5 tables are intentionally out
-- of scope for this migration.

-- Durable Endpoint identity (m0-endpoint-identity-lifecycle.md "Endpoint
-- identity lifecycle"). `inventory_signal` is the WP1 stand-in for the real
-- MAC/disk-fingerprint matching algorithm and is the reconnect-matching key.
CREATE TABLE endpoints (
    id UUID PRIMARY KEY,
    inventory_signal TEXT NOT NULL UNIQUE,
    identity_state TEXT NOT NULL
        CHECK (identity_state IN ('PendingEnrollment', 'Enrolled', 'Retired')),
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

-- ADR-0012 runtime-credential chain: at most one predecessor in grace, plus
-- at most one current unconfirmed successor (ADR-0012 point 4, "Bounded
-- valid set"). One row per Endpoint, always written together with its
-- `endpoints` row in the same transaction. `*_verifier` is the opaque,
-- never-reversible hash representation (ADR-0012 point 10) — an "opaque
-- credential/assertion blob" in ADR-0013 Sec.6 terms; every other column is a
-- real, queryable lifecycle/timestamp field.
CREATE TABLE endpoint_credentials (
    endpoint_id UUID PRIMARY KEY REFERENCES endpoints (id),
    predecessor_verifier BYTEA NOT NULL,
    predecessor_issued_at TIMESTAMPTZ NOT NULL,
    predecessor_expires_at TIMESTAMPTZ NOT NULL,
    successor_verifier BYTEA,
    successor_issued_at TIMESTAMPTZ,
    successor_expires_at TIMESTAMPTZ,
    revoked BOOLEAN NOT NULL DEFAULT FALSE,
    CHECK (
        (successor_verifier IS NULL AND successor_issued_at IS NULL AND successor_expires_at IS NULL)
        OR (successor_verifier IS NOT NULL AND successor_issued_at IS NOT NULL AND successor_expires_at IS NOT NULL)
    )
);

-- Domain events (m0-persistence-observability-and-domain-events.md
-- "Domain-event model"). Envelope fields are real columns so they remain
-- queryable/indexable; `payload` is JSONB reserved for the genuinely
-- type-specific remainder only (ADR-0013 Sec.6) — never lifecycle/state that
-- must be queryable.
CREATE TABLE domain_events (
    event_id UUID PRIMARY KEY,
    event_type TEXT NOT NULL,
    event_version INTEGER NOT NULL DEFAULT 1,
    endpoint_id UUID NOT NULL REFERENCES endpoints (id),
    occurred_at TIMESTAMPTZ NOT NULL,
    payload JSONB NOT NULL
);

CREATE INDEX idx_domain_events_endpoint_id ON domain_events (endpoint_id);

-- Safety-relevant audit records (m0-persistence-observability-and-domain-events.md
-- "Auditability"), durable and immutable once written. Actor attribution is
-- relational (small, fixed shape), not JSONB.
CREATE TABLE audit_records (
    audit_id UUID PRIMARY KEY,
    endpoint_id UUID NOT NULL REFERENCES endpoints (id),
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('operator', 'system')),
    actor_label TEXT,
    occurred_at TIMESTAMPTZ NOT NULL,
    detail TEXT NOT NULL,
    CHECK (
        (actor_kind = 'operator' AND actor_label IS NOT NULL)
        OR (actor_kind = 'system' AND actor_label IS NULL)
    )
);

CREATE INDEX idx_audit_records_endpoint_id ON audit_records (endpoint_id);
