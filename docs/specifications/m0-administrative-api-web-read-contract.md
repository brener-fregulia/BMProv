# M0 — Minimum Administrative API and Web Read Contract

Status: **Proposed - awaiting owner approval**

## Context

This Specification defines the minimum versioned Server ↔ Web Administrative API read contract required by the first post-M0 simulated vertical slice, executing Issue #12 (`[WP] Define minimum Administrative API and Web read contract`). It materializes the M0 architecture-planning gap identified and owner-approved during Issue #7 review (`docs/specifications/m0-simulator-contract-and-validation-strategy.md`, "M0 architecture-planning gap: Administrative API / Web contract").

The vertical slice (`docs/specifications/m0-architecture-baseline.md` "First implementation slice after M0") ends with:

```text
... → job reaches terminal state → Web reflects result
```

This Specification defines only the contract needed for that final observation step. It does not implement Server or Web, and does not design a complete Administrative API for all future Bamep functionality.

## Goal

Define the minimum read/query surface, resource representations, versioning boundary, and correlation identifiers Web needs to observe Endpoint, Job, progress/reconciliation, and terminal result state produced by the first vertical slice — without Web reading the Server's internal database, and without inventing parallel Web-specific lifecycle vocabulary alongside the Domain states already defined by the approved M0 Specifications.

## Scope

- the minimum Administrative API v1 read/query surface for the first vertical slice;
- resource representations for Endpoint, Job, JobStep, Attempt, and transfer/Artifact summary state;
- versioning boundary and wire-format conventions for this contract;
- stable identifiers/correlation exposed across Server and Web;
- representation of absent, pending, failed, terminal, and reconciliation states;
- the delivery model this minimum contract requires (query/poll, not push);
- contract-test expectations for the Server ↔ Web boundary.

## Out of scope

Per Issue #12's approved scope, this Specification does **not** define or decide:

- implementation code;
- frontend components/layout/design;
- a complete Administrative API for all future Bamep functionality;
- administrative login/authentication mechanism;
- multi-user support;
- RBAC/permissions;
- user/account lifecycle;
- destructive commands initiated from Web;
- workflow creation/editing UI;
- a final browser real-time event-delivery mechanism (WebSocket/SSE) — this minimum read contract does not demonstrate a requirement for one (see "Delivery model" below);
- Agent Protocol changes;
- data-plane transfer mechanics;
- persistence/database schema;
- external ERP/integration API;
- public third-party API guarantees beyond the Server ↔ Bamep Web boundary addressed here.

Authentication and command/write semantics are surfaced as unresolved findings below, per Issue #12's instruction, rather than silently decided.

## Architectural constraints

Restates Issue #12's approved constraints, consistent with `docs/specifications/m0-stack-and-boundaries-baseline.md` treating Web Administration and the Administrative API as an independently deployable Presentation-layer boundary:

- Web consumes an explicit versioned contract; it never reads the Server's internal database directly;
- persistence schema is not an API, and Rust/Domain/internal types are not the external API contract;
- internal module boundaries must not leak accidentally into the API;
- the API representation may project/summarize Domain state for presentation, but must not redefine authoritative lifecycle semantics — this Specification reuses the Domain state vocabulary already `Accepted` in ADR-0004/`m0-endpoint-identity-lifecycle.md`, ADR-0006/`m0-job-lifecycle-and-scheduling.md`, and ADR-0008/`m0-data-plane-and-storage-contracts.md` directly, rather than inventing parallel Web-specific state names;
- this Specification defines the smallest contract sufficient for the first vertical slice, not a future-complete management API.

## Delivery model

This minimum contract is **request/response, query-oriented**: Web requests current state; the Server does not push updates to Web. Web observes progress and terminal outcome by issuing queries (polling), not by subscribing to a push channel.

This directly satisfies Issue #12's exclusion of "a final browser event-delivery mechanism unless the minimum slice contract genuinely requires an architectural decision" and "WebSocket/SSE simply for real-time convenience without a demonstrated requirement": the first vertical slice's final step ("Web reflects result") does not, by itself, demonstrate a requirement for push delivery — a query issued after the Job reaches a terminal state is sufficient to satisfy it. This is a scoping decision for this minimum contract only; it does not decide the eventual production Administrative API's real-time delivery mechanism, which `docs/discovery/architecture-redesign.md` ("Control plane") already leaves open among REST + polling, REST + long polling, WebSocket, and SSE — that broader decision remains for a future Work Package if a concrete requirement emerges.

This contract is also **snapshot-oriented, not event-stream-oriented**: Web queries current durable domain state; it does not consume the domain-event stream defined by `docs/specifications/m0-persistence-observability-and-domain-events.md` directly. That domain-event envelope remains available for other future integrations (e.g., a future ERP, per `docs/discovery/architecture-redesign.md` "Open-source and commercial boundary"); this Specification does not redefine or narrow it, and does not require Web to be one of its consumers.

## Wire format and versioning baseline

Administrative API v1 reuses, rather than reinvents, the cross-language conventions already `Accepted` for Agent Protocol v1 (`docs/specifications/m0-agent-protocol-contract.md` "Wire encoding"), for consistency across Bamep's contracts:

- UTF-8 JSON request/response bodies;
- timestamps as RFC 3339 / ISO 8601 UTC strings, never epoch integers;
- identifiers (`endpoint_id`, `job_id`, `jobstep_id`, `attempt_id`, `action_id`, `transfer_id`) as UUID v4, lowercase hyphenated string — the same identifiers already defined by `docs/specifications/m0-persistence-observability-and-domain-events.md` "Correlation model", not a new identifier scheme;
- an absent optional field is omitted from the JSON object entirely, never sent as `null`;
- a client (Web) ignores fields it does not recognize within an otherwise known response shape, to allow forward-compatible minor additions.

The contract is versioned as **Administrative API v1**, independently of Agent Protocol v1 and of Server/Web SemVer (`docs/specifications/m0-stack-and-boundaries-baseline.md` "Packaging and versioning baseline": "contracts versioned separately"). Exact transport-level routing (URL paths, HTTP methods, status-code conventions) is implementation-time detail, not decided here — this Specification defines the resources, their representations, and required semantics; not the literal request routing.

## Read/query surface

The minimum resources Web must be able to query, each scoped by its already-defined Domain identifier:

### Endpoint

- `endpoint_id`;
- identity-lifecycle state: `PendingEnrollment` | `Enrolled` | `Retired` (`m0-endpoint-identity-lifecycle.md` "Endpoint identity lifecycle");
- credential/session state, summarized for operator display: whether a session is currently active (derived from `CredentialActive` vs. `NoActiveCredential`/`CredentialExpired`/`CredentialRevoked`); exposing the full credential dimension is not required beyond what an operator needs to understand current connectivity;
- hardware-confidence state: `Consistent` | `LoweredConfidence` | `Conflict` (`m0-endpoint-identity-lifecycle.md` "Hardware/identity-confidence state") — required so Web can surface a `LoweredConfidence`/`Conflict` condition for operator review, consistent with that Specification's own requirement that these conditions "surface for operator awareness and review";
- current inventory revision reference and an inventory summary sufficient for the operator-facing result; the exact inventory summary field set is not decided here — it depends on the inventory content model, which is implementation-time detail not owned by this Specification.

### Job

- `job_id`, `endpoint_id`;
- lifecycle state: `Pending` | `Running` | `Cancelling` | `Succeeded` | `Failed` | `Cancelled` (`m0-job-lifecycle-and-scheduling.md` "Job lifecycle");
- its ordered JobStep summaries (see below);
- terminal outcome and, when `Failed`, the operator-relevant failure information carried by the failing JobStep's `failure_reason`.

### JobStep

- `jobstep_id`, `job_id`;
- lifecycle state: `Pending` | `PreconditionsSatisfied` | `Dispatching` | `Succeeded` | `Failed` | `Cancelled` (`m0-job-lifecycle-and-scheduling.md` "JobStep lifecycle");
- `failure_reason` when `Failed`: `PreconditionNotMet` | `DispatchRejected` | `ExecutionFailed` | `ReconciliationIndeterminate`;
- its current or most recent Attempt summary (see below), when one exists.

### Attempt

- `attempt_id`, `jobstep_id`, `action_id?` (present when the Attempt is Agent-executed, per `m0-persistence-observability-and-domain-events.md` "Correlation model" — `attempt_id` and `action_id` remain distinct fields, not merged);
- lifecycle state: `Dispatched` | `InProgress` | `AwaitingReconciliation` | `Succeeded` | `Failed` | `Cancelled` | `Rejected` | `Indeterminate` (`m0-job-lifecycle-and-scheduling.md` "Attempt lifecycle") — exposed directly and honestly, including `AwaitingReconciliation` and `Indeterminate`, so Web can represent genuine uncertainty to the operator rather than approximating it as success or failure;
- progress snapshot, when the Attempt is `InProgress`: `percent?`, `bytes_processed?`, `eta?`, projected from the Agent Protocol's `ActionProgress` (`m0-agent-protocol-contract.md`) — transient/latest-value-only per `m0-persistence-observability-and-domain-events.md`'s durable/transient boundary; absence of a progress snapshot (e.g., no `ActionProgress` received yet) is not an error and must be represented as an omitted field, not a default value like `0`.

### Transfer / Artifact summary (for a JobStep involving data-plane transfer)

- Artifact lifecycle state: `Incomplete` | `PendingVerification` | `Verified` | `Failed` (`m0-data-plane-and-storage-contracts.md` "Artifact lifecycle");
- `capture_consistency`: `NotApplicable` | `NotEstablished` | `Established`, when the Artifact type requires it (`m0-data-plane-and-storage-contracts.md` "Capture/source-consistency fact");
- `transfer_id` for correlation.

Full chunk-manifest detail (per-chunk digests, chunk indices) is not part of this minimum read contract — Artifact-level summary state is sufficient for the first vertical slice's operator-facing result. Exposing chunk-level detail, if ever needed, is a future extension of this contract, not decided here.

## Representation of absent, pending, failed, terminal, and reconciliation states

- **Absent** (no record yet, e.g. an Endpoint that has never connected) — represented by the resource simply not existing for that identifier, not by a special "empty" state value.
- **Pending** — the Domain's own `Pending` states (Job, JobStep) are reused directly; Web does not need a separate "not started" concept.
- **Failed** — reuses the Domain's own `Failed`/`Rejected` states and `failure_reason` vocabulary; never collapsed into a single generic "error" flag, so an operator can distinguish e.g. `PreconditionNotMet` from `ExecutionFailed`.
- **Terminal** — a Job's terminal states (`Succeeded`/`Failed`/`Cancelled`) are exposed as-is; this Specification does not introduce a separate "done" boolean alongside them.
- **Reconciliation** — `AwaitingReconciliation` and `Indeterminate` are exposed as their own distinct Attempt states, never silently mapped to `InProgress` or `Failed`; an `Indeterminate` Attempt must never be presented to an operator as if its outcome were known (`m0-job-lifecycle-and-scheduling.md`: "`Indeterminate` must never be interpreted as `Succeeded`, `Failed`, or 'not executed'").

## Unresolved findings surfaced for owner review

Per Issue #12's instruction, the following are surfaced as explicit findings rather than silently decided or silently expanded into this Work Package's scope:

1. **Authentication.** This Specification defines a read contract with no access-control mechanism. Even a read-only API exposing Endpoint/Job/inventory state has a genuine access-control question before any real deployment — this Specification does not answer it, consistent with Issue #12's exclusion of "administrative login/authentication mechanism." It remains owner-relevant and is not solved by any current M0 Work Package (also noted as unresolved in `m0-persistence-observability-and-domain-events.md` "Open questions" for audit-record attribution).
2. **Command/write semantics.** Nothing in this Specification allows Web to initiate a Job, cancel a Job, approve enrollment, or issue any other write. If the first vertical slice's actual implementation finds it cannot demonstrate "Web reflects result" through reads alone (for example, if triggering the simulated scenario must itself go through this contract), that would require a new, separately authorized decision — it is not assumed or designed here.

Neither finding blocks approval of this Specification: both are genuinely out of Issue #12's scope, not gaps in this contract's own read-only completeness.

## Acceptance criteria

- An owner-approved Specification defines the minimum Administrative API / Web read contract for the first vertical slice — this document.
- Web can conceptually observe Endpoint, Job, progress/reconciliation, and terminal result state without database coupling — satisfied by "Read/query surface" and "Representation of absent, pending, failed, terminal, and reconciliation states."
- The contract has an explicit versioning boundary — "Administrative API v1" (see "Wire format and versioning baseline").
- Externally exposed identifiers and state representations are defined sufficiently for independent Server/Web implementation — satisfied by reusing the already-defined Domain vocabulary and the Agent Protocol v1 wire conventions.
- Contract-test expectations are defined (see "Validation expectations").
- No authentication, multi-user, command/write, or broad future API design has been silently introduced — see "Unresolved findings surfaced for owner review."
- No architectural decision required by "Web reflects result" remains hidden in the future implementation Work Package — the delivery-model scoping decision is made explicit above, not assumed.

## Validation expectations

Automated: none produced directly by this Work Package — it is decision/specification work, consistent with Issue #12's own stated validation.

Expected contract-test coverage once implemented, per `docs/development/testing.md` "Contract tests":

- serialization of every resource representation defined above, including the wire-format conventions (timestamp format, identifier format, omitted-vs-null optional fields);
- correct representation of each Domain state value listed above, with no state silently collapsed or substituted (in particular `AwaitingReconciliation`, `Indeterminate`, and each `failure_reason` value);
- absence of a progress snapshot represented as an omitted field, not a default value;
- a nonexistent resource identifier represented as absent, not as an empty/default-valued resource;
- version-mismatch handling for an incompatible Administrative API version request (exact behavior implementation-time, but the expectation that a mismatch is detected explicitly, not silently ignored, is part of this contract).

Manual: owner approval of this Specification.

## Related ADRs

No new ADR is introduced by this Work Package, per Issue #12's explicit instruction not to create one merely because the API is versioned. This Specification reuses the already-`Accepted` wire-format conventions from ADR-0005/`m0-agent-protocol-contract.md` and the already-`Accepted` Domain state models from ADR-0004, ADR-0006, and ADR-0008, without introducing a new durable architectural boundary with meaningful alternatives that would require separate ADR treatment.

## Related work

- Issue #12 — `[WP] Define minimum Administrative API and Web read contract`.
- Issue #7 / `m0-simulator-contract-and-validation-strategy.md` — origin of this Work Package (the identified architecture-planning gap).
- Issue #2 / ADR-0004 / `m0-endpoint-identity-lifecycle.md` — Endpoint state vocabulary reused here.
- Issue #3 / ADR-0005 / `m0-agent-protocol-contract.md` — wire-format conventions reused here.
- Issue #4 / ADR-0006 / `m0-job-lifecycle-and-scheduling.md` — Job/JobStep/Attempt state vocabulary reused here.
- Issue #5 / ADR-0007 / `m0-persistence-observability-and-domain-events.md` — correlation model and durable/transient boundary reused here.
- Issue #6 / ADR-0008 / `m0-data-plane-and-storage-contracts.md` — Artifact state vocabulary reused here.
- Issue #1 / `m0-stack-and-boundaries-baseline.md` — Presentation-layer component boundary and independent contract-versioning baseline.

## Open questions

None of the following are blocking for owner approval of Issue #12 — each is explicitly deferred implementation-time detail or a separately surfaced finding, not an unresolved architectural fork of this Specification's own scope.

1. Exact inventory-summary field set exposed to Web — depends on the inventory content model, implementation-time.
2. Exact transport-level routing (URL paths, HTTP methods, status-code conventions) — implementation-time.
3. Authentication mechanism — surfaced as an unresolved finding above, not decided here, not owned by any current M0 Work Package.
4. Whether command/write semantics will eventually be required for the vertical slice — surfaced as an unresolved finding above; not assumed.
5. The eventual production Administrative API's real-time delivery mechanism (if any) — `docs/discovery/architecture-redesign.md` leaves this open among several candidates; this Specification's poll/query-only scoping applies to this minimum contract only.

Status: Proposed - awaiting owner approval.
