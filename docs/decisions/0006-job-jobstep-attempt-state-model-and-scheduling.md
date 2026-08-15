# ADR-0006: Job/JobStep/Attempt state model and resource-lease scheduling

Status: Accepted

## Context

M0 requires resolving the durable Job/JobStep state model and the scheduler/resource-lease model (`docs/discovery/adr-triage.md` candidates 7, 11; `docs/specifications/m0-architecture-baseline.md` scope items "Job/JobStep lifecycle", "scheduler and resource model"). Issue #4 executes this Work Package.

`docs/discovery/architecture-redesign.md` "Durable workflow": "Each relevant provisioning stage is a JobStep with preconditions, execution state, result, postconditions, retry semantics, and cancellation semantics. After power loss or reconnect, the Server must reconcile actual endpoint state with durable workflow state. Destructive operations must never be automatically retried merely because a generic retry policy exists."

`docs/discovery/architecture-redesign.md` "Capacity and scheduling": concurrency must not be a single fixed global number; JobSteps compete for resource leases/tokens representing endpoint exclusivity, network capacity, storage read/write capacity, CPU/worker capacity, and other constrained resources.

`docs/discovery/architecture-redesign.md` "Proposed component boundaries" already names `Attempt` as a distinct Domain concept alongside `Job` and `JobStep`; this ADR is the first to give it concrete meaning.

This ADR also resolves two questions explicitly deferred to Issue #4 by earlier M0 ADRs: what "authorized Job/action" means for ADR-0004's destructive-operation authorization precondition 3, and the retry *policy* that ADR-0005 left open (ADR-0005 defines only the retry *mechanism* — a fresh `action_id` with `retry_of`).

## Decision

### Three-tier domain model: Job, JobStep, Attempt

- **Job**: one overall workflow targeting a single Endpoint (e.g., a provisioning or recovery workflow), composed of an ordered sequence of JobSteps. M0 defines a linear sequence, not a DAG — no current requirement or evidence describes branching or parallel JobSteps within one Job.
- **JobStep**: one stage of the Job's workflow (e.g., "write image", "verify backup"). Carries preconditions, results, postconditions, and cancellation semantics. A JobStep may be attempted more than once; each attempt is a separate `Attempt`.
- **Attempt**: one execution attempt of a JobStep, corresponding 1:1 to one Agent Protocol `action_id` lifecycle (ADR-0005). A retry creates a new Attempt with a fresh `action_id` and `retry_of` referencing the prior Attempt's `action_id`. The Attempt consumes the Agent Protocol's per-action states (`Accepted`/`Running`/`Succeeded`/`Failed`/`Cancelled`/`Unknown`) and translates them into this Server-side durable record — the Attempt's own state is the Server's authoritative interpretation, not a re-export of the Agent's local vocabulary.

### Job states

`Pending → Running → {Succeeded | Failed | Cancelled}`

- `Pending`: created, no JobStep has begun.
- `Running`: iterating through its ordered JobSteps.
- `Succeeded`: all JobSteps succeeded.
- `Failed`: a JobStep reached `Failed` and no partial-failure/skip policy applied. M0 does not define partial-failure tolerance — a single JobStep failure fails the Job. Whether finer-grained partial-failure semantics are needed is an explicit open question, not decided here.
- `Cancelled`: explicit cancellation (operator-initiated or a defined system trigger), propagated to the currently active JobStep/Attempt via `CancelAction`.

### JobStep states

`Pending → PreconditionsSatisfied → Dispatching → {Succeeded | Failed | Cancelled}`

- `Pending`: created, preconditions not yet evaluated.
- `PreconditionsSatisfied`: preconditions hold at evaluation time — including, for a destructive JobStep, the full destructive-operation authorization precondition set from `docs/specifications/m0-endpoint-identity-lifecycle.md`. Eligible for resource-lease acquisition and dispatch.
- `Dispatching`: at least one Attempt has been made; persists across retries. The JobStep does not reach a terminal state merely because one Attempt failed, if retry policy (below) permits another Attempt.
- `Succeeded` / `Failed` / `Cancelled`: terminal, set once an Attempt succeeds, once retry policy determines no further Attempt will be made, or once cancellation completes.

### Attempt states

`Dispatched → InProgress → {Succeeded | Failed | Cancelled | Rejected}`, with `AwaitingReconciliation` reachable from `Dispatched` or `InProgress`.

- `Dispatched`: `ActionDispatch` sent; awaiting `ActionAck`.
- `InProgress`: `ActionAck{outcome: Accepted}` received.
- `Rejected`: `ActionAck{outcome: Rejected}` received — the Attempt never executed. Kept distinct from `Failed` for diagnostic and retry-decision purposes, and because ADR-0005 already requires the Agent Protocol layer to never conflate the two.
- `Succeeded` / `Failed` / `Cancelled`: terminal, populated from `ActionResult` (`Succeeded`/`Failed`) or `CancelAck{Cancelled}`.
- `AwaitingReconciliation`: the Attempt's true outcome cannot currently be determined — reached on `ActionAck` timeout (an uncertain delivery outcome per ADR-0005, not proof of non-execution), on connection loss while `InProgress`, or on Server restart with an Attempt that was `Dispatched`/`InProgress` at the time of the crash. Resolved only by an explicit `StatusQuery`/`StatusReport` exchange (see "Reconciliation" below) — never by assumption.

### Resource leases and scheduling

The Scheduler grants **resource leases** to a JobStep's Attempt before dispatch. Lease types are extensible, not a closed enum; M0 requires at minimum: endpoint exclusivity, network capacity, storage read/write capacity, and CPU/worker capacity. Concurrency is governed by lease availability, never a single fixed global number.

- A lease is acquired when a JobStep enters `PreconditionsSatisfied` and is about to be dispatched, and released when its Attempt reaches a terminal state (`Succeeded`/`Failed`/`Cancelled`/`Rejected`).
- **Endpoint-exclusivity leases are the one type this ADR constrains precisely, for safety**: retained through `AwaitingReconciliation`, never released early, so that no second JobStep can be dispatched against the same Endpoint while an earlier Attempt's true outcome is unknown. This directly supports the "authorized Job/action" precondition below.
- Retention/release policy for other lease types (network, storage, CPU/worker) during `AwaitingReconciliation` is not decided here — it does not carry the same safety implication as endpoint exclusivity and is left as implementation-time policy.
- Lease acquisition ordering, fairness, and priority across competing JobSteps are not decided here — M0 requires the lease-competition mechanism to exist, not a specific scheduling algorithm.

### "Authorized Job/action" (satisfies ADR-0004's deferred precondition 3)

A Job/action dispatch is authorized when, at dispatch time:

1. the Job is `Running` and the JobStep is the Job's current active step (not stale, not already terminal, not a step not yet reached);
2. the JobStep is `PreconditionsSatisfied`, or is creating a new Attempt permitted by retry policy (below);
3. no other Attempt for the same Endpoint is currently in `AwaitingReconciliation` (the endpoint-exclusivity lease is held and uncontested);
4. all required resource leases for this Attempt are currently held.

### Retry policy (fulfills ADR-0005's deferred policy question)

- **Destructive JobSteps**: no automatic retry, under any circumstance, for any Attempt outcome (`Failed`, `Rejected`, or an `AwaitingReconciliation` that resolves to `Unknown`). A further Attempt requires an explicit, recorded operator decision. This is a hard invariant, not a tunable policy (`AGENTS.md`; architecture-redesign.md "Durable workflow"; Issue #4 safety constraints).
- **Non-destructive JobSteps**: an automatic, bounded retry (a fresh Attempt) is permitted after `Failed` or an `AwaitingReconciliation` that resolves to `Unknown`, at implementation's discretion. Exact retry counts, backoff, and which non-destructive JobStep types opt in are implementation-time tuning, not decided here. This ADR establishes only that automatic retry is *permitted* for non-destructive steps and *never permitted* for destructive ones — the safety-relevant boundary.
- A `Rejected` Attempt is never treated as `Failed` for retry-policy purposes without explicit consideration — a protocol-level rejection (e.g., unknown `action_type`) often indicates a version/compatibility problem an automatic retry with the same parameters will not fix.

### Reconciliation after restart or reconnect

On Server restart, every Attempt persisted as `Dispatched` or `InProgress` is loaded as `AwaitingReconciliation`. Once the relevant Agent session is re-established (ADR-0004/ADR-0005 handshake), the Server issues `StatusQuery` for each such Attempt's `action_id`:

- `StatusReport` reports a terminal Agent-action state (`Succeeded`/`Failed`/`Cancelled`) the Server can adopt: the Attempt (and, per retry policy, the JobStep/Job) transitions to the corresponding terminal state.
- `StatusReport` reports `Running`: the Attempt returns to `InProgress`.
- `StatusReport` reports `Unknown`, or no Agent session re-establishes within an implementation-defined window: the Attempt remains in `AwaitingReconciliation`; resolution follows the retry policy above (operator-only for destructive JobSteps).

This is never a blind resume: no Attempt leaves `AwaitingReconciliation` without an explicit `StatusReport` (or, for destructive steps, an explicit operator decision) — satisfying the safety constraint that reconciliation "must not blindly resume or replay destructive steps."

## Alternatives considered

- **Two-tier model (JobStep only, no Attempt)**: rejected — would require encoding multiple dispatch attempts as ad hoc fields on JobStep itself, duplicating what a first-class `Attempt` entity already expresses cleanly, and `Attempt` is already a named Discovery domain concept this ADR is obligated to give meaning to.
- **DAG/branching JobStep graph**: rejected for M0 — no current requirement or evidence describes branching or parallel JobSteps within one Job; a linear sequence is the smallest structure that satisfies "each relevant provisioning stage is a JobStep."
- **Single fixed global concurrency limit**: rejected — explicitly excluded by architecture-redesign.md.
- **Releasing all lease types (including endpoint exclusivity) immediately on connection loss**: rejected — would allow a second JobStep to be dispatched against an Endpoint whose true state is unknown, undermining the destructive-operation preconditions (ADR-0004).
- **Automatic retry for destructive JobSteps with a "confirm before executing" safeguard**: considered and rejected as a design pattern for M0 — the repository's safety policy is unconditional on this point, and introducing any automatic-retry code path for destructive steps, even a gated one, was judged more likely to leak dangerous behavior later than not having the code path at all.

## Consequences

- Issue #5 (persistence, observability, and domain-event model) must persist Job, JobStep, and Attempt durably enough to survive restart and support the reconciliation procedure above; this ADR does not choose the persistence technology.
- Issue #5's domain-event model will likely reference Job/JobStep/Attempt state transitions as event sources; this ADR does not define specific domain events.
- Issue #6 (data-plane and storage contracts) JobSteps that perform transfers must fit within this Attempt model; this ADR does not define transfer-specific preconditions or postconditions.
- Issue #7 (Simulator) must be able to simulate `AwaitingReconciliation` scenarios (disconnect mid-Attempt, Server restart with in-flight Attempts, `Unknown` `StatusReport`) as first-class scenarios, not edge cases.
- Any future partial-failure/skip semantics for Jobs, or a branching JobStep structure, requires a new ADR or Specification update rather than silent implementation-time extension.

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Durable workflow", "Capacity and scheduling", "Proposed component boundaries" (`Attempt`).
- `docs/discovery/adr-triage.md` — candidates 7, 11.
- ADR-0004 — Endpoint identity (destructive-operation authorization preconditions; this ADR defines precondition 3, "authorized Job/action").
- ADR-0005 — Agent control-plane protocol (Agent-action state vocabulary consumed by Attempt; retry mechanism this ADR's policy governs; `StatusQuery`/`StatusReport` this ADR's reconciliation procedure uses).
- `docs/specifications/m0-job-lifecycle-and-scheduling.md` — detailed state tables and validation expectations.

## Related work

- Issue #4 — `[WP] Define Job lifecycle and scheduling model`.
- Issue #2 / ADR-0004 — Endpoint identity and destructive-operation preconditions.
- Issue #3 / ADR-0005 — Agent Protocol v1 (action states, retry mechanism, `StatusQuery`).
- Issue #5 — `[WP] Define persistence, observability, and domain-event model` (durability of Job/JobStep/Attempt; domain events).
- Issue #6 — `[WP] Define data-plane and storage contracts` (transfer JobSteps fit this model).
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (must simulate reconciliation scenarios).
