# ADR-0003: Worker and Agent implementation language strategy

Status: Proposed

## Context

With ADR-0002 fixing Rust for the Bamep Server, M0 also requires evaluating the implementation language for:

- the **Worker** boundary established in ADR-0001 (transfer, compression, verification, artifact-movement workloads, release-coupled to the Server per that ADR);
- the **Agent** (the endpoint-resident supervisor running inside the Alpine maintenance/live environment, executing typed actions only — no arbitrary `sh -c`, per `docs/discovery/architecture-redesign.md` "Backend and Agent").

Issue #1 records explicit owner direction that this decision is intentionally left open: "The Worker language is intentionally unresolved. Rust and Go are the primary candidates... Do not select Go solely as a learning opportunity" and "Do not assume the Agent must use the same language as the Server without evaluating its own constraints."

This ADR is **not** an acceptance of a final decision. It records the evaluation and a recommendation, and is submitted as `Proposed` per `docs/development/sdd.md`'s owner-approval requirement for architectural decisions with meaningful alternatives.

## Evaluation

**Worker** (compression, transfer, verification, artifact movement; release-coupled to the Server per ADR-0001):

- *Rust*: shares a single toolchain, CI, and release pipeline with the Server; no GC pauses during large transfers/compression; static binaries; because Workers ship with the Server release rather than independently, a shared build pipeline has direct operational value beyond code reuse.
- *Go*: simpler goroutine-based concurrency, fast compile times, straightforward static-binary deployment; but introduces a second toolchain, dependency ecosystem, and CI/release pipeline for artifacts that are release-coupled to a Rust Server anyway.

**Agent** (endpoint-resident supervisor inside the Alpine/musl live environment; typed actions, authentication, state machine, retries, cancellation, process supervision):

- *Rust*: compiles to static musl-compatible binaries well suited to an Alpine live environment; no bundled GC runtime; matches the "supervisor with typed actions" direction already accepted in Discovery.
- *Go*: also cross-compiles to static binaries and is historically simple for daemon/supervisor-style programs; includes a GC runtime, a small but non-zero footprint cost in a RAM-constrained diskless-boot environment (`docs/reference/poc-lessons.md` records storage/RAM pressure already observed during the previous PoC's diskless Alpine boot, though under different conditions).

**Cross-cutting factor**: `docs/discovery/architecture-redesign.md` explicitly frames the language question in terms of "the cost of operating a polyglot stack as a primarily solo-maintained project." A single language across Server, Worker, and Agent minimizes the number of toolchains, dependency ecosystems, and CI/release pipelines one maintainer must operate, and maximizes direct sharing of protocol/contract types between components without an additional schema-generation step.

## Recommendation (not accepted)

Rust for both Worker and Agent, for consistency with the Server (ADR-0002), the solo-maintainer cost argument above, and no identified requirement that specifically favors Go's concurrency model or footprint for either workload.

This recommendation is not finalized in this ADR: it is a decision with meaningful alternatives, and per `docs/development/sdd.md` "Owner approval," accepting a significant architectural decision requires explicit owner approval rather than being inferred by the executing session.

## Alternatives considered

- **Go for Worker and/or Agent**: viable; not recommended above, but not eliminated by a concrete blocker either — the owner may still choose it for reasons this evaluation cannot weigh (e.g., a distinct, explicitly-stated non-"learning opportunity" justification).
- **Split stack** (e.g., Rust Server + Worker, Go Agent, or the reverse): technically viable via a versioned wire contract (Agent Protocol v1) regardless of language pairing, but reintroduces the polyglot maintenance cost the owner asked to be weighed.
- **Python for Worker**: not seriously considered; weak fit for CPU-bound compression/verification, and the previous PoC's Python/FastAPI use is historical evidence only (`docs/reference/poc-lessons.md`), not a forward candidate here.

## Consequences

- If accepted as recommended, Bamep becomes a single-language (Rust) stack across Server, Worker, and Agent, simplifying CI/build/release and maximizing shared crates for protocol/contract types.
- If the owner instead splits the stack, this ADR must be updated or superseded with the actual accepted decision, and the polyglot-cost trade-off explicitly re-examined rather than silently dropped.
- Until this ADR is `Accepted`, Worker and Agent implementation must not begin, since their language is not yet a durable decision.

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Backend and Agent".
- ADR-0001 — Runtime topology (establishes the Worker boundary this ADR evaluates).
- ADR-0002 — Backend/Server language (Rust; the baseline this ADR evaluates consistency against).

## Related work

- Issue #1 — `[WP] Define product, runtime, and stack architecture baseline`. This ADR being `Proposed` rather than `Accepted` is the explicit isolation of an unresolved question required by that Work Package's acceptance criteria.
