-- Durable CurrentBoot foundation for WP1 trusted bootstrap
-- (docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md
-- "Trusted-bootstrap semantic model", "Authoritative current boot and
-- durable Server state"; docs/decisions/0014-agent-credential-lookup-and-
-- boot-context-correlation.md "Amendment (owner-approved, WP1 trusted-
-- bootstrap checkpoint)"): boot_contexts.boot_nonce, the
-- trusted_bootstrap_state native enum, and endpoints' nullable
-- current_boot_context_id/current_boot_nonce/trusted_bootstrap_state
-- projection (CurrentBoot). This migration adds columns/constraints only --
-- no evidence processing or Adapter routing behavior is implemented here.
-- 0001-0003 are immutable and are not edited by this migration.
--
-- Historical rows predate this contract and cannot be assigned a truthful
-- nonce or current-boot state: boot_contexts.boot_nonce stays NULL for
-- existing rows (no fabricated/backfilled nonce), and every existing
-- endpoints row receives NULL/NULL/NULL for its current-boot projection,
-- which the Adapter maps to EndpointAggregate.current_boot = None -- fail
-- closed, never inferred trust. No backfill, no dual security model, no rule
-- that treats an old resolved BootContext as current.

CREATE TYPE trusted_bootstrap_state AS ENUM (
    'NotEstablished',
    'Established'
);

-- boot_nonce: nullable only for historical pre-migration rows. Every
-- BootContext inserted by BootOrchestrationService from this checkpoint
-- onward provides a non-NULL exact 32-byte nonce
-- (bamep_trusted_bootstrap::BootNonce) -- never fabricated or derived here.
ALTER TABLE boot_contexts
    ADD COLUMN boot_nonce BYTEA
        CHECK (boot_nonce IS NULL OR octet_length(boot_nonce) = 32);

-- Relational correlation invariant: an Endpoint's current-boot
-- (boot_context_id, boot_nonce) pair must reference the SAME durable
-- BootContext row and nonce, never an independently-chosen pair. This
-- composite UNIQUE backs the composite FOREIGN KEY added to endpoints below.
-- boot_context_id already carries its own PRIMARY KEY uniqueness; this
-- constraint additionally makes the (id, nonce) pair itself referenceable.
ALTER TABLE boot_contexts
    ADD CONSTRAINT boot_contexts_id_nonce_unique UNIQUE (boot_context_id, boot_nonce);

-- Nullable all-or-none current-boot projection (Domain CurrentBoot). All
-- three NULL means EndpointAggregate.current_boot = None (legacy/unknown --
-- fail closed); a complete non-NULL triple is the authoritative current
-- boot.
ALTER TABLE endpoints
    ADD COLUMN current_boot_context_id BYTEA
        CHECK (current_boot_context_id IS NULL OR octet_length(current_boot_context_id) = 16),
    ADD COLUMN current_boot_nonce BYTEA
        CHECK (current_boot_nonce IS NULL OR octet_length(current_boot_nonce) = 32),
    ADD COLUMN trusted_bootstrap_state trusted_bootstrap_state;

ALTER TABLE endpoints
    ADD CONSTRAINT endpoints_current_boot_all_or_none CHECK (
        (current_boot_context_id IS NULL AND current_boot_nonce IS NULL AND trusted_bootstrap_state IS NULL)
        OR (current_boot_context_id IS NOT NULL AND current_boot_nonce IS NOT NULL AND trusted_bootstrap_state IS NOT NULL)
    );

-- Structurally ties the Endpoint's current-boot pair to the exact durable
-- BootContext row/nonce it came from, rather than trusting the Adapter to
-- keep them consistent by convention alone. PostgreSQL's composite FOREIGN
-- KEY uses MATCH SIMPLE by default: the constraint is only evaluated when
-- BOTH referencing columns are non-NULL, which the all-or-none CHECK above
-- already guarantees happens together -- so a legacy/unresolved current-boot
-- projection (NULL/NULL) never spuriously fails this FK, and a populated one
-- can never reference a historical BootContext row whose own boot_nonce is
-- NULL (no NULL value can equal another NULL under FK/unique matching).
ALTER TABLE endpoints
    ADD CONSTRAINT endpoints_current_boot_fk
        FOREIGN KEY (current_boot_context_id, current_boot_nonce)
        REFERENCES boot_contexts (boot_context_id, boot_nonce);
