# M0 — Minimum Administrative API and Web Read Contract

Status: **Proposed - awaiting owner approval**

## Context

This Specification defines the minimum versioned Server ↔ Web Administrative API read contract required by the first post-M0 simulated vertical slice, executing Issue #12 (`[WP] Define minimum Administrative API and Web read contract`). It materializes the M0 architecture-planning gap identified and owner-approved during Issue #7 review (`docs/specifications/m0-simulator-contract-and-validation-strategy.md`, "M0 architecture-planning gap: Administrative API / Web contract").

The vertical slice (`docs/specifications/m0-architecture-baseline.md` "First implementation slice after M0") ends with:

```text
... → job reaches terminal state → Web reflects result
```

This Specification defines only the contract needed for that final observation step. It does not implement Server or Web, and does not design a complete Administrative API for all future Bamep functionality.

**Three distinct communication responsibilities.** Bamep has three separate communication boundaries, and this Work Package touches only the third:

1. **Agent Control Plane** — Agent ↔ Server, Agent Protocol v1, WSS; already `Accepted` (ADR-0005, Issue #3).
2. **Data Plane** — Agent ↔ Server, large artifact transfer, HTTP chunk-oriented; already `Accepted` (ADR-0008, Issue #6).
3. **Administrative / Management Plane** — Bamep Web ↔ Bamep Server; this is the boundary Issue #12 defines.

This Specification does not change, reinterpret, or modify the Agent control-plane decision. Administrative API v1 is a separate contract for a separate boundary; it must never be called or treated as "the Agent control plane."

## Goal

Define the minimum read/query surface, resource representations, versioning boundary, and correlation identifiers Web needs to observe Endpoint, Job, progress/reconciliation, and terminal result state produced by the first vertical slice — without Web reading the Server's internal database, and without inventing parallel Web-specific lifecycle vocabulary alongside the Domain states already defined by the approved M0 Specifications.

## Scope

- the minimum Administrative API v1 read/query surface for the first vertical slice;
- resource representations for Endpoint, Job, JobStep, Attempt, and transfer/Artifact summary state;
- versioning boundary and wire-format conventions for this contract;
- stable identifiers/correlation exposed across Server and Web;
- representation of absent, pending, failed, terminal, and reconciliation states;
- the request/response snapshot-read delivery model this contract defines, and its explicit boundary from any future update-notification mechanism;
- the minimum normative HTTP read operations (routing, success/not-found semantics) needed by the first vertical slice;
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
- Job creation, cancellation, or enrollment-approval endpoints, destructive commands, or any other generic write API initiated from Web;
- workflow creation/editing UI;
- choosing a final browser real-time event-delivery/notification mechanism (SSE, WebSocket, long polling, or otherwise) — none is chosen by this Work Package (see "Delivery model" below);
- Agent Protocol changes, and any reinterpretation of the Agent control plane already `Accepted` in ADR-0005;
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

Administrative API v1 defines a **request/response read surface for obtaining the authoritative current snapshot of Server-owned state**. That is the contract: given a resource identifier, a read returns the current authoritative state for it.

**Polling is not part of the architectural contract.** It is a permitted implementation strategy: the first vertical-slice implementation may periodically query these resources to observe change over time. Administrative API v1 itself does not require, assume, or standardize any particular query cadence or change-detection strategy — it only defines what a single request/response read returns. Explicitly:

- Administrative API v1 exposes authoritative current-state snapshots through HTTP request/response reads.
- The first vertical-slice implementation may periodically query those resources.
- No push mechanism is required to satisfy M0.
- The eventual mechanism by which Web learns that state changed remains undecided.
- SSE, WebSocket, long polling, or another notification mechanism may be evaluated later if a concrete product requirement justifies one — **none is chosen by this Work Package**. This is distinct from, and must not be confused with or presented as changing, the Agent Protocol v1 WSS control plane already `Accepted` in ADR-0005 (Issue #3) — that is the separate Agent↔Server control plane (see "Three distinct communication responsibilities" above).

**Snapshot reads remain authoritative even if a push/change-notification mechanism is introduced later.** Conceptually, any future Web client behavior is expected to follow:

Web opens or reconnects → reads current authoritative resource state (Administrative API v1) → may later receive change notifications (mechanism not designed here) → re-reads/reconciles against authoritative Server state as necessary.

Web must never be required to reconstruct authoritative Job/Endpoint state solely by replaying domain events or browser notifications — a future push mechanism, if introduced, supplements this read surface; it does not replace it as the source of truth. This Specification does not design that future push protocol.

This contract is also **snapshot-oriented, not event-stream-oriented**: a read returns current durable domain state; it does not consume the domain-event stream defined by `docs/specifications/m0-persistence-observability-and-domain-events.md` directly. That domain-event envelope remains available for other future integrations (e.g., a future ERP, per `docs/discovery/architecture-redesign.md` "Open-source and commercial boundary"); this Specification does not redefine or narrow it, and does not require Web to be one of its consumers.

## Wire format and versioning baseline

Administrative API v1 reuses, rather than reinvents, the cross-language conventions already `Accepted` for Agent Protocol v1 (`docs/specifications/m0-agent-protocol-contract.md` "Wire encoding"), for consistency across Bamep's contracts:

- UTF-8 JSON request/response bodies;
- timestamps as RFC 3339 / ISO 8601 UTC strings, never epoch integers;
- Domain identifiers exposed to Web (`endpoint_id`, `job_id`, `jobstep_id`, `attempt_id`, `transfer_id`, `artifact_id`, inventory revision identifiers, etc.) are **stable opaque JSON strings** — the same identifiers already defined by `docs/specifications/m0-persistence-observability-and-domain-events.md` "Correlation model", not a new identifier scheme. Web must not infer semantic meaning from their textual format. This Work Package does not choose the generation format for any of them; the authoritative contract/Specification that owns a given identifier may define a more specific format independently of this one. `action_id`, when present, continues to follow Agent Protocol v1's own format (`docs/specifications/m0-agent-protocol-contract.md` "Wire encoding" — UUID v4, lowercase hyphenated string), because that identifier is owned by that contract, not by this one;
- an absent optional field is omitted from the JSON object entirely, never sent as `null`;
- a client (Web) ignores fields it does not recognize within an otherwise known response shape, to allow forward-compatible minor additions.

The contract is versioned as **Administrative API v1**, independently of Agent Protocol v1 and of Server/Web SemVer (`docs/specifications/m0-stack-and-boundaries-baseline.md` "Packaging and versioning baseline": "contracts versioned separately"). The `/api/admin/v1/` routing prefix (see "Minimum HTTP read operations" below) is this contract's explicit version boundary.

## Minimum HTTP read operations

Issue #12 exists specifically to define an independently implementable Server ↔ Web contract, so — unlike broader REST conventions — the minimum routing and success/not-found semantics needed by the first vertical slice are normative, not implementation-time detail:

- `GET /api/admin/v1/endpoints/{endpoint_id}` → the Endpoint representation defined below.
- `GET /api/admin/v1/jobs/{job_id}` → the Job representation defined below, including its ordered JobStep summaries, the relevant Attempt summary, progress snapshot, and Transfer/Artifact summary needed by the vertical slice. Separate JobStep/Attempt endpoints are not defined — no concrete need to query them independently of their parent Job has been identified for this minimum slice.
- Existing resource → HTTP `200` with the documented JSON representation.
- Nonexistent resource identifier → HTTP `404`.
- Response bodies are JSON, per the wire-format conventions above.

This Specification does not design list endpoints, pagination, filtering, caching/ETag behavior, a broad error catalog, CRUD semantics, or future-complete REST conventions — those remain out of scope for this minimum contract and are added only when a concrete need is identified.

## Read/query surface

The minimum resources Web must be able to query, each scoped by its already-defined Domain identifier:

### Endpoint

- `endpoint_id`;
- identity-lifecycle state: `PendingEnrollment` | `Enrolled` | `Retired` (`m0-endpoint-identity-lifecycle.md` "Endpoint identity lifecycle");
- credential/session state: `NoActiveCredential` | `CredentialActive` | `CredentialExpired` | `CredentialRevoked` (`m0-endpoint-identity-lifecycle.md` "Credential/session lifecycle") — exposed directly, reusing the accepted vocabulary, not summarized into a derived connectivity flag;
- current Agent presence, represented **separately** as a simple runtime observation (e.g. `Connected` | `Disconnected`): transient/runtime state, distinct from and never derived from the credential dimension above. An Endpoint may hold a valid (`CredentialActive`) credential while currently disconnected — presence and credential validity are different facts (`m0-endpoint-identity-lifecycle.md` "State dimensions") and this representation must not conflate them;
- hardware-confidence state: `Consistent` | `LoweredConfidence` | `Conflict` (`m0-endpoint-identity-lifecycle.md` "Hardware/identity-confidence state") — required so Web can surface a `LoweredConfidence`/`Conflict` condition for operator review, consistent with that Specification's own requirement that these conditions "surface for operator awareness and review";
- current inventory revision identifier/reference, and whether a current inventory revision exists for the Endpoint (i.e., whether at least one durable inventory revision has been recorded, per `m0-persistence-observability-and-domain-events.md` "Inventory persistence boundary"). No open-ended inventory-summary object is defined by this minimum contract — its structure would only be decidable during implementation; a richer inventory read surface is a future extension once the inventory content model itself is specified by a future Work Package, not invented here.

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

1. **Authentication.** This Specification defines a read contract with no access-control mechanism. Even a read-only API exposing Endpoint/Job/inventory state has a genuine access-control question before any real deployment — this Specification does not answer it, consistent with Issue #12's exclusion of "administrative login/authentication mechanism." Issue #12 defines the Server ↔ Web read contract, not production access control: a development/simulated vertical-slice implementation may exercise this contract without establishing the final production administrative authentication model, but such an implementation must not be represented as production-secure. Real production exposure requires an explicit authentication/authorization decision before deployment. This remains owner-relevant and is not solved by any current M0 Work Package (also noted as unresolved in `m0-persistence-observability-and-domain-events.md` "Open questions" for audit-record attribution); no new Work Package or GitHub Issue is created by this task to solve it.
2. **Command/write semantics.** Issue #12 owns only the observation side required by "Web reflects result." Nothing in this Specification allows Web to initiate a Job, cancel a Job, approve enrollment, issue any other destructive command, or use a generic write API. The simulated vertical slice may be initiated by the Simulator/test harness or another internal implementation mechanism instead. If the first vertical slice's actual implementation finds it cannot demonstrate "Web reflects result" through reads alone, that would require a new, separately authorized decision — it is not assumed or designed here.

Neither finding blocks approval of this Specification: both are genuinely out of Issue #12's scope, not gaps in this contract's own read-only completeness.

## Acceptance criteria

- An owner-approved Specification defines the minimum Administrative API / Web read contract for the first vertical slice — this document.
- Web can conceptually observe Endpoint, Job, progress/reconciliation, and terminal result state without database coupling — satisfied by "Read/query surface" and "Representation of absent, pending, failed, terminal, and reconciliation states."
- The contract has an explicit versioning boundary — "Administrative API v1" (see "Wire format and versioning baseline").
- Externally exposed identifiers and state representations are defined sufficiently for independent Server/Web implementation — satisfied by reusing the already-defined Domain vocabulary and the Agent Protocol v1 wire conventions.
- Contract-test expectations are defined (see "Validation expectations").
- No authentication, multi-user, command/write, or broad future API design has been silently introduced — see "Unresolved findings surfaced for owner review."
- No architectural decision required by "Web reflects result" remains hidden in the future implementation Work Package — the delivery-model boundary (snapshot reads are the contract; polling and any future push mechanism are strategy, not architecture) is made explicit above, not assumed.
- The Agent control plane (ADR-0005) is not modified, reinterpreted, or conflated with this Administrative/Management plane — see "Three distinct communication responsibilities."

## Validation expectations

Automated: none produced directly by this Work Package — it is decision/specification work, consistent with Issue #12's own stated validation.

Expected contract-test coverage once implemented, per `docs/development/testing.md` "Contract tests":

- serialization of every resource representation defined above, including the wire-format conventions (timestamp format, opaque identifier strings, omitted-vs-null optional fields);
- correct representation of each Domain state value listed above, with no state silently collapsed or substituted (in particular `AwaitingReconciliation`, `Indeterminate`, and each `failure_reason` value);
- Agent presence represented independently of credential state, including the case of a `CredentialActive` Endpoint currently `Disconnected`;
- absence of a progress snapshot represented as an omitted field, not a default value;
- `GET /api/admin/v1/endpoints/{endpoint_id}` and `GET /api/admin/v1/jobs/{job_id}` returning HTTP `200` with the documented representation for an existing resource, and HTTP `404` for a nonexistent resource identifier;
- the Job response correctly nesting JobStep, Attempt, progress, and Transfer/Artifact summaries as defined above, without exposing separate JobStep/Attempt endpoints.

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

1. A richer inventory read surface — deferred until the inventory content model itself is specified by a future Work Package; not invented here.
2. HTTP conventions beyond the minimum defined above (headers, additional status codes, a broader error catalog) — implementation-time.
3. Generation format for each Domain identifier (`endpoint_id`, `job_id`, `jobstep_id`, `attempt_id`, `transfer_id`, `artifact_id`, inventory revision identifiers) — not chosen here; owned independently by whichever Specification defines each identifier.
4. The mechanism by which the Server determines Agent presence (`Connected`/`Disconnected`) — implementation-time, likely related to the Agent Protocol session/heartbeat, but not designed by this Specification.
5. Authentication mechanism — surfaced as an unresolved finding above, not decided here, not owned by any current M0 Work Package.
6. Whether command/write semantics will eventually be required for the vertical slice — surfaced as an unresolved finding above; not assumed.
7. The eventual production Administrative API's update-notification mechanism (if any) — `docs/discovery/architecture-redesign.md` leaves this open among several candidates for the Web/administrative boundary; this Specification's snapshot-read-only scoping applies to this minimum contract only and does not decide it.

Status: Proposed - awaiting owner approval.
