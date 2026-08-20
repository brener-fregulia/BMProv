-- Converts the WP1 closed, low-cardinality vocabularies (endpoint identity
-- state, domain event type, audit actor kind) from repeated free-form TEXT
-- to native PostgreSQL ENUM types, per docs/development/persistence.md
-- "Closed categorical values". Existing textual labels are preserved
-- exactly; only the columns' underlying representation changes. 0001 is
-- immutable and is not edited by this migration.

CREATE TYPE endpoint_identity_state AS ENUM (
    'PendingEnrollment',
    'Enrolled',
    'Retired'
);

CREATE TYPE domain_event_type AS ENUM (
    'EndpointPendingEnrollment',
    'EndpointEnrolled',
    'OperatorDecisionRecorded'
);

CREATE TYPE audit_actor_kind AS ENUM (
    'operator',
    'system'
);

-- endpoints.identity_state: the closed-vocabulary CHECK (0001's unnamed
-- column constraint, named by PostgreSQL's default convention) becomes
-- redundant once the column itself is typed as endpoint_identity_state —
-- the enum's own type system enforces the closed set.
ALTER TABLE endpoints
    DROP CONSTRAINT endpoints_identity_state_check;

ALTER TABLE endpoints
    ALTER COLUMN identity_state TYPE endpoint_identity_state
    USING identity_state::endpoint_identity_state;

-- domain_events.event_type had no CHECK constraint in 0001; the new
-- domain_event_type ENUM now enforces the closed vocabulary the Domain
-- already implements (bamep_domain::DomainEvent::event_type).
ALTER TABLE domain_events
    ALTER COLUMN event_type TYPE domain_event_type
    USING event_type::domain_event_type;

-- audit_records.actor_kind: drop both the closed-vocabulary CHECK (now
-- redundant, same reasoning as identity_state above) and the
-- actor_kind/actor_label pairing CHECK. The pairing CHECK is a business
-- invariant, not a vocabulary check, so it is re-added below rather than
-- discarded.
ALTER TABLE audit_records
    DROP CONSTRAINT audit_records_actor_kind_check,
    DROP CONSTRAINT audit_records_check;

ALTER TABLE audit_records
    ALTER COLUMN actor_kind TYPE audit_actor_kind
    USING actor_kind::audit_actor_kind;

ALTER TABLE audit_records
    ADD CONSTRAINT audit_records_actor_label_check CHECK (
        (actor_kind = 'operator' AND actor_label IS NOT NULL)
        OR (actor_kind = 'system' AND actor_label IS NULL)
    );
