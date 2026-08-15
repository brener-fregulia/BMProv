# ADR-0007: Persistence backend and durable/transient boundary

Status: Accepted

## Context

M0 requires resolving Bamep's persistence strategy for standalone deployments (`docs/discovery/adr-triage.md` candidate 6; `docs/specifications/m0-architecture-baseline.md` scope item "persistence strategy"). Issue #5 executes this Work Package.

`docs/discovery/architecture-redesign.md` "Persistence": "Durable domain state and history must be separated from runtime connection and presence state. SQLite is a strong candidate for standalone single-node deployments. PostgreSQL remains an alternative if concrete requirements for heavier write concurrency, remote database operation, HA, or multi-site emerge. The final choice remains an M0 ADR."

`docs/discovery/architecture-redesign.md` "Observability": "Correlation must make it possible to relate endpoint, job, step, attempt, action, and transfer... High-frequency telemetry does not need to be persisted indefinitely."

The owner provided explicit direction for this decision, which this ADR follows and records: SQLite is strongly preferred for the standalone/single-node profile, but the architecture must not assume every future deployment stays small — a future high-throughput profile (e.g., a large system integrator or retailer provisioning dozens or more endpoints concurrently) must remain reachable through an architectural boundary, not through building unused flexibility into M0 itself. The owner was explicit that **endpoint concurrency is not equivalent to database writer concurrency**, and that the correct evaluation is the actual durable write model — durable domain state/events/audit records separated from high-frequency logs, progress, presence, and telemetry — rather than a naive inference from endpoint count.

## Decision

### 1. SQLite is accepted as the M0 persistence backend

SQLite is accepted for standalone/single-node Bamep Server deployments in M0. No concrete blocker was found once the *actual* durable write model is evaluated (see "Durable vs. transient/high-frequency boundary" below): M0's durable writes are bounded by domain-state transitions (Job/JobStep/Attempt state changes, Endpoint identity/credential events, inventory revision changes, domain events, audit records), not by endpoint count, message count, or raw telemetry sample count. At the M0 target of 20–24 concurrent simulated endpoints, this produces a moderate, transactional write rate well within SQLite's practical throughput in WAL (write-ahead logging) mode, which supports concurrent readers alongside a serialized writer without blocking reads.

### 2. Durable vs. transient/high-frequency boundary

This is the core architectural evaluation the owner required, and the mechanism that keeps SQLite viable regardless of endpoint count:

**Durable domain data** (written to the durable database):

- Job, JobStep, and Attempt records and their state *transitions* (`docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md`) — a bounded number of writes per JobStep (precondition evaluation, dispatch, terminal outcome), not a write per Agent message.
- Endpoint identity, enrollment, credential, and hardware-confidence *state changes* (`docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md`, `docs/specifications/m0-endpoint-identity-lifecycle.md`) — written on transition, not on every observation.
- Inventory, on **revision change only** — a durable inventory revision is written when observed inventory actually differs from the last durable revision, not on every poll/report cycle. This is the single largest lever for keeping inventory-related write volume bounded as endpoint count grows.
- Artifact/Snapshot metadata — written once per artifact lifecycle transition (created, verified, committed), not per byte or per chunk.
- Domain events (see "Domain-event model" below) — curated, coarse-grained, state-transition-level events, not a firehose of raw activity.
- Audit records for safety-relevant decisions — operator approvals (endpoint enrollment, reconciliation/`Indeterminate` decisions, destructive-JobStep retry authorization, cancellation), destructive-operation dispatch and outcome.

**Transient / high-frequency data** (not written as one durable database row per message or sample):

- Agent runtime connection/presence state (connected/disconnected, session liveness) — ephemeral runtime state, explicitly separated from durable domain state per architecture-redesign.md "Persistence."
- `ActionProgress` messages (percent/bytes/ETA ticks during a long-running Attempt, `docs/specifications/m0-agent-protocol-contract.md`) — high-frequency; kept as the latest observed value (in memory, or a single mutable/overwritten record at most), never one durable insert per progress tick. Only the Attempt's final terminal outcome is durably persisted.
- General application/structured logs — not domain data; standard log output, not the domain database.
- High-frequency telemetry/metrics (throughput, resource usage samples) — architecture-redesign.md "Observability" already establishes these do not need indefinite persistence; if retained at all, retention/aggregation strategy is implementation-time detail, not decided here, but the invariant is fixed: never one durable write per sample.

This separation is what makes the SQLite decision hold at scale: durable write volume tracks the number of *meaningful domain-state transitions*, which grows far more slowly than endpoint count × message frequency.

### 3. Repository port/adapter boundary preserved for a future PostgreSQL adapter

The Domain and Application layers depend only on repository port interfaces (`docs/specifications/m0-stack-and-boundaries-baseline.md` already names `repositories` as a Port). SQLite is the M0 Adapter implementation behind that port. A future PostgreSQL adapter would implement the same port interfaces without requiring changes to Domain or Application code.

This boundary is triggered by concrete needs, not spent speculatively in M0. Legitimate future triggers include:

- substantially higher concurrent-write pressure than M0's actual durable write model (not merely a higher endpoint count within a similar write-per-transition profile);
- remote database operation;
- multiple Server processes/instances sharing one database;
- HA;
- multi-site.

### 4. No dual-backend support in M0

M0 implements only the SQLite adapter. Dual SQLite/PostgreSQL support is explicitly not built now, per direct owner instruction — building it for a hypothetical future need would add cost (two adapters to write, test, and maintain) without present benefit, and M0's own acceptance criteria do not require it.

### 5. The domain model is not constrained to a SQL lowest common denominator

Domain and Application code is designed correctly for its actual requirements; it is not artificially restricted to whatever SQL feature subset both SQLite and a hypothetical future PostgreSQL adapter would share. Portability is the repository port's responsibility, not a constraint the domain model carries in advance. If a future PostgreSQL adapter requires different queries or storage mechanics behind the same port interface, that is the adapter's concern.

## Alternatives considered

- **PostgreSQL for M0 by default**: rejected — would add operational complexity (a separate database service, network configuration, credential management) to a standalone single-node product whose actual M0-scale durable write pattern, once the durable/transient boundary above is applied, does not require it. Remains available later through the repository-port adapter boundary once a concrete trigger emerges.
- **Choosing PostgreSQL merely because M0 targets 20–24 simulated endpoints**: explicitly rejected per owner direction — endpoint concurrency is not equivalent to database writer concurrency; the relevant variable is the durable write model established in this ADR, not raw endpoint count.
- **Dual SQLite/PostgreSQL support in M0**: rejected — explicit owner instruction; defers real cost to validate a hypothetical need instead of building the adapter boundary that defers it properly.
- **Treating high-frequency telemetry, progress, and presence as durable per-message/per-sample writes**: rejected — would make write volume scale with message count rather than domain-event count, undermining the durable write model this ADR relies on, regardless of which backend were chosen.
- **Constraining the domain model to a lowest-common-denominator SQL subset now**: rejected per explicit owner instruction — the repository port, not domain-model compromise, is the correct location for future backend portability.

## Consequences

- Every M0 Work Package that persists state (already-accepted: WP2 identity, WP4 Job/JobStep/Attempt; forward: WP6 data-plane/artifacts) must respect the durable-vs-transient boundary established here — state *transitions* are durable; high-frequency observation is not, by default.
- Inventory persistence specifically must implement write-on-revision-change, not write-on-poll; this is a concrete, testable requirement for whichever Work Package implements inventory collection.
- `ActionProgress` (ADR-0005) must not be wired to a durable-database insert per message in any future implementation; only `ActionResult` (or an explicitly defined checkpoint policy, not decided here) is durably persisted.
- The persistence Adapter must be built behind the existing `repositories` Port (`docs/specifications/m0-stack-and-boundaries-baseline.md`); Domain/Application code must not reference SQLite directly.
- Issue #7 (Simulator) should eventually exercise representative persistence load at the M0 20–24 concurrent-endpoint target, so that any future backend change is evidence-driven rather than speculative (see `docs/specifications/m0-persistence-observability-and-domain-events.md` "Validation expectations").
- Revisiting this ADR requires one of the concrete triggers listed in "Repository port/adapter boundary" to actually materialize, not a general sense that Bamep "might get bigger."

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Persistence", "Observability".
- `docs/discovery/adr-triage.md` — candidate 6.
- `docs/specifications/m0-stack-and-boundaries-baseline.md` — `repositories` Port this ADR's Adapter boundary depends on.
- `docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md` — Job/JobStep/Attempt durable state this ADR persists.
- `docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md` / `docs/specifications/m0-endpoint-identity-lifecycle.md` — Endpoint identity durable state.
- `docs/specifications/m0-agent-protocol-contract.md` — `ActionProgress` as the canonical high-frequency, non-durable example.
- `docs/specifications/m0-persistence-observability-and-domain-events.md` — domain-event catalog and correlation model built on this ADR's boundary.

## Related work

- Issue #5 — `[WP] Define persistence, observability, and domain-event model`.
- Issue #2 / ADR-0004 — Endpoint identity durable state.
- Issue #3 / ADR-0005 — `ActionProgress`, the high-frequency non-durable example.
- Issue #4 / ADR-0006 — Job/JobStep/Attempt durable state this ADR persists.
- Issue #6 — `[WP] Define data-plane and storage contracts` (artifact metadata durability; must respect this ADR's boundary for transfer progress).
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (representative persistence load validation at 20–24 endpoints).
