# M0 — Persistence, Observability, and Domain-Event Model

Status: **Approved**

## Context

This Specification details the durable/transient persistence boundary originally accepted in ADR-0007 and carried forward unchanged by ADR-0013 (`Accepted`; ADR-0007 is `Superseded by ADR-0013` for the backend selection only), the domain-event catalog, the observability correlation model, and inventory/auditability boundaries required by M0, executing Issue #5 (`[WP] Define persistence, observability, and domain-event model`).

## Durable vs. transient/high-frequency data

Restates and applies the durable/transient boundary ADR-0007 established and ADR-0013 carries forward unchanged (see those ADRs for full reasoning):

- **Durable** (written to the PostgreSQL-backed domain database, on state *transition*, not on observation): Job/JobStep/Attempt state transitions; Endpoint identity/credential/hardware-confidence state transitions; inventory on revision change only; Artifact/Snapshot metadata on lifecycle transition; domain events; audit records for safety-relevant operator decisions.
- **Transient/high-frequency** (not one durable write per message/sample): Agent connection/presence state; `ActionProgress` ticks (latest-value only); general logs; high-frequency telemetry/metrics.

Any future Work Package or implementation that persists new state must classify it against this boundary explicitly rather than defaulting to "durable."

**This boundary is an architectural expectation, not yet an empirically validated result** (originally ADR-0007; carried forward unchanged by ADR-0013). It bounds write volume to the number of domain-state transitions rather than message/sample count, but it does not make database load independent of endpoint count — more concurrent endpoints still produce more transitions. Whether the resulting load is comfortable for the adopted persistence backend (PostgreSQL, ADR-0013) at the M0 20–24 endpoint target is measured empirically by the post-M0 first implementation vertical slice, running the persistence-load scenario Issue #7's Specification defines (see "Validation expectations"). This measurement is not itself part of the M0 architecture/contract baseline — no implementation exists during M0 to run it against.

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

### Domain-event envelope

Every durable domain event carries at least:

- `event_id` — unique and immutable once assigned;
- `event_type` — the event name (e.g. `JobSucceeded`);
- `event_version` — versions the schema of that specific `event_type`, independent of other event types' versions;
- `occurred_at` — when the underlying transition committed;
- `endpoint_id?`, `job_id?`, `jobstep_id?`, `attempt_id?`, `action_id?`, `transfer_id?` — correlation fields (see "Correlation model" below), present only where applicable to that event type;
- `payload` — the event-type-specific data.

This Specification defines the envelope only. No external transport, webhook mechanism, message broker, or ERP-facing API is defined here — how a future integration might consume these events is out of scope (see "Transactional consistency and event model" below).

## Correlation model

Every durable record and domain event carries whichever of the following identifiers are applicable to it, so that endpoint, job, step, attempt, action, and transfer can be related:

- `endpoint_id`
- `job_id`
- `jobstep_id`
- `attempt_id` — the Server-side Domain identity for one JobStep execution attempt (`docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md`).
- `action_id` — the Agent Protocol identity for the corresponding dispatched action (`docs/specifications/m0-agent-protocol-contract.md`).
- `transfer_id` — used here for correlation only; its full semantics (the durable identity of one logical data-plane transfer operation, distinct from HTTP connection/request identity and from `attempt_id`) are defined by ADR-0008 point 10 / `docs/specifications/m0-data-plane-and-storage-contracts.md`, not redefined by this Specification

`attempt_id` and `action_id` are **kept distinct** — `Attempt` is a Server-side Domain concept and `action_id` is an Agent Protocol (wire) concept. For an Agent-executed Attempt the relationship is 1:1, but the identifiers are not merged into one field: both are recorded, and durably linked, so Domain identity is never coupled to the wire protocol's identifier scheme. A future non-Agent-executed Attempt type (not defined by this Specification) could in principle have no `action_id` at all.

This is the minimum correlation set required by `docs/discovery/architecture-redesign.md` "Observability" ("Correlation must make it possible to relate endpoint, job, step, attempt, action, and transfer"). `transfer_id` is one such addition, already introduced above and owned by ADR-0008 point 10 / `docs/specifications/m0-data-plane-and-storage-contracts.md`. Additional correlation fields may likewise be added by the Work Package that introduces the relevant concept, without requiring this Specification to be revised for every addition, as long as they compose with this set rather than replace it.

## Transactional consistency and event model

Per ADR-0013 (originally established by ADR-0007): when a durable domain transition requires a domain event and/or an audit record, the domain-state change, its event, and any required audit record are persisted atomically in the same persistence transaction. A crash must never leave committed state without its required event/audit record, or a committed event/audit record for a transition that did not itself commit.

For an Agent-executed Attempt specifically, this transaction commits **before** the Server attempts to transmit `ActionDispatch` to the Agent — a database transaction and a WebSocket send cannot be atomic with each other, so persistence always comes first, an invariant ADR-0013 carries forward directly (point 16). The exact ordering, and how a crash around the send boundary is handled (via the existing `Dispatched` → `AwaitingReconciliation` path, `docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md`), is defined in ADR-0007 "Crash-safe dispatch persistence ordering" (historical origin, preserved unedited; the requirement itself is authoritative through ADR-0013); this Specification does not duplicate that detail.

**This is not event sourcing.** Current durable domain state remains the source of truth. The domain event describing a transition is persisted in the **same atomic transaction** as that transition — it becomes durable and visible only if that transaction commits; it is not a second, post-commit database write, and there is no window where the transition is committed but its event is not (or vice versa). Domain events are not a log Bamep replays to reconstruct state. External publication/delivery semantics for future integrations are outside this Work Package.

## Inventory persistence boundary

- A durable inventory revision is written only when observed inventory differs from the Endpoint's last durable revision — not on every report/poll cycle. This is the concrete, testable expression of ADR-0013's durable-write-model requirement for inventory specifically (originally established by ADR-0007).
- The current inventory revision identifier is what `docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md` / `docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md` reference as the destructive-operation precondition "sufficiently fresh inventory" — this Specification is the durable source of that identifier.
- Historical inventory revisions are retained as an append/revision chain sufficient for audit and precondition checks; a specific retention duration or pruning policy is implementation-time detail, not decided here.

## Auditability

Durable audit records exist for two categories of safety-relevant activity, aligning this Specification with ADR-0013's full auditability requirement (originally established by ADR-0007; covers both operator decisions and destructive-operation dispatch/outcome, not operator decisions alone):

**Operator decisions**, already required by earlier M0 ADRs:

- Endpoint enrollment approval (`docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md`);
- hardware-confidence conflict resolution (`docs/specifications/m0-endpoint-identity-lifecycle.md`);
- reconciliation decisions closing an Attempt as `Indeterminate`, and any decision authorizing a further Attempt for a destructive JobStep (`docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md`);
- Job cancellation decisions.

**Destructive execution**, required by ADR-0013 (originally ADR-0007) and not previously made explicit in this Specification:

- the authorization/decision enabling a destructive dispatch, where applicable (e.g., the operator decision authorizing a retry after `Indeterminate` — the same record as above, linked to the dispatch it authorizes);
- the destructive dispatch **commitment** — the durable record that the Server authorized and durably committed that Attempt for transmission (originally `docs/decisions/0007-persistence-backend-and-durable-transient-boundary.md` "Crash-safe dispatch persistence ordering", carried forward by ADR-0013). This record represents the Server's own committed decision, not confirmation that the `ActionDispatch` frame was actually transmitted or received — a crash can occur after this commit and before transmission is attempted. Actual Agent-side knowledge of the action remains represented by the existing Agent Protocol lifecycle (`ActionAck`, `ActionResult`, `StatusQuery`/`StatusReport`) and the Attempt's reconciliation transitions, not by a second audit record;
- its known terminal outcome (`Succeeded`/`Failed`/`Cancelled`/`Rejected`) or its `Indeterminate` resolution, once established through that Agent Protocol lifecycle.

An audit record is durable, immutable once written, and carries the correlation fields above. Actor attribution distinguishes an **operator actor** (a human decision, when operator identity is available) from a **system actor** (e.g., an automatic non-destructive retry per JobStep retry policy) where the distinction is known. This Specification does not define the operator-identity/authentication model itself (an Administrative API concern, not yet a dedicated M0 Work Package) — it only establishes that these records must be durably and immutably kept, with whichever actor information is available at the point of recording.

Required audit records associated with a domain transition participate in the same atomic persistence transaction as that transition and its domain event (see "Transactional consistency and event model" above) — an audit record is never a best-effort side write.

## Observability responsibilities

- Correlation (above) is the primary observability requirement M0 must satisfy structurally.
- High-frequency telemetry (throughput, resource usage) does not need indefinite persistence (`docs/discovery/architecture-redesign.md` "Observability") — if retained at all, retention/aggregation strategy is implementation-time detail.
- Domain events (above) are the durable observability surface useful for operator-facing history and future integrations; high-frequency telemetry is not a substitute for them, and domain events are not a substitute for telemetry's real-time detail.

## Out of scope

- concrete database schema, indexing, or migration strategy — implementation-time;
- concrete performance thresholds for the adopted persistence backend's write contention/latency/backpressure — owned by Issue #7's validation, not defined here;
- external event transport, webhook mechanism, message broker, or ERP-facing API — not defined by this Specification;
- operator-identity/authentication model for audit-record attribution — not yet a dedicated M0 Work Package;
- artifact-specific event shapes — owned by ADR-0008 point 10 / `docs/specifications/m0-data-plane-and-storage-contracts.md`;
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

Per `docs/development/testing.md` "Simulator", and per ADR-0013 (obligation originally established by ADR-0007): the post-M0 first implementation vertical slice **must** exercise representative persistence load at the M0 20–24 concurrent-endpoint target, measuring actual durable write volume, contention, latency, and backpressure against the expectation carried forward in ADR-0013. If that measurement shows unacceptable results, ADR-0013 must be revisited. This is a requirement on the scenario Issue #7's Specification defines (`docs/specifications/m0-simulator-contract-and-validation-strategy.md` "Persistence-load validation"), not implemented by this Specification, and this Specification does not define the concrete performance thresholds that would make a result "unacceptable" — that determination belongs to the vertical slice that runs the measurement, informed by observed behavior. This obligation is a defined, non-optional part of post-M0 validation, not a precondition for the M0 architecture/contract baseline itself: the baseline is complete when its own acceptance criteria are satisfied (`docs/specifications/m0-architecture-baseline.md`), and the absence of an implementation during M0 is expected, not a contradiction or license to fabricate performance evidence.

Per "Unit and domain tests": atomic-transaction tests demonstrating that a domain-state change, its required domain event, and any required audit record commit or fail together — never partially.

Per "Local development environments," these are expected to run in the Linux reference environment (WSL2 or containers from Windows).

Manual: owner approval of this Specification — confirmed (see Status).

## Related ADRs

- ADR-0013 — PostgreSQL persistence backend baseline (`Accepted`) — the current persistence-backend authority; carries forward the durable/transient boundary, transactional-consistency, and persist-before-send invariants this Specification applies.
- ADR-0007 — Persistence backend and durable/transient boundary (`Superseded by ADR-0013`) — historical record of the original M0 backend evaluation; the backend-independent invariants it established remain authoritative through ADR-0013, not through this now-superseded document.
- ADR-0004 — Endpoint identity (durable identity/credential/confidence state this Specification's events cover).
- ADR-0005 — Agent control-plane protocol (`ActionProgress` as the canonical high-frequency, non-durable example; `action_id` as the distinct-but-linked counterpart to `attempt_id` in the correlation model).
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

Status: Approved.
