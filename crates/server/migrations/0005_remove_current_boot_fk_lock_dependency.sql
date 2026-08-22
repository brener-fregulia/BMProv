-- Lock-topology correction for the CurrentBoot foundation added by
-- 0004_current_boot_and_trusted_bootstrap_state.sql (ADR-0014 "Amendment
-- (owner-approved, WP1 trusted-bootstrap checkpoint)";
-- docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md
-- "Authoritative current boot and durable Server state" — "Persistence
-- persistence may not acquire a BootContext lock after acquiring the
-- Endpoint lock").
--
-- PostgreSQL enforces a referencing-side composite FOREIGN KEY through an
-- internal trigger that performs the equivalent of a `SELECT ... FOR KEY
-- SHARE` against the referenced row on every UPDATE that touches the
-- referencing columns. With `endpoints_current_boot_fk` in place, an
-- Endpoint-only UPDATE that already holds the `endpoints` row lock would
-- therefore implicitly reacquire a `boot_contexts` lock -- an
-- Endpoint -> BootContext dependency the accepted lock order explicitly
-- forbids, and the one the forthcoming BootstrapEvidence transition (under
-- Endpoint lock alone) depends on not existing.
--
-- 0004 is immutable and is not edited by this migration.
--
-- Final authority model (unchanged in meaning, only in how it is enforced):
-- BootContext remains the durable historical issuance/redemption record;
-- Endpoint.CurrentBoot remains the authoritative CURRENT-boot projection.
-- Their relationship is established atomically by the accepted first-
-- contact/genuine-reboot transaction, which already writes CurrentBoot from
-- the exact BootContext participating in that same transaction -- the FK
-- was defense-in-depth, not the mechanism that makes the projection correct.
-- No replacement trigger, FK, or runtime BootContext lookup is introduced;
-- doing so would recreate the same lock dependency this migration removes.

ALTER TABLE endpoints
    DROP CONSTRAINT endpoints_current_boot_fk;

-- Redundant now that no FK references the (boot_context_id, boot_nonce)
-- pair: boot_context_id alone remains the boot_contexts PRIMARY KEY.
ALTER TABLE boot_contexts
    DROP CONSTRAINT boot_contexts_id_nonce_unique;
