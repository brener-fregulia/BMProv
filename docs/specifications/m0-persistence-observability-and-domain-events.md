# M0 — Persistence, Observability, and Domain-Event Model

Status: **Proposed - awaiting owner approval**

## Context

This Specification details the durable/transient persistence boundary accepted in ADR-0007, the domain-event catalog, the observability correlation model, and inventory/auditability boundaries required by M0, executing Issue #5 (`[WP] Define persistence, observability, and domain-event model`).

## Durable vs. transient/high-frequency data

Restates and applies ADR-0007's boundary (see that ADR for full reasoning):

- **Durable** (written to the SQLite-backed domain database, on state *transition*, not on observation): Job/JobStep/Attempt state transitions; Endpoint identity/credential/hardware-confidence state transitions; inventory on revision change only; Artifact/Snapshot metadata on lifecycle transition; domain events; audit records for safety-relevant operator decisions.
- **Transient/high-frequency** (not one durable write per message/sample): Agent connection/presence state; `ActionProgress` ticks (latest-value only); general logs; high-frequency telemetry/metrics.

Any future Work Package or implementation that persists new state must classify it against this boundary explicitly rather than defaulting to "durable."

## Domain-event model

Domain events are durable, coarse-grained records of a completed state transition, useful to the product itself and to future integrations (`docs/discovery/architecture-redesign.md` "Observability"; the open-source/commercial boundary in that document requires future ERP integration through domain events, never through Bamep's internal database directly).

**Illustrative M0 event catalog** (not exhaustive — extensible as later Work Packages and implementation introduce further transitions; each event carries the correlation fields defined below):

| Event | Emitted when |
|---|---|
| `EndpointPendingEnrollment` | an Endpoint identity record enters `PendingEnrollment` (`m0-endpoint-identity-lifecycle.md`) |
| `EndpointEnrolled` | an Endpoint identity record enters `Enrolled` |
| `EndpointHardwareConfidenceChanged` | the hardware-confidence dimension changes (`Consistent`/`LoweredConfidence`/`Conflict`) |
| `EndpointRetired` | an Endpoint identity record enters `Retired` |
| `InventoryRevisionRecorded` | a new durable inventory revision is written (on change only, never per poll) |
| `JobStarted` | a Job transitions `Pending` → `Running` |
| `JobSucceeded` / `JobFailed` / `JobCancelled` | a Job reaches the corresponding terminal state (`m0-job-lifecycle-and-scheduling.md`) |
| `JobStepFailed` | a JobStep reaches `Failed`, with its `failure_reason` |
| `AttemptIndeterminate` | an Attempt is closed `Indeterminate` — a natural operator-notification source, since destructive JobSteps require an explicit operator decision afterward |
| `ArtifactCreated` / `ArtifactVerified` | an artifact/snapshot completes its creation or verification lifecycle stage (final shape owned by Issue #6) |
| `OperatorDecisionRecorded` | an operator approval/decision is durably recorded (enrollment approval, reconciliation/`Indeterminate` resolution, destructive retry authorization, cancellation) |

Each event is emitted once per underlying transition — never re-derived from, or duplicating, high-frequency observation data.

## Correlation model

Every durable record and domain event carries whichever of the following identifiers are applicable to it, so that endpoint, job, step, attempt, action, and (in a future Work Package) transfer can be related:

- `endpoint_id`
- `job_id`
- `jobstep_id`
- `attempt_id` (equals the Agent Protocol `action_id`, `docs/specifications/m0-agent-protocol-contract.md`)
- `transfer_id` — reserved for Issue #6; not defined by this Specification

This is the minimum correlation set required by `docs/discovery/architecture-redesign.md` "Observability" ("Correlation must make it possible to relate endpoint, job, step, attempt, action, and transfer"). Additional correlation fields may be added by the Work Package that introduces the relevant concept (e.g., Issue #6 for `transfer_id`'s full meaning), without requiring this Specification to be revised for every addition, as long as they compose with this set rather than replace it.

## Inventory persistence boundary

- A durable inventory revision is written only when observed inventory differs from the Endpoint's last durable revision — not on every report/poll cycle. This is the concrete, testable expression of ADR-0007's durable-write-model requirement for inventory specifically.
- The current inventory revision identifier is what `docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md` / `docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md` reference as the destructive-operation precondition "sufficiently fresh inventory" — this Specification is the durable source of that identifier.
- Historical inventory revisions are retained as an append/revision chain sufficient for audit and precondition checks; a specific retention duration or pruning policy is implementation-time detail, not decided here.

## Auditability

Durable audit records exist for every safety-relevant operator decision already required by earlier M0 ADRs:

- Endpoint enrollment approval (`docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md`);
- hardware-confidence conflict resolution (`docs/specifications/m0-endpoint-identity-lifecycle.md`);
- reconciliation decisions closing an Attempt as `Indeterminate`, and any decision authorizing a further Attempt for a destructive JobStep (`docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md`);
- Job cancellation decisions.

An audit record is durable, immutable once written, and carries the correlation fields above plus whichever operator/actor identity is available. This Specification does not define the operator-identity/authentication model itself (an Administrative API concern, not yet a dedicated M0 Work Package) — it only establishes that these decisions must be durably and immutably recorded.

## Observability responsibilities

- Correlation (above) is the primary observability requirement M0 must satisfy structurally.
- High-frequency telemetry (throughput, resource usage) does not need indefinite persistence (`docs/discovery/architecture-redesign.md` "Observability") — if retained at all, retention/aggregation strategy is implementation-time detail.
- Domain events (above) are the durable observability surface useful for operator-facing history and future integrations; high-frequency telemetry is not a substitute for them, and domain events are not a substitute for telemetry's real-time detail.

## Out of scope

- concrete database schema, indexing, or migration strategy — implementation-time;
- operator-identity/authentication model for audit-record attribution — not yet a dedicated M0 Work Package;
- `transfer_id`'s full meaning and artifact-specific event shapes — Issue #6;
- telemetry retention/aggregation policy — implementation-time, not an M0 architectural blocker;
- Job/JobStep/Attempt state-machine semantics themselves — already defined by Issue #4 / ADR-0006; this Specification only defines how that state is persisted and observed.

## Acceptance criteria

- Durable vs. transient/high-frequency boundary is explicit and applies to every M0 domain concept introduced so far (Endpoint identity, Job/JobStep/Attempt) as well as future ones.
- A representative domain-event catalog and correlation model are defined (Issue #5 acceptance criterion).
- Relevant requirements have a defined validation strategy (below).

## Validation expectations

Per `docs/development/testing.md` "Unit and domain tests": domain-event emission tests (one event per underlying transition, never duplicated or omitted); inventory write-on-change tests (an unchanged inventory report produces no new durable revision).

Per `docs/development/testing.md` "Contract tests": domain-event schema/versioning expectations, consistent with how `docs/specifications/m0-agent-protocol-contract.md` versions its own contract.

Per `docs/development/testing.md` "Persistence and recovery tests": durable state survives restart; transient/high-frequency data (presence, progress) does not need to, and its absence after restart must not be misinterpreted as a domain-state loss.

Per `docs/development/testing.md` "Simulator", and per the owner's explicit direction for this Work Package: the Simulator/validation strategy should eventually exercise **representative persistence load at the M0 20–24 concurrent-endpoint target**, measuring actual durable write volume and rate against the model in ADR-0007, so that any future backend change (e.g., to PostgreSQL) is evidence-driven rather than speculative. This is a requirement on Issue #7's scope, not implemented by this Specification.

Per "Local development environments," these are expected to run in the Linux reference environment (WSL2 or containers from Windows).

Manual: owner approval of this Specification.

## Related ADRs

- ADR-0007 — Persistence backend and durable/transient boundary (`Accepted`).
- ADR-0004 — Endpoint identity (durable identity/credential/confidence state this Specification's events cover).
- ADR-0005 — Agent control-plane protocol (`ActionProgress` as the canonical high-frequency, non-durable example; `action_id`/`attempt_id` correlation).
- ADR-0006 — Job/JobStep/Attempt state model (durable state and `Indeterminate` this Specification's events cover).

## Related work

- Issue #5 — `[WP] Define persistence, observability, and domain-event model`.
- Issue #2 / ADR-0004 — Endpoint identity durable state and events.
- Issue #3 / ADR-0005 — `ActionProgress`, correlation identifiers.
- Issue #4 / ADR-0006 — Job/JobStep/Attempt durable state and events.
- Issue #6 — `[WP] Define data-plane and storage contracts` (artifact events, `transfer_id`).
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (representative persistence-load validation).

## Open questions

1. Concrete database schema, indexing, and migration strategy — implementation-time.
2. Operator-identity/authentication model for audit-record attribution — not yet owned by any M0 Work Package.
3. Telemetry retention/aggregation policy, if any — implementation-time.
4. Exact `transfer_id` semantics — Issue #6.

Status: Proposed - awaiting owner approval.
