# M0 — Job Lifecycle and Scheduling

Status: **Approved**

## Context

This Specification details the Job/JobStep/Attempt state model and the resource-lease scheduling model accepted in ADR-0006, executing Issue #4 (`[WP] Define Job lifecycle and scheduling model`). It incorporates the owner-review corrections to the initially proposed model: revalidation before dispatch, Job-scoped endpoint exclusivity, the terminal `Indeterminate` Attempt outcome, a complete Agent Protocol state mapping, and the non-terminal Job `Cancelling` state.

## Domain model

- **Job** — one workflow targeting one Endpoint, composed of an ordered sequence of JobSteps.
- **JobStep** — one stage of that workflow; may be attempted more than once.
- **Attempt** — one execution attempt of a JobStep, corresponding 1:1 to one Agent Protocol `action_id` lifecycle (`docs/specifications/m0-agent-protocol-contract.md`).

## Job lifecycle

States: `Pending`, `Running`, `Cancelling`, `Succeeded`, `Failed`, `Cancelled`.

Transitions:

- `Pending` → `Running`: the target Endpoint's exclusivity lease becomes available and is granted to this Job (see "Endpoint-exclusivity lease"); the Job's first JobStep begins evaluation.
- `Pending` → `Cancelled`: cancellation requested before any JobStep has begun. Nothing active to await; any exclusivity lease already granted is released as part of this transition.
- `Running` → `Succeeded`: every JobStep in the Job's ordered sequence reached `Succeeded`.
- `Running` → `Failed`: a JobStep reached `Failed` (M0 defines no partial-failure/skip tolerance — see "Out of scope").
- `Running` → `Cancelling`: cancellation requested while the Job is active.
  - If a JobStep currently has an active Attempt (`Dispatched`/`InProgress`/`AwaitingReconciliation`), `CancelAction` is sent for it.
  - If no JobStep currently has an active Attempt (e.g., a JobStep is `Pending`/`PreconditionsSatisfied` awaiting Attempt-scoped leases, or between retries), no `CancelAction` is sent — there is nothing dispatched to cancel.
- `Cancelling` (while active): no new JobStep or Attempt may begin.
  - `CancelAck{outcome: Cancelled}`: the active Attempt/JobStep becomes `Cancelled` → Job proceeds to `Cancelled`.
  - `CancelAck{outcome: AlreadyCompleted}`: proves only that the Agent considers the action already terminal — it does **not** by itself reveal whether the Attempt `Succeeded`, `Failed`, or was `Cancelled`. If the Server already holds the Attempt's authoritative terminal outcome (an earlier `ActionResult`), that outcome stands unchanged. If the Server does not yet know the terminal outcome, the Attempt moves to `AwaitingReconciliation` and the Server issues `StatusQuery` to learn it — never assumed from `AlreadyCompleted` alone. The Job remains `Cancelling` until the actual terminal state is known or reconciliation explicitly resolves it (including, if necessary, to `Indeterminate`).
  - `CancelAck{outcome: CannotCancel}`: does **not** by itself produce `Cancelled`. The Attempt continues to its actual terminal outcome (`Succeeded`/`Failed`) → Job proceeds to `Cancelled` only once that outcome is known.
  - `CancelAck{outcome: Unknown}`: requires reconciliation (`AwaitingReconciliation` → `Indeterminate` or a resolved terminal state, per the Attempt lifecycle below); the Job remains `Cancelling` until resolved.
  - No active Attempt existed when cancellation was requested: nothing to await — proceed directly to `Cancelling` → `Cancelled`.
- `Cancelling` → `Cancelled`: reached once the active Attempt (if any) has reached `Succeeded`, `Failed`, `Cancelled`, `Rejected`, or `Indeterminate` with no further Attempt authorized, or there was no active Attempt to begin with, and no active or uncertain execution remains. The Job's endpoint-exclusivity lease is released at this point.
- `Succeeded` / `Failed` / `Cancelled` are terminal; no further transitions.

## Endpoint-exclusivity lease (Job-scoped)

One Job owns its target Endpoint exclusively for the Job's entire active lifetime. M0 does not support interleaving two active Jobs against the same Endpoint.

- Acquired at `Pending` → `Running` admission. A Job remains `Pending`, queued, while its target Endpoint's exclusivity lease is held by another active Job.
- Retained without exception across JobStep boundaries, Attempt boundaries, retries, and `AwaitingReconciliation`, for as long as the Job is `Running` or `Cancelling`.
- Released only at a genuinely terminal Job state (`Succeeded`, `Failed`, `Cancelled`). Never released while any Attempt's fate is uncertain, because `Cancelled` itself is not reached until that uncertainty resolves.

## Other resource leases (Attempt-scoped)

Lease types (extensible, not closed): network capacity, storage read/write capacity, CPU/worker capacity.

- Acquired immediately before an Attempt's dispatch, jointly with precondition revalidation (see below).
- Released when that Attempt reaches `Succeeded`, `Failed`, `Cancelled`, `Rejected`, or `Indeterminate`.
- Retention during `AwaitingReconciliation` is implementation-time policy, not decided here — unlike endpoint exclusivity, these lease types carry no equivalent safety implication.

## JobStep lifecycle

States: `Pending`, `PreconditionsSatisfied`, `Dispatching`, `Succeeded`, `Failed`, `Cancelled`.

Transitions:

- `Pending` → `PreconditionsSatisfied`: **preliminary eligibility only**. Confirms whichever JobStep-specific declared preconditions do not depend on Attempt-scoped resource leases or workflow/scheduler authorization (e.g., a prior JobStep's required output existing). Does **not** claim that the complete destructive dispatch precondition set (see "Destructive dispatch preconditions" below) already holds — workflow/scheduler authorization and Attempt-scoped leases are not yet established at this stage. Makes the JobStep eligible to request Attempt-scoped leases; does not by itself authorize dispatch.
- `Pending` / `PreconditionsSatisfied` → `Failed{failure_reason: PreconditionNotMet}`: preconditions evaluated and do not hold (initial evaluation).
- `PreconditionsSatisfied` → `Dispatching`: Attempt-scoped leases acquired, revalidation (below) passes, and the first Attempt is dispatched.
- `PreconditionsSatisfied` → `Pending`: Attempt-scoped leases acquired but revalidation fails — see "Revalidation immediately before dispatch." No Attempt is created; leases are released.
- `Dispatching` → `Dispatching`: a new Attempt is created per retry policy after a prior Attempt's `Rejected`, `Failed`, or `Indeterminate` outcome. The JobStep remains `Dispatching` across retries.
- `Dispatching` → `Succeeded`: the current (or a retried) Attempt reached `Succeeded`.
- `Dispatching` → `Failed`: the current Attempt reached a terminal failure (`Failed`, `Rejected`) or `Indeterminate`, and retry policy determines no further Attempt will be made. `failure_reason`: `DispatchRejected`, `ExecutionFailed`, or `ReconciliationIndeterminate` as applicable.
- `Dispatching` → `Cancelled`: `CancelAck{Cancelled}` or `ActionResult{outcome: Cancelled}` received, per the Job cancellation rules above.
- `Succeeded` / `Failed` / `Cancelled` are terminal; no further transitions.

## Workflow/scheduler authorization (ADR-0004 precondition 3)

Defined **strictly as workflow/scheduler-level authorization** — it does not itself require, or recursively depend on, the complete destructive dispatch precondition set below, of which it is one independent member. Required for **every** Attempt dispatch, destructive or not:

1. the Job is `Running`, holds its Endpoint-exclusivity lease, and is not `Cancelling` or terminal;
2. the JobStep is the Job's current active step;
3. retry/reconciliation policy permits creation of this Attempt (see "Retry policy");
4. all required Attempt-scoped resource leases are held;
5. no unresolved prior Attempt state exists that requires an explicit decision (an operator decision, for a destructive JobStep) before another Attempt may be created.

## Destructive dispatch preconditions

For a destructive JobStep, dispatch additionally requires the full composition of the six independent preconditions from `docs/specifications/m0-endpoint-identity-lifecycle.md`:

1. trusted persistent Endpoint identity;
2. authenticated current Agent session (`CredentialActive`);
3. workflow/scheduler authorization — exactly the definition immediately above, **not** recursively this complete six-item set;
4. sufficiently fresh inventory (current inventory revision);
5. target disk identity/fingerprint revalidation;
6. hardware confidence `Consistent` (not `LoweredConfidence`/`Conflict`).

A non-destructive JobStep requires only workflow/scheduler authorization, plus whichever of its own declared preconditions are time-sensitive (see "Revalidation immediately before dispatch").

## Revalidation immediately before dispatch

`PreconditionsSatisfied` is preliminary eligibility only — it does not assert that workflow/scheduler authorization or the destructive dispatch preconditions already hold, since Attempt-scoped leases and workflow authorization are not yet established at that stage. Once Attempt-scoped leases are held, the complete precondition set relevant to the JobStep — workflow/scheduler authorization always, plus the full six-item destructive dispatch composition for a destructive JobStep — is evaluated once, atomically, **immediately before the durable dispatch commitment is created and persisted** (`docs/decisions/0007-persistence-backend-and-durable-transient-boundary.md`'s persist-before-send ordering: leases held → revalidate → durably commit → transaction commits → transmission attempted immediately after). Revalidation gates the durable commitment, not the WebSocket send directly — there is no second precondition evaluation between the commit and the transmission attempt.

If revalidation fails: the Attempt is **not** created, no durable dispatch commitment is persisted, `ActionDispatch` is **not** sent, the just-acquired Attempt-scoped leases are released, and the JobStep returns to `Pending`. A JobStep must never dispatch based only on a stale earlier `PreconditionsSatisfied` evaluation. Whether repeated revalidation failure should eventually produce a terminal `Failed{PreconditionNotMet}` is implementation-time policy, not decided here.

## Attempt lifecycle

States: `Dispatched`, `InProgress`, `AwaitingReconciliation`, `Succeeded`, `Failed`, `Cancelled`, `Rejected`, `Indeterminate`.

`Dispatched` means the Server has durably committed the Attempt for dispatch and the `ActionDispatch` *may* have been transmitted — not that transmission is confirmed. A durable database transaction and the Agent WebSocket send cannot be atomic with each other, so the Server persists the Attempt as `Dispatched` (per `docs/decisions/0007-persistence-backend-and-durable-transient-boundary.md`'s persist-before-send ordering) and only then attempts transmission, immediately, with no deliberate delay. No `ActionAck` has yet established accepted delivery at this point. On restart, a persisted `Dispatched` Attempt is an uncertain delivery outcome exactly like `InProgress`, and enters `AwaitingReconciliation` rather than being retransmitted blindly.

Transitions:

- `Dispatched` → `InProgress`: `ActionAck{outcome: Accepted}` received.
- `Dispatched` → `Rejected`: `ActionAck{outcome: Rejected}` received. Never represented as `Failed`.
- `Dispatched` → `AwaitingReconciliation`: `ActionAck` not received within the expected acknowledgment window (an uncertain delivery outcome, not proof of non-receipt).
- `InProgress` → `Succeeded` / `Failed`: `ActionResult{outcome: Succeeded | Failed}` received.
- `InProgress` → `Cancelled`: `CancelAck{outcome: Cancelled}` or `ActionResult{outcome: Cancelled}` received.
- `InProgress` → `AwaitingReconciliation`: connection to the Agent is lost, or the Server restarts, while the Attempt was `Dispatched` or `InProgress`.
- `AwaitingReconciliation` → `InProgress`: `StatusReport{known_state: Accepted}` or `StatusReport{known_state: Running}` received. Both Agent-side `Accepted` and `Running` map to Attempt `InProgress` — the Server does not need the Agent Protocol's finer-grained distinction between "accepted, not yet started" and "running" at this level.
- `AwaitingReconciliation` → `Succeeded` / `Failed` / `Cancelled`: `StatusReport` reports a matching terminal Agent-action state, adopted explicitly.
- `AwaitingReconciliation` → `Indeterminate`: an **explicit reconciliation decision** closes the Attempt as indeterminate — typically because `StatusReport{known_state: Unknown}` was received and no further evidence is expected, or because no Agent session re-establishes within an implementation-defined window and the decision is made to stop waiting. Never automatic merely because one `Unknown` `StatusReport` was received.
- `Succeeded` / `Failed` / `Cancelled` / `Rejected` / `Indeterminate` are terminal; no further transitions. `Indeterminate` must never be interpreted as `Succeeded`, `Failed`, or "not executed" — it means the Server cannot establish whether or how the Attempt executed.

## Agent Protocol state mapping

| Agent Protocol (`m0-agent-protocol-contract.md`) | Attempt state |
|---|---|
| `ActionAck{Accepted}` | `InProgress` |
| `StatusReport{known_state: Accepted}` | `InProgress` |
| `StatusReport{known_state: Running}` | `InProgress` |
| `ActionResult{Succeeded}` | `Succeeded` |
| `ActionResult{Failed}` | `Failed` |
| `ActionResult{Cancelled}` / `CancelAck{Cancelled}` | `Cancelled` |
| `ActionAck{Rejected}` | `Rejected` |
| `ActionAck` timeout | `AwaitingReconciliation` |
| Connection loss / Server restart while `Dispatched`/`InProgress` | `AwaitingReconciliation` |
| `StatusReport{known_state: Unknown}`, after explicit reconciliation decision | `Indeterminate` |

No Attempt state duplicates an Agent Protocol state 1:1 beyond what this table defines; in particular, the Agent-local `Accepted`/`Running` distinction is deliberately collapsed into one Attempt state (`InProgress`), and `Unknown` is deliberately **not** mapped directly to `Indeterminate` — it requires the explicit decision step.

## Reconciliation procedure

On Server restart:

1. every persisted Attempt in `Dispatched` or `InProgress` is loaded as `AwaitingReconciliation`;
2. when the relevant Agent session re-establishes (`docs/specifications/m0-agent-protocol-contract.md` handshake), the Server issues `StatusQuery{action_id}` for each such Attempt;
3. the response is applied per the Agent Protocol state mapping and Attempt-lifecycle transitions above;
4. no Attempt is redispatched, resumed, or assumed-successful without an explicit `StatusReport`, or — for a destructive JobStep whose reconciliation resolves to `Indeterminate` — an explicit recorded operator decision that also determines whether a new Attempt is authorized.

## Retry policy

- **Destructive JobSteps**: no automatic retry under any circumstance, for any Attempt outcome (`Failed`, `Rejected`, `Indeterminate`). A further Attempt requires an explicit, recorded operator decision — the same decision that closes a prior uncertain Attempt as `Indeterminate`.
- **Non-destructive JobSteps**: an automatic, bounded retry (fresh Attempt) is permitted after `Failed`. After `Indeterminate`, a retry is permitted only when that JobStep's own retry policy explicitly supports retrying from an indeterminate outcome — being non-destructive does not by itself imply duplicate execution is safe. Exact bounds/backoff and per-JobStep-type opt-in are implementation-time detail.
- `Rejected` is never auto-retried by treating it as equivalent to `Failed`.

## Job admission

Checked once per Job, at `Pending` → `Running`:

1. the target Endpoint's exclusivity lease is available and is granted to this Job.

Distinct from, and prior to, the per-Attempt "Workflow/scheduler authorization" and "Destructive dispatch preconditions" above, which are evaluated at every dispatch throughout the Job's active lifetime, not just at admission.

## Out of scope

- partial-failure/skip semantics for Jobs — not decided, a future Specification if a concrete requirement emerges;
- DAG/branching JobStep structures — not evidenced by current requirements;
- scheduling algorithm (ordering, fairness, priority among competing leases, including fairness among `Pending` Jobs queued for the same Endpoint) — implementation-time;
- lease retention policy for non-exclusivity resource types during `AwaitingReconciliation` — implementation-time;
- persistence technology and domain-event catalog (Issue #5);
- transfer-specific JobStep preconditions/postconditions (Issue #6);
- Agent-side protocol mechanics — already decided (Issue #3 / ADR-0005), only consumed here.

## Acceptance criteria

- Job/JobStep/Attempt state machine defined with valid and rejected transitions, including `Cancelling` and `Indeterminate` (Issue #4 acceptance criterion).
- Workflow/scheduler authorization defined non-circularly at both Job-admission and Attempt-dispatch granularity, satisfying ADR-0004's deferred precondition 3 without recursing into the destructive dispatch precondition set it is itself one member of.
- Resource-lease model defined, with endpoint exclusivity correctly scoped to the Job.
- Time-sensitive preconditions are revalidated immediately before every dispatch, not assumed valid from an earlier evaluation.

## Validation expectations

Per `docs/development/testing.md` "Unit and domain tests":

- state-transition tests (valid and rejected) for Job (including `Pending`→`Cancelled` and `Cancelling`→`Cancelled` paths), JobStep, and Attempt independently;
- revalidation tests: a JobStep whose destructive preconditions become false between `PreconditionsSatisfied` and lease acquisition must return to `Pending` and must never dispatch;
- retry-policy tests demonstrating a destructive JobStep's Attempt never triggers an automatic retry from `Failed`, `Rejected`, or `Indeterminate`;
- `Indeterminate` tests: an Attempt in `AwaitingReconciliation` receiving `StatusReport{Unknown}` does not automatically become `Indeterminate` without the explicit reconciliation decision step, and a destructive JobStep's next Attempt is never authorized without a recorded operator decision;
- Agent Protocol state-mapping tests: `StatusReport{Accepted}` and `StatusReport{Running}` both resolve to Attempt `InProgress`.

Per "Unit and domain tests" (resource leases): lease acquisition/release tests, including that the Job-scoped endpoint-exclusivity lease is acquired only at Job admission, is never released while the Job is `Running`/`Cancelling`, and correctly blocks a second Job from admission against the same Endpoint.

Per `docs/development/testing.md` "Persistence and recovery tests": Server-restart reconciliation scenarios covering `Running`, a terminal state, and `Unknown` → `Indeterminate` resolution; a `Cancelling` Job surviving restart with its exclusivity lease intact.

Per `docs/development/testing.md` "Simulator": scheduler contention and reconciliation scenarios at 20–24 concurrent endpoints, including disconnect mid-Attempt, Server restart with in-flight Attempts, and Job cancellation while a destructive JobStep is in progress.

Per "Local development environments," these are expected to run in the Linux reference environment (WSL2 or containers from Windows).

Manual: owner approval of this Specification — confirmed (see Status).

## Related ADRs

- ADR-0006 — Job/JobStep/Attempt state model and resource-lease scheduling (`Accepted`).
- ADR-0004 — Endpoint identity and enrollment/trust bootstrap model (destructive-operation authorization preconditions).
- ADR-0005 — Agent control-plane protocol and typed-action model (Agent-action states, retry mechanism, `StatusQuery`).
- ADR-0007 — Persistence backend and durable/transient boundary (`Accepted`; persist-before-send dispatch ordering `Dispatched` relies on).

## Related work

- Issue #4 — `[WP] Define Job lifecycle and scheduling model`.
- Issue #2 / ADR-0004 — Endpoint identity and destructive-operation preconditions.
- Issue #3 / ADR-0005 — Agent Protocol v1.
- Issue #5 / ADR-0007 — persistence, observability, and domain-event model, including the persist-before-send dispatch ordering and an `Indeterminate` notification event.
- Issue #6 — `[WP] Define data-plane and storage contracts` (transfer JobSteps).
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (reconciliation and cancellation scenarios).

## Open questions

1. Partial-failure/skip semantics for Jobs — not decided, future Specification if needed.
2. DAG/branching JobStep structure — not evidenced, future Specification if needed.
3. Scheduling algorithm (ordering/fairness/priority among competing leases, and among `Pending` Jobs queued for the same Endpoint) — implementation-time.
4. Lease retention policy for non-exclusivity resources during `AwaitingReconciliation` — implementation-time.
5. Whether repeated revalidation failure should eventually produce a terminal `Failed{PreconditionNotMet}` rather than returning to `Pending` indefinitely — implementation-time.

Status: Approved.
