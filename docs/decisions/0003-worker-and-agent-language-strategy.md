# ADR-0003: Worker and Agent implementation language strategy

Status: Accepted

## Context

With ADR-0002 fixing Rust for the Bamep Server, M0 also requires evaluating the implementation language for:

- the **Worker** boundary established in ADR-0001 (transfer, compression, verification, artifact-movement workloads, release-coupled to the Server per that ADR);
- the **Agent** (the endpoint-resident supervisor running inside the Alpine maintenance/live environment, executing typed actions only — no arbitrary `sh -c`, per `docs/discovery/architecture-redesign.md` "Backend and Agent").

Issue #1 records explicit owner direction that this decision is intentionally left open: "The Worker language is intentionally unresolved. Rust and Go are the primary candidates... Do not select Go solely as a learning opportunity" and "Do not assume the Agent must use the same language as the Server without evaluating its own constraints."

This ADR records the evaluation performed against that instruction and the owner's resulting decision.

## Evaluation

**Worker** (compression, transfer, verification, artifact movement; release-coupled to the Server per ADR-0001):

- *Rust*: shares a single toolchain, CI, and release pipeline with the Server; no GC pauses during large transfers/compression; static binaries; because Workers ship with the Server release rather than independently, a shared build pipeline has direct operational value beyond code reuse.
- *Go*: simpler goroutine-based concurrency, fast compile times, straightforward static-binary deployment; but introduces a second toolchain, dependency ecosystem, and CI/release pipeline for artifacts that are release-coupled to a Rust Server anyway.

**Agent** (endpoint-resident supervisor inside the Alpine/musl live environment; typed actions, authentication, state machine, retries, cancellation, process supervision):

- *Rust*: compiles to static musl-compatible binaries well suited to an Alpine live environment; no bundled GC runtime; matches the "supervisor with typed actions" direction already accepted in Discovery.
- *Go*: also cross-compiles to static binaries and is historically simple for daemon/supervisor-style programs; includes a GC runtime, a small but non-zero footprint cost in a RAM-constrained diskless-boot environment (`docs/reference/poc-lessons.md` records storage/RAM pressure already observed during the previous PoC's diskless Alpine boot, though under different conditions).

**Cross-cutting factor**: `docs/discovery/architecture-redesign.md` explicitly frames the language question in terms of "the cost of operating a polyglot stack as a primarily solo-maintained project." A single language across Server, Worker, and Agent minimizes the number of toolchains, dependency ecosystems, and CI/release pipelines one maintainer must operate, and maximizes direct sharing of protocol/contract types between components without an additional schema-generation step.

## Decision

Rust is accepted as the implementation language for both the Worker and the Agent, for consistency with the Server (ADR-0002), the solo-maintainer cost argument above, and because no identified requirement specifically favors Go's concurrency model or footprint for either workload.

**Contract independence is a required constraint of this decision, not an incidental detail.** Using Rust across Server, Worker, and Agent must not make shared Rust types or crates the sole definition of any inter-process or wire contract. The Agent Protocol, the Administrative API, and any other externally relevant contract must remain explicit and independently versioned, as already required by `docs/discovery/architecture-redesign.md` ("Any Agent Protocol must define correlation, acknowledgement, duplicate handling, timeout, reconnect, cancellation, progress, protocol version, and idempotency semantics") and by the packaging baseline's "contracts versioned separately" direction (`docs/specifications/m0-stack-and-boundaries-baseline.md`).

Sharing implementation types or generated representations between Server, Worker, and Agent is allowed where useful (for example, generating Rust bindings from a schema, or sharing an internal crate between same-language components), but the architecture must preserve the ability to implement a contract participant in another language without redefining the protocol. A single-language stack is a deployment and maintenance convenience; it must not become the load-bearing definition of a contract that the Agent Protocol or Administrative API Work Packages (Issues #2, #3) are responsible for specifying explicitly.

## Alternatives considered

- **Go for Worker and/or Agent**: viable; not recommended above, but not eliminated by a concrete blocker either — the owner may still choose it for reasons this evaluation cannot weigh (e.g., a distinct, explicitly-stated non-"learning opportunity" justification).
- **Split stack** (e.g., Rust Server + Worker, Go Agent, or the reverse): technically viable via a versioned wire contract (Agent Protocol v1) regardless of language pairing, but reintroduces the polyglot maintenance cost the owner asked to be weighed.
- **Python for Worker**: not seriously considered; weak fit for CPU-bound compression/verification, and the previous PoC's Python/FastAPI use is historical evidence only (`docs/reference/poc-lessons.md`), not a forward candidate here.

## Consequences

- Bamep becomes a single-language (Rust) stack across Server, Worker, and Agent, simplifying CI/build/release and enabling shared crates for internal implementation types where useful.
- The Agent Protocol, Administrative API, and other externally relevant contracts must still be specified explicitly and versioned independently of any shared Rust type — this ADR does not substitute for that contract work, owned by the relevant M0 Work Packages (Issues #2, #3).
- A future contract participant (e.g., a third-party integration, or a language change for one component) must remain implementable from the versioned contract alone, without needing to read Rust source.
- Worker and Agent implementation itself remains out of scope for M0; this ADR only fixes the language, not an authorization to begin implementation.

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Backend and Agent".
- ADR-0001 — Runtime topology (establishes the Worker boundary this ADR evaluates).
- ADR-0002 — Backend/Server language (Rust; the baseline this ADR evaluates consistency against).

## Related work

- Issue #1 — `[WP] Define product, runtime, and stack architecture baseline`.
- Issue #3 — `[WP] Define Agent control and action contracts` (owns the explicit, independently versioned Agent Protocol this ADR's contract-independence constraint depends on).
