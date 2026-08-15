# M0 — Job Lifecycle and Scheduling

Status: **Proposed - awaiting owner approval**

## Context

This Specification details the Job/JobStep/Attempt state model and the resource-lease scheduling model accepted in ADR-0006, executing Issue #4 (`[WP] Define Job lifecycle and scheduling model`).

## Domain model

- **Job** — one workflow targeting one Endpoint, composed of an ordered sequence of JobSteps.
- **JobStep** — one stage of that workflow; may be attempted more than once.
- **Attempt** — one execution attempt of a JobStep, corresponding 1:1 to one Agent Protocol `action_id` lifecycle (`docs/specifications/m0-agent-protocol-contract.md`).

## Job lifecycle

States: `Pending`, `Running`, `Succeeded`, `Failed`, `Cancelled`.

Transitions:

- `Pending` → `Running`: the Job's first JobStep begins evaluation.
- `Running` → `Succeeded`: every JobStep in the Job's ordered sequence reached `Succeeded`.
- `Running` → `Failed`: a JobStep reached `Failed` (M0 defines no partial-failure/skip tolerance — see "Out of scope").
- `Running` → `Cancelled`: explicit cancellation, propagated to the currently active JobStep/Attempt via `CancelAction`.
- `Succeeded` / `Failed` / `Cancelled` are terminal; no further transitions.

## JobStep lifecycle

States: `Pending`, `PreconditionsSatisfied`, `Dispatching`, `Succeeded`, `Failed`, `Cancelled`.

Transitions:

- `Pending` → `PreconditionsSatisfied`: preconditions evaluated and hold, including — for a destructive JobStep — the full destructive-operation authorization precondition set (`docs/specifications/m0-endpoint-identity-lifecycle.md`).
- `Pending` / `PreconditionsSatisfied` → `Failed`: preconditions evaluated and do not hold. The Attempt-independent `Failed` here is recorded with a `failure_reason` of `PreconditionNotMet`, distinguishing it from an Attempt-level failure.
- `PreconditionsSatisfied` → `Dispatching`: required resource leases acquired and the first Attempt dispatched.
- `Dispatching` → `Dispatching`: a new Attempt is created per retry policy (see "Retry policy") after a prior Attempt's `Rejected`, `Failed`, or unresolved `AwaitingReconciliation`. The JobStep remains `Dispatching` across retries.
- `Dispatching` → `Succeeded`: the current (or a retried) Attempt reached `Succeeded`.
- `Dispatching` → `Failed`: the current Attempt reached a terminal failure (`Failed` or `Rejected`) and retry policy determines no further Attempt will be made. `failure_reason` records `DispatchRejected`, `ExecutionFailed`, or `ReconciliationFailed` as applicable.
- `Dispatching` → `Cancelled`: `CancelAck` confirms cancellation, or `ActionResult{outcome: Cancelled}` is received.
- `Succeeded` / `Failed` / `Cancelled` are terminal; no further transitions.

## Attempt lifecycle

States: `Dispatched`, `InProgress`, `AwaitingReconciliation`, `Succeeded`, `Failed`, `Cancelled`, `Rejected`.

Transitions:

- `Dispatched` → `InProgress`: `ActionAck{outcome: Accepted}` received.
- `Dispatched` → `Rejected`: `ActionAck{outcome: Rejected}` received. Never represented as `Failed` (ADR-0005).
- `Dispatched` → `AwaitingReconciliation`: `ActionAck` not received within the expected acknowledgment window (an uncertain delivery outcome, not proof of non-receipt — `docs/specifications/m0-agent-protocol-contract.md` "Acknowledgment timeout semantics").
- `InProgress` → `Succeeded` / `Failed`: `ActionResult{outcome: Succeeded | Failed}` received.
- `InProgress` → `Cancelled`: `CancelAck{outcome: Cancelled}` or `ActionResult{outcome: Cancelled}` received.
- `InProgress` → `AwaitingReconciliation`: connection to the Agent is lost, or the Server restarts, while the Attempt was `Dispatched` or `InProgress`.
- `AwaitingReconciliation` → `InProgress`: `StatusReport{known_state: Running}` received.
- `AwaitingReconciliation` → `Succeeded` / `Failed` / `Cancelled`: `StatusReport` reports a matching terminal Agent-action state, adopted explicitly.
- `AwaitingReconciliation` → `AwaitingReconciliation` (remains): `StatusReport{known_state: Unknown}` received, or no Agent session re-establishes within an implementation-defined window. Resolution follows the retry policy below.
- `Succeeded` / `Failed` / `Cancelled` / `Rejected` are terminal; no further transitions. In particular, `AwaitingReconciliation` never transitions to a terminal state without an explicit `StatusReport` or, for a destructive JobStep, an explicit recorded operator decision.

## Resource leases

Lease types (extensible, not closed): endpoint exclusivity, network capacity, storage read/write capacity, CPU/worker capacity.

- Acquired when a JobStep enters `PreconditionsSatisfied` and is about to dispatch its (next) Attempt.
- Released when that Attempt reaches `Succeeded`, `Failed`, `Cancelled`, or `Rejected`.
- **Endpoint-exclusivity lease**: retained through `AwaitingReconciliation` without exception — this is the one safety-relevant retention rule this Specification fixes. No other JobStep may be dispatched against the same Endpoint while an Attempt targeting it is in `AwaitingReconciliation`.
- Retention of other lease types (network, storage, CPU/worker) during `AwaitingReconciliation` is implementation-time policy, not decided here.

## "Authorized Job/action" (ADR-0004 precondition 3)

At dispatch time, all of the following must hold:

1. the Job is `Running` and the JobStep is the Job's current active step;
2. the JobStep is `PreconditionsSatisfied`, or is creating a new Attempt permitted by retry policy;
3. no other Attempt for the same Endpoint is currently `AwaitingReconciliation` (endpoint-exclusivity lease held, uncontested);
4. all required resource leases for this Attempt are currently held.

## Retry policy

- **Destructive JobSteps**: no automatic retry under any circumstance, for any Attempt outcome. A further Attempt requires an explicit, recorded operator decision.
- **Non-destructive JobSteps**: an automatic, bounded retry (fresh Attempt) is permitted after `Failed` or an `AwaitingReconciliation` resolving to `Unknown`. Exact bounds/backoff and per-JobStep-type opt-in are implementation-time detail.
- `Rejected` is never auto-retried by treating it as equivalent to `Failed` — a rejection often indicates a version/compatibility problem, not a transient execution failure.

## Reconciliation procedure

On Server restart:

1. every persisted Attempt in `Dispatched` or `InProgress` is loaded as `AwaitingReconciliation`;
2. when the relevant Agent session re-establishes (`docs/specifications/m0-agent-protocol-contract.md` handshake), the Server issues `StatusQuery{action_id}` for each such Attempt;
3. the response is applied per the Attempt-lifecycle transitions above;
4. no Attempt is redispatched, resumed, or assumed-successful without an explicit `StatusReport`, or — for a destructive JobStep whose `StatusReport` is `Unknown` — an explicit recorded operator decision.

## Out of scope

- partial-failure/skip semantics for Jobs (e.g., an "optional" JobStep that does not fail the whole Job) — not decided, a future Specification if a concrete requirement emerges;
- DAG/branching JobStep structures — not evidenced by current requirements;
- scheduling algorithm (ordering, fairness, priority among competing leases) — implementation-time;
- lease retention policy for non-exclusivity resource types during `AwaitingReconciliation` — implementation-time;
- persistence technology and domain-event catalog (Issue #5);
- transfer-specific JobStep preconditions/postconditions (Issue #6);
- Agent-side protocol mechanics — already decided (Issue #3 / ADR-0005), only consumed here.

## Acceptance criteria

- Job/JobStep/Attempt state machine defined with valid and rejected transitions (Issue #4 acceptance criterion).
- "Authorized Job/action" defined, satisfying ADR-0004's deferred precondition 3.
- Resource-lease model defined, including the endpoint-exclusivity safety rule.

## Validation expectations

Per `docs/development/testing.md` "Unit and domain tests": state-transition tests (valid and rejected) for Job, JobStep, and Attempt independently, including that a destructive JobStep's Attempt never triggers an automatic retry from `Failed`, `Rejected`, or an `Unknown`-resolved `AwaitingReconciliation`.

Per "Unit and domain tests" (resource leases): lease acquisition/release tests, including that an endpoint-exclusivity lease is never released while an Attempt for that Endpoint is `AwaitingReconciliation`, and that a second JobStep cannot acquire it while contested.

Per `docs/development/testing.md` "Persistence and recovery tests": Server-restart reconciliation scenarios — a `Dispatched`/`InProgress` Attempt reloaded as `AwaitingReconciliation`, resolved via `StatusReport` for each of `Running`, a terminal state, and `Unknown`.

Per `docs/development/testing.md` "Simulator": scheduler contention and reconciliation scenarios at 20–24 concurrent endpoints, including disconnect mid-Attempt and Server restart with in-flight Attempts.

Per "Local development environments," these are expected to run in the Linux reference environment (WSL2 or containers from Windows).

Manual: owner approval of this Specification.

## Related ADRs

- ADR-0006 — Job/JobStep/Attempt state model and resource-lease scheduling (`Accepted`).
- ADR-0004 — Endpoint identity and enrollment/trust bootstrap model (destructive-operation authorization preconditions).
- ADR-0005 — Agent control-plane protocol and typed-action model (Agent-action states, retry mechanism, `StatusQuery`).

## Related work

- Issue #4 — `[WP] Define Job lifecycle and scheduling model`.
- Issue #2 / ADR-0004 — Endpoint identity and destructive-operation preconditions.
- Issue #3 / ADR-0005 — Agent Protocol v1.
- Issue #5 — `[WP] Define persistence, observability, and domain-event model` (durability; domain events).
- Issue #6 — `[WP] Define data-plane and storage contracts` (transfer JobSteps).
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (reconciliation scenarios).

## Open questions

1. Partial-failure/skip semantics for Jobs — not decided, future Specification if needed.
2. DAG/branching JobStep structure — not evidenced, future Specification if needed.
3. Scheduling algorithm (ordering/fairness/priority among competing leases) — implementation-time.
4. Lease retention policy for non-exclusivity resources during `AwaitingReconciliation` — implementation-time.

Status: Proposed - awaiting owner approval.
