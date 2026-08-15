# ADR-0006: Job/JobStep/Attempt state model and resource-lease scheduling

Status: Accepted

## Context

M0 requires resolving the durable Job/JobStep state model and the scheduler/resource-lease model (`docs/discovery/adr-triage.md` candidates 7, 11; `docs/specifications/m0-architecture-baseline.md` scope items "Job/JobStep lifecycle", "scheduler and resource model"). Issue #4 executes this Work Package.

`docs/discovery/architecture-redesign.md` "Durable workflow": "Each relevant provisioning stage is a JobStep with preconditions, execution state, result, postconditions, retry semantics, and cancellation semantics. After power loss or reconnect, the Server must reconcile actual endpoint state with durable workflow state. Destructive operations must never be automatically retried merely because a generic retry policy exists."

`docs/discovery/architecture-redesign.md` "Capacity and scheduling": concurrency must not be a single fixed global number; JobSteps compete for resource leases/tokens representing endpoint exclusivity, network capacity, storage read/write capacity, CPU/worker capacity, and other constrained resources.

`docs/discovery/architecture-redesign.md` "Proposed component boundaries" already names `Attempt` as a distinct Domain concept alongside `Job` and `JobStep`; this ADR is the first to give it concrete meaning.

This ADR also resolves two questions explicitly deferred to Issue #4 by earlier M0 ADRs: what "authorized Job/action" means for ADR-0004's destructive-operation authorization precondition 3, and the retry *policy* that ADR-0005 left open (ADR-0005 defines only the retry *mechanism* — a fresh `action_id` with `retry_of`).

This revision incorporates owner review corrections to the initially accepted model: revalidation of time-sensitive preconditions immediately before dispatch, Job-scoped (not Attempt-scoped) endpoint exclusivity, an explicit terminal `Indeterminate` Attempt outcome, a complete Agent Protocol state mapping, and a non-terminal Job `Cancelling` state.

## Decision

### Three-tier domain model: Job, JobStep, Attempt

- **Job**: one overall workflow targeting a single Endpoint (e.g., a provisioning or recovery workflow), composed of an ordered sequence of JobSteps. M0 defines a linear sequence, not a DAG — no current requirement or evidence describes branching or parallel JobSteps within one Job.
- **JobStep**: one stage of the Job's workflow (e.g., "write image", "verify backup"). Carries preconditions, results, postconditions, and cancellation semantics. A JobStep may be attempted more than once; each attempt is a separate `Attempt`.
- **Attempt**: one execution attempt of a JobStep, corresponding 1:1 to one Agent Protocol `action_id` lifecycle (ADR-0005). A retry creates a new Attempt with a fresh `action_id` and `retry_of` referencing the prior Attempt's `action_id`. The Attempt consumes the Agent Protocol's per-action states and translates them into this Server-side durable record — the Attempt's own state is the Server's authoritative interpretation, not a re-export of the Agent's local vocabulary.

### Job states

`Pending → Running → Cancelling → {Succeeded | Failed | Cancelled}` (`Cancelling` reachable only from `Running`; `Cancelled` also reachable directly from `Pending`)

- `Pending`: created. Also the state in which a Job waits for its target Endpoint's exclusivity lease to become available (see "Endpoint-exclusivity lease" below).
- `Running`: the Job holds its Endpoint's exclusivity lease and is iterating through its ordered JobSteps.
- `Cancelling`: cancellation was requested while the Job was `Running`. No new JobStep or Attempt may begin while `Cancelling`. A cancellation request is not itself proof that execution stopped — the Job remains `Cancelling` until the currently active Attempt reaches a terminal outcome or is closed `Indeterminate` with no further Attempt authorized (see "Job cancellation" below).
- `Succeeded`: every JobStep in the Job's ordered sequence reached `Succeeded` (not reachable once cancellation has been requested).
- `Failed`: a JobStep reached `Failed` and no partial-failure/skip policy applied. M0 defines none — a single JobStep failure fails the Job. Finer-grained partial-failure semantics remain an explicit open question.
- `Cancelled`: reached from `Pending` (no Attempt ever started, nothing active to await) or from `Cancelling` (once the workflow is known to have stopped and no active or uncertain execution remains).

### Endpoint-exclusivity lease (Job-scoped)

Exclusivity is scoped to the **Job**, not the Attempt or JobStep: one Job owns its target Endpoint exclusively for the Job's entire active lifetime. M0 does not need interleaving of two active Jobs against the same Endpoint.

- Acquired when the Job is admitted from `Pending` to `Running`. A Job remains `Pending` until its target Endpoint's exclusivity lease is available and granted to it; a second Job targeting an already-exclusively-held Endpoint remains `Pending` (queued) rather than being rejected outright.
- Retained, without exception, across JobStep boundaries, Attempt boundaries, retries, and `AwaitingReconciliation`, for as long as the Job is `Running` or `Cancelling`.
- Released only when the Job reaches a genuinely terminal state (`Succeeded`, `Failed`, or `Cancelled`). Because `Cancelled` is itself only reached once no active or uncertain execution remains, release never happens while an Attempt's fate is still uncertain.

### Other resource leases (Attempt-scoped)

Network capacity, storage read/write capacity, CPU/worker capacity, and other non-exclusivity resource types (extensible, not a closed set) remain scoped to individual Attempts: acquired immediately before an Attempt's dispatch, jointly with the revalidation step below, and released when that Attempt reaches a terminal state (`Succeeded`, `Failed`, `Cancelled`, `Rejected`, or `Indeterminate`). Retention policy for these lease types during `AwaitingReconciliation` remains implementation-time, not decided here. Lease acquisition ordering, fairness, and priority across competing JobSteps are likewise not decided here — M0 requires the lease-competition mechanism to exist, not a specific scheduling algorithm.

### JobStep states

`Pending → PreconditionsSatisfied → Dispatching → {Succeeded | Failed | Cancelled}`

- `Pending`: created, preconditions not yet evaluated.
- `PreconditionsSatisfied`: **preliminary eligibility only**. Confirms whichever JobStep-specific declared preconditions do not depend on Attempt-scoped resource leases or workflow/scheduler authorization (for example, a prior JobStep's required output existing). It does **not** claim that the complete destructive dispatch precondition set (see "Destructive dispatch preconditions" below) already holds — workflow/scheduler authorization and Attempt-scoped leases are not yet established at this stage. It only makes the JobStep eligible to request its Attempt-scoped leases.
- `Dispatching`: at least one Attempt has been made; persists across retries. The JobStep does not reach a terminal state merely because one Attempt failed or was closed `Indeterminate`, if retry policy permits another Attempt.
- `Succeeded` / `Failed` / `Cancelled`: terminal. `Failed` carries a `failure_reason`: `PreconditionNotMet` (initial or revalidation failure), `DispatchRejected`, `ExecutionFailed`, or `ReconciliationIndeterminate` (retry policy determined no further Attempt will be made after an `Indeterminate` outcome).

### Workflow/scheduler authorization (satisfies ADR-0004's deferred precondition 3)

Defined **strictly as workflow/scheduler-level authorization**. It does not itself require, or recursively depend on, the complete destructive dispatch precondition set below — of which it is one independent member, not a synonym for the whole set. Required for **every** Attempt dispatch, destructive or not:

1. the Job is `Running`, holds its Endpoint-exclusivity lease, and is not `Cancelling` or terminal;
2. the JobStep is the Job's current active step;
3. retry/reconciliation policy permits creation of this Attempt (see "Retry policy");
4. all required Attempt-scoped resource leases are held;
5. no unresolved prior Attempt state exists that requires an explicit decision (an operator decision, for a destructive JobStep) before another Attempt may be created.

### Destructive dispatch preconditions

For a destructive JobStep, dispatch additionally requires the full composition of the six independent preconditions from `docs/specifications/m0-endpoint-identity-lifecycle.md`:

1. trusted persistent Endpoint identity;
2. authenticated current Agent session (`CredentialActive`);
3. workflow/scheduler authorization — exactly the definition immediately above, **not** recursively this complete six-item set;
4. sufficiently fresh inventory (current inventory revision);
5. target disk identity/fingerprint revalidation;
6. hardware confidence `Consistent` (not `LoweredConfidence`/`Conflict`).

A non-destructive JobStep requires only workflow/scheduler authorization, plus whichever of its own declared preconditions are time-sensitive (see "Revalidation immediately before dispatch").

### Revalidation immediately before dispatch

`PreconditionsSatisfied` is preliminary eligibility only (see "JobStep states") — it does not assert that workflow/scheduler authorization or the destructive dispatch preconditions already hold, since Attempt-scoped leases and workflow authorization are not yet established at that stage. Once Attempt-scoped leases are held, the complete precondition set relevant to the JobStep — workflow/scheduler authorization always, plus the full six-item destructive dispatch composition for a destructive JobStep — is evaluated once, atomically, **immediately before the durable dispatch commitment is created and persisted** (ADR-0007's persist-before-send ordering, step 2 of 5: leases held → revalidate → durably commit → transaction commits → transmission attempted immediately after). Revalidation gates the durable commitment, not the WebSocket send directly — there is no second precondition evaluation between the commit and the transmission attempt.

If revalidation fails, the Attempt is **not** created, no durable dispatch commitment is persisted, and `ActionDispatch` is **not** sent. The just-acquired Attempt-scoped leases are released, and the JobStep returns to `Pending` for re-evaluation. A JobStep must never dispatch based only on a stale earlier `PreconditionsSatisfied` evaluation. Whether repeated revalidation failure should eventually produce a terminal `Failed{PreconditionNotMet}` (e.g., after a bounded number of cycles, or immediately for a permanent condition such as the Endpoint reaching `Retired`) is implementation-time policy, not decided here.

### Attempt states

`Dispatched → InProgress → {Succeeded | Failed | Cancelled | Rejected}`, with `AwaitingReconciliation` reachable from `Dispatched` or `InProgress`, and `Indeterminate` reachable only from `AwaitingReconciliation`.

- `Dispatched`: the Server has durably committed the Attempt for dispatch (per ADR-0007's persist-before-send ordering) and the `ActionDispatch` may have been transmitted; no `ActionAck` has yet established accepted delivery. Under normal operation, the Server attempts transmission immediately after the durable commit — `Dispatched` does not imply a deliberate delay before sending. On Server restart, any persisted `Dispatched` Attempt is treated as an uncertain delivery outcome and enters `AwaitingReconciliation`, exactly as already required for `InProgress`. A persisted `Dispatched` state must never cause blind retransmission of a destructive action.
- `InProgress`: the Agent has accepted the action and no terminal outcome is yet known. Reached from `ActionAck{outcome: Accepted}`, and equally from `StatusReport{known_state: Accepted}` or `StatusReport{known_state: Running}` during reconciliation — both Agent-side `Accepted` and `Running` map to this one Attempt state. This Specification-level model does not duplicate the Agent Protocol's finer-grained Accepted/Running distinction; the Server does not need it at this granularity.
- `Rejected`: `ActionAck{outcome: Rejected}` received — the Attempt never executed. Kept distinct from `Failed`, consistent with ADR-0005.
- `Succeeded` / `Failed` / `Cancelled`: terminal, populated from `ActionResult` or `CancelAck{outcome: Cancelled}`.
- `AwaitingReconciliation`: non-terminal. The Attempt's true outcome cannot currently be determined — reached on `ActionAck` timeout (an uncertain delivery outcome, not proof of non-execution), on connection loss while `InProgress`, or on Server restart with an Attempt that was `Dispatched`/`InProgress` at the time of the crash.
- `Indeterminate` (terminal): the Server has concluded, through an **explicit reconciliation decision**, that it cannot establish whether or how the Attempt executed. Must never be interpreted as `Succeeded`, `Failed`, or "not executed" — it is its own distinct outcome. Reached only from `AwaitingReconciliation`, never automatically merely because a single `StatusReport{known_state: Unknown}` was received.

### Reconciliation and the Indeterminate outcome

- `AwaitingReconciliation` → `InProgress`: `StatusReport{known_state: Accepted | Running}`.
- `AwaitingReconciliation` → `Succeeded` / `Failed` / `Cancelled`: `StatusReport` reports a matching terminal Agent-action state, adopted explicitly.
- `AwaitingReconciliation` → `Indeterminate`: an explicit reconciliation decision closes the Attempt as indeterminate — typically because `StatusReport{known_state: Unknown}` was received and no further evidence is expected, or because no Agent session re-establishes within an implementation-defined window and the decision is made to stop waiting.
  - For a **destructive** JobStep: this transition, and any subsequent Attempt, require an **explicit recorded operator decision**. The same operator decision both closes the prior Attempt as `Indeterminate` and separately determines whether a new Attempt is authorized. `Unknown` never automatically creates a new Attempt for a destructive JobStep.
  - For a **non-destructive** JobStep: a new Attempt after `Indeterminate` is permitted only when that JobStep's own retry policy explicitly supports retrying from an indeterminate outcome. Being non-destructive does not by itself imply duplicate execution is safe.

On Server restart, every Attempt persisted as `Dispatched` or `InProgress` is loaded as `AwaitingReconciliation`, and reconciliation proceeds as above once the relevant Agent session re-establishes (ADR-0004/ADR-0005 handshake). This is never a blind resume: no Attempt leaves `AwaitingReconciliation` without an explicit `StatusReport`-driven transition or, for destructive steps, an explicit operator decision.

### Job admission

Checked once per Job, at `Pending` → `Running`:

1. the target Endpoint's exclusivity lease is available and is granted to this Job.

This is distinct from, and prior to, the per-Attempt "Workflow/scheduler authorization" and "Destructive dispatch preconditions" above, which are evaluated at every dispatch throughout the Job's active lifetime, not just at admission.

### Retry policy (fulfills ADR-0005's deferred policy question)

- **Destructive JobSteps**: no automatic retry, under any circumstance, for any Attempt outcome (`Failed`, `Rejected`, or `Indeterminate`). A further Attempt requires an explicit, recorded operator decision — the same decision that closes a prior uncertain Attempt as `Indeterminate` (see "Reconciliation and the Indeterminate outcome"). This is a hard invariant, not a tunable policy (`AGENTS.md`; architecture-redesign.md "Durable workflow"; Issue #4 safety constraints).
- **Non-destructive JobSteps**: an automatic, bounded retry (a fresh Attempt) is permitted after `Failed`, and after `Indeterminate` only when that JobStep's retry policy explicitly supports retrying from an indeterminate outcome — non-destructive alone is not sufficient justification, since some non-destructive JobSteps may still have side effects that make a blind retry unsafe. Exact retry counts, backoff, and per-JobStep-type opt-in are implementation-time tuning, not decided here.
- A `Rejected` Attempt is never treated as `Failed` for retry-policy purposes without explicit consideration — a protocol-level rejection often indicates a version/compatibility problem an automatic retry with the same parameters will not fix.

### Job cancellation

- `Pending` → `Cancelled`: cancellation requested before any JobStep has begun — nothing active to await. Any Endpoint-exclusivity lease already granted is released as part of this transition.
- `Running` → `Cancelling`: cancellation requested while the Job is active.
  - If a JobStep currently has an active Attempt (`Dispatched`/`InProgress`/`AwaitingReconciliation`), `CancelAction` is sent for it.
  - If no JobStep currently has an active Attempt (e.g., a JobStep is `Pending`/`PreconditionsSatisfied` awaiting Attempt-scoped leases, or in the gap between retries), no `CancelAction` is sent — there is nothing dispatched to cancel.
- While `Cancelling`: no new JobStep or Attempt may begin.
  - `CancelAck{outcome: Cancelled}`: the Attempt (and its JobStep) becomes `Cancelled`; the Job proceeds to `Cancelled`.
  - `CancelAck{outcome: AlreadyCompleted}`: proves only that the Agent considers the action already terminal — it does **not** by itself reveal whether the Attempt `Succeeded`, `Failed`, or was `Cancelled`. If the Server already holds the Attempt's authoritative terminal outcome (from an earlier `ActionResult`), that outcome stands unchanged. If the Server does not yet know the terminal outcome, the Attempt moves to `AwaitingReconciliation` and the Server issues `StatusQuery` to learn it — the outcome is never assumed from `AlreadyCompleted` alone. The Job remains `Cancelling` until the Attempt's actual terminal state is known or reconciliation explicitly resolves it (including, if necessary, to `Indeterminate`).
  - `CancelAck{outcome: CannotCancel}`: must **not** by itself produce Job `Cancelled`. The Attempt continues to whatever terminal outcome it actually reaches (`Succeeded`/`Failed`); only once that real outcome is known does the Job proceed to `Cancelled` — no further JobStep begins, even though this JobStep itself ran to completion.
  - `CancelAck{outcome: Unknown}`: requires reconciliation. The Attempt moves through `AwaitingReconciliation` → (`Indeterminate` via explicit decision, or a resolved terminal state) exactly as in the non-cancellation case; the Job remains `Cancelling` until that resolves.
  - No active Attempt existed when cancellation was requested: nothing to await — proceed directly to `Cancelling` → `Cancelled` (below), since no execution is active or uncertain.
- `Cancelling` → `Cancelled`: reached once the workflow is known to have stopped — the active Attempt (if any) has reached `Succeeded`, `Failed`, `Cancelled`, `Rejected`, or `Indeterminate` with no further Attempt authorized, or there was no active Attempt to begin with — and no active or uncertain execution remains. The Job's Endpoint-exclusivity lease is released at this point.

## Alternatives considered

- **Two-tier model (JobStep only, no Attempt)**: rejected — would require encoding multiple dispatch attempts as ad hoc fields on JobStep itself, duplicating what a first-class `Attempt` entity already expresses cleanly, and `Attempt` is already a named Discovery domain concept this ADR is obligated to give meaning to.
- **DAG/branching JobStep graph**: rejected for M0 — no current requirement or evidence describes branching or parallel JobSteps within one Job; a linear sequence is the smallest structure that satisfies "each relevant provisioning stage is a JobStep."
- **Single fixed global concurrency limit**: rejected — explicitly excluded by architecture-redesign.md.
- **Attempt/JobStep-scoped endpoint exclusivity (the initially accepted design)**: superseded on owner review — did not match the actual M0 requirement (no interleaving of two active Jobs against one Endpoint) and left a narrower safety margin than Job-scoped exclusivity, which is both simpler and strictly safer for that requirement.
- **Treating `StatusReport{Unknown}` as an immediate, automatic terminal outcome (originally, remaining in `AwaitingReconciliation` indefinitely with implicit retry eligibility)**: rejected on owner review — collapsed a case requiring explicit judgment into an ambiguous default; `Indeterminate` makes the "we do not know" outcome a first-class, explicit, terminal record instead.
- **Immediate Job `Cancelled` on cancellation request**: rejected on owner review — a cancellation request is not proof execution stopped; `Cancelling` makes the uncertainty window explicit and durable rather than assuming success.
- **Automatic retry for destructive JobSteps with a "confirm before executing" safeguard**: considered and rejected as a design pattern for M0 — the repository's safety policy is unconditional on this point, and introducing any automatic-retry code path for destructive steps, even a gated one, was judged more likely to leak dangerous behavior later than not having the code path at all.

## Consequences

- Issue #5 (persistence, observability, and domain-event model) must persist Job (including `Cancelling`), JobStep, and Attempt (including `Indeterminate`) durably enough to survive restart and support the reconciliation procedure above; this ADR does not choose the persistence technology.
- Issue #5's domain-event model will likely want an event for "Attempt became Indeterminate" as an operator-notification source, and will likely reference other Job/JobStep/Attempt transitions as event sources; this ADR does not define specific domain events.
- Issue #6 (data-plane and storage contracts) JobSteps that perform transfers must fit within this Attempt model, including revalidation immediately before dispatch; this ADR does not define transfer-specific preconditions or postconditions.
- Issue #7 (Simulator) must be able to simulate `AwaitingReconciliation`, `Indeterminate` resolution, `Cancelling`, and Job-scoped exclusivity contention as first-class scenarios, not edge cases.
- The Scheduler's admission logic (Job `Pending` → `Running`) owns endpoint-exclusivity arbitration exclusively; JobStep/Attempt-level dispatch logic no longer acquires or releases it. A Job that cannot obtain its Endpoint's exclusivity lease queues in `Pending`; fairness/ordering among queued Jobs contending for the same Endpoint remains implementation-time, not decided here.
- Any future partial-failure/skip semantics for Jobs, or a branching JobStep structure, requires a new ADR or Specification update rather than silent implementation-time extension.

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Durable workflow", "Capacity and scheduling", "Proposed component boundaries" (`Attempt`).
- `docs/discovery/adr-triage.md` — candidates 7, 11.
- ADR-0004 — Endpoint identity (destructive-operation authorization preconditions; this ADR defines precondition 3, "authorized Job/action").
- ADR-0005 — Agent control-plane protocol (Agent-action state vocabulary consumed by Attempt; retry mechanism this ADR's policy governs; `StatusQuery`/`StatusReport` this ADR's reconciliation procedure uses).
- ADR-0007 — Persistence backend and durable/transient boundary (defines the persist-before-send ordering that this ADR's `Dispatched` state's durable-commit semantics rely on).
- `docs/specifications/m0-job-lifecycle-and-scheduling.md` — detailed state tables and validation expectations.

## Related work

- Issue #4 — `[WP] Define Job lifecycle and scheduling model`.
- Issue #2 / ADR-0004 — Endpoint identity and destructive-operation preconditions.
- Issue #3 / ADR-0005 — Agent Protocol v1 (action states, retry mechanism, `StatusQuery`).
- Issue #5 / ADR-0007 — persist-before-send dispatch ordering this ADR's `Dispatched` semantics rely on.
- Issue #5 — `[WP] Define persistence, observability, and domain-event model` (durability of Job/JobStep/Attempt; domain events).
- Issue #6 — `[WP] Define data-plane and storage contracts` (transfer JobSteps fit this model).
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (must simulate reconciliation and cancellation scenarios).
