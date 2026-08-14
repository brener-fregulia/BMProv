# ADR-0002: Bamep Server/backend implementation language — Rust

Status: Accepted

## Context

M0 requires a durable choice of implementation language for the Bamep Server/backend (`docs/specifications/m0-architecture-baseline.md` scope item "Backend/Agent stack"; `docs/discovery/adr-triage.md` candidate 5).

Issue #1 (`[WP] Define product, runtime, and stack architecture baseline`) records explicit owner direction: "Rust is the preferred language for the Bamep Server/backend and should be treated as the default candidate unless Discovery identifies a concrete architectural blocker." This ADR records the evaluation performed against that instruction and the resulting decision.

The previous FORGE PoC used Python/FastAPI. That is historical evidence of a prior implementation choice, not a Bamep requirement or constraint (`docs/reference/poc-lessons.md`: "Their previous use is evidence that they existed in the PoC, not justification for selecting or rejecting them in Bamep without current requirements and analysis").

## Decision

Rust is accepted as the implementation language for the Bamep Server/backend.

No concrete architectural blocker was identified during this evaluation for building an HTTP/typed-protocol control-plane server, SQLite- or PostgreSQL-backed persistence (ADR pending, see the persistence Work Package), and a scheduler/resource-lease model in Rust. Rust's async ecosystem, static-binary deployment, and strong memory/thread-safety guarantees fit a program that authenticates endpoints, coordinates concurrent Jobs, and gates destructive operations.

## Alternatives considered

- **Go**: strong concurrency primitives, fast compilation, straightforward static-binary deployment. Not selected as the default: the owner explicitly excluded "selecting Go solely as a learning opportunity" as a valid justification, and no concrete Server-specific requirement in `docs/discovery/architecture-redesign.md` or `docs/specifications/m0-architecture-baseline.md` favors Go's concurrency model over Rust's for the control-plane workload.
- **Python**: the previous PoC's language. Rejected as a forward decision for the Server: dynamic typing and interpreter-level concurrency limits are a weaker fit for a systems/orchestration program executing destructive operations, and the previous PoC's own evidence flagged "blocking or heavy work capable of starving the control plane" as a problem pattern (`docs/reference/poc-lessons.md`).
- **Rust**: accepted. No blocker identified; matches the owner's stated default.

## Consequences

- Rust becomes the default implementation language for Bamep Server.
- Raises the contribution bar for anyone unfamiliar with Rust; mitigated by the project currently having one primary maintainer (`docs/discovery/architecture-redesign.md` "Backend and Agent").
- Does not by itself decide the Worker or Agent language — see ADR-0003. Do not assume Rust for those merely because the Server uses it.
- If a concrete architectural blocker is later discovered (e.g., during the driver-provider or WinPE Technical Spikes), this ADR must be revisited through the normal SDD process rather than silently overridden during implementation.

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Backend and Agent".
- `docs/discovery/adr-triage.md` — candidate 5.

## Related work

- Issue #1 — `[WP] Define product, runtime, and stack architecture baseline`.
- ADR-0003 — Worker and Agent language strategy.
