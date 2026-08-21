-- ADR-0014 persistence foundation: durable BootContext for unresolved
-- boot-scoped enrollment credentials, and a globally unique Endpoint
-- credential lookup index, per docs/development/persistence.md "Closed
-- categorical values" and ADR-0013 "Modeling: relational-first, JSONB
-- selective". This migration creates schema only -- no credential-routing,
-- promotion, or Adapter behavior is implemented here. 0001 and 0002 are
-- immutable and are not edited by this migration.
--
-- Pre-existing endpoint_credentials rows created by the transitional
-- pre-ADR-0014 flow have no self-locating lookup identifier to backfill
-- from their existing verifier, so endpoint_credential_lookups is
-- deliberately left empty for them here; such rows remain unreachable
-- through the future lookup-based AuthRequest path until re-established by
-- the new flow.

-- Adapter/PostgreSQL persistence vocabulary distinguishing a predecessor
-- lookup mapping from a successor one (bamep_domain::CredentialChain's two
-- slots). Not introduced as a Domain type in this checkpoint.
CREATE TYPE credential_slot_role AS ENUM (
    'predecessor',
    'successor'
);

-- Durable state backing an unresolved boot-scoped enrollment credential
-- (ADR-0014 point 4). `boot_context_id` and `verifier` are the durable byte
-- representations of bamep_domain::presented_credential::CredentialLookupId
-- and bamep_domain::credential::CredentialHash respectively -- never the
-- plaintext enrollment secret. `inventory_signal` is correlation evidence
-- only (ADR-0004; ADR-0014 point 4) -- never authentication, never durable
-- Endpoint identity. Multiple BootContexts may legitimately share the same
-- inventory_signal across separate boot attempts, so it carries no
-- uniqueness constraint. `resolved_endpoint_id` is set once, atomically with
-- the Endpoint credential transition, on first successful redemption (ADR-0014
-- point 5); that atomic commit is a later Port/Adapter checkpoint.
CREATE TABLE boot_contexts (
    boot_context_id BYTEA PRIMARY KEY
        CHECK (octet_length(boot_context_id) = 16),
    verifier BYTEA NOT NULL
        CHECK (octet_length(verifier) = 48),
    issued_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL
        CHECK (expires_at > issued_at),
    inventory_signal TEXT NOT NULL,
    resolved_endpoint_id UUID REFERENCES endpoints (id)
);

-- Direct indexed lookup from a presented credential's non-secret lookup
-- identifier to the Endpoint credential chain slot (predecessor/successor)
-- it belongs to (ADR-0014 point 2/3). PRIMARY KEY (endpoint_id, slot) makes
-- every lifecycle transition an upsert on a fixed two-row-per-endpoint
-- shape; the separate UNIQUE (lookup_id) is the structural guarantee that
-- one lookup identifier can route to at most one Endpoint/slot -- including
-- across predecessor-vs-successor roles and across different Endpoints --
-- rather than relying on random-collision probability alone. Revocation is
-- not modeled here: a revoked chain remains lookup-resolvable and is
-- rejected by CredentialChain.revoked at authentication time, not by
-- removing its lookup mapping.
CREATE TABLE endpoint_credential_lookups (
    endpoint_id UUID NOT NULL REFERENCES endpoints (id),
    slot credential_slot_role NOT NULL,
    lookup_id BYTEA NOT NULL
        CHECK (octet_length(lookup_id) = 16),
    PRIMARY KEY (endpoint_id, slot),
    UNIQUE (lookup_id)
);
