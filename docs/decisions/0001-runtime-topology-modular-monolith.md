# ADR-0001: Runtime topology — modular monolith with worker/process isolation

Status: Accepted

## Context

M0 requires a durable choice of Bamep Server runtime topology before implementation begins (`docs/specifications/m0-architecture-baseline.md` scope item "Backend/Agent stack"; `docs/discovery/adr-triage.md` candidate 1).

`docs/discovery/architecture-redesign.md` ("Accepted runtime direction") already records the owner's direction: "The initial direction is modular monolith first, with explicit internal boundaries and process/worker isolation for heavy workloads when required. Microservices, clustering, Redis, leader election, and a distributed scheduler are not V1 requirements." This ADR formalizes that already-accepted Discovery direction as a durable architectural record, per the M0 acceptance criterion that blocking ADRs be `Accepted` before the baseline is complete.

Bamep V1 operates as a single-server deployment (already accepted, `docs/discovery/adr-triage.md` "Do not require ADRs at this stage"), has no HA requirement in V1, and is currently maintained by one primary maintainer.

## Decision

Bamep Server is built as a modular monolith: one deployable Server artifact with explicit internal boundaries between Presentation, Application, Domain, Runtime Services, Ports, and Adapters (`docs/discovery/architecture-redesign.md` "Proposed component boundaries", persisted in `docs/specifications/m0-stack-and-boundaries-baseline.md`).

Heavy or risky workloads (transfer, compression, verification, artifact movement) run in a separate Worker process/isolation boundary rather than in-process on the control-plane path. Workers initially belong to the Bamep Server release and do not receive independent versioning (already-accepted direction, architecture-redesign.md "Accepted runtime direction").

The domain must not depend on GRUB, MikroTik, `/dev/sda`, `snmpwalk`, WebSocket, SQLite, or zstd directly — those belong to Adapters (architecture-redesign.md "Proposed component boundaries").

## Alternatives considered

- **Microservices from the outset**: rejected. No current V1 requirement (independent scaling, multi-site, HA) justifies the added operational and deployment complexity for a solo-maintained, single-node product.
- **Single process with no worker isolation**: rejected. CPU-bound work (compression, verification, large transfers) could starve the control plane if it ran in-process alongside request handling — a problem pattern already observed in the previous PoC (`docs/reference/poc-lessons.md`: "blocking or heavy work capable of starving the control plane").
- **Fully distributed scheduler / clustering**: rejected as a V1 requirement; no current multi-node or HA requirement exists (`docs/discovery/adr-triage.md`).

## Consequences

- Internal module boundaries must be respected in code even though Server, Application, Domain, and Adapters ship as one deployable artifact.
- Worker isolation (process or equivalent) is required for heavy workloads; the exact isolation mechanism and Worker language remain to be finalized (ADR-0003).
- Because Workers do not receive independent versioning and ship with the Server release, their build/release pipeline should not diverge unnecessarily from the Server's.
- Revisiting this decision requires new multi-site, HA, or clustering requirements to emerge through the normal SDD process, not implementation convenience.

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Accepted runtime direction", "Proposed component boundaries".
- `docs/specifications/m0-stack-and-boundaries-baseline.md` — persists the component-boundary layering this ADR assumes.

## Related work

- Issue #1 — `[WP] Define product, runtime, and stack architecture baseline`.
- ADR-0003 — Worker and Agent language strategy (depends on the Worker isolation boundary established here).
