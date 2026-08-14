# ADR-0005: Agent control-plane protocol and typed-action model

Status: Accepted

## Context

M0 requires resolving the Agent control-plane protocol and typed-action model (`docs/discovery/adr-triage.md` candidates 3 and 4; `docs/specifications/m0-architecture-baseline.md` scope item "control-plane contract"; `docs/discovery/architecture-redesign.md` "Control plane", "Backend and Agent"). Issue #3 executes this Work Package.

`docs/discovery/architecture-redesign.md` states the protocol choice "remains open and requires an ADR," listing candidates neutrally (REST + polling, REST + long polling, WebSocket with a typed application protocol, SSE for browser events + HTTP commands) and notes Browser and Agent do not need to share the same mechanism. This ADR resolves the **Agent** side only; the Administrative API (Browser/Web ↔ Server) protocol is not decided here.

The Agent must not accept arbitrary `sh -c` execution from the Server (`AGENTS.md`: "Do not use unrestricted remote shell execution as a substitute for typed Agent actions"; architecture-redesign.md "Backend and Agent"). ADR-0004 (Endpoint identity, `Accepted`) defers the concrete Agent/Server mutual-authentication mechanism to this Work Package, and its destructive-operation precondition 2 ("authenticated current Agent session") depends on this ADR establishing `CredentialActive`. ADR-0003 (Worker/Agent language, `Accepted`) requires that this protocol remain explicit and independently versioned regardless of the Rust implementation shared across components.

## Decision

1. **Transport**: WebSocket over TLS (WSS), carrying a typed, versioned application-level message envelope.
2. **Mutual authentication handshake**: on connect, the Agent verifies the Server's TLS identity via a certificate fingerprint distributed through the same trusted boot chain that issues the enrollment credential (Boot Orchestrator, ADR-0004) — no separate PKI/CA is introduced, consistent with ADR-0004's rejection of PKI/mTLS machinery for M0. The Agent then presents its enrollment/runtime credential (per ADR-0004's redemption flow) in an `AuthRequest`; the Server validates it against the Endpoint identity/credential state model (`docs/specifications/m0-endpoint-identity-lifecycle.md`) and responds with `SessionEstablished` or an explicit `AuthError`. Successful completion of this handshake is the mechanism that establishes `CredentialActive`, referenced by ADR-0004's destructive-operation precondition 2.
3. **Message envelope**: every message carries `message_id`, `protocol_version`, `type`, `timestamp`, and a `correlation_id` where applicable (the relevant `action_id`). `protocol_version` is checked at handshake time; an incompatible version is rejected explicitly, never silently accepted or best-effort interpreted.
4. **Typed action model**: actions are a closed, versioned set of message types, each with a defined parameter schema and result shape. The Agent never accepts a generic/arbitrary command — only a fixed, versioned catalog of typed actions, each ultimately invoking a specific fixed tool available in the Alpine maintenance environment. The concrete catalog of action types accumulates as other M0 Work Packages are implemented; this ADR defines the shape every action must conform to, not a frozen catalog.
5. **Acknowledgement vs. result**: dispatch (`ActionDispatch`) is acknowledged (`ActionAck`) separately from completion (`ActionResult`), so the Server can distinguish "not delivered" from "delivered, still running" from "finished."
6. **Idempotency**: the Agent treats redelivery of the same `action_id` as the same logical action — if already completed, it responds with the stored result instead of re-executing; if in progress, it responds with current status instead of starting a second execution. `action_id` uniqueness is the idempotency mechanism; no separate idempotency key is introduced.
7. **Retry (mechanism only, not policy)**: a retry is a new `ActionDispatch` with a fresh `action_id` and an optional `retry_of` field referencing the prior `action_id`, for audit/correlation. This ADR does not decide when a retry is appropriate — especially for destructive actions, that decision belongs to the Job lifecycle Work Package (Issue #4) and must never be automatic merely because a generic retry policy exists (architecture-redesign.md "Durable workflow"; Issue #3 safety constraint).
8. **Cancellation**: `CancelAction(action_id)` is a distinct typed message; the Agent attempts graceful cancellation where the invoked tool supports interruption, and reports the actual resulting state (`Cancelled`, `AlreadyCompleted`, or `CannotCancel` — a destructive action already past its point of no return must be reported honestly, never silently marked cancelled).
9. **Progress**: long-running actions emit periodic `ActionProgress` messages correlated to `action_id` (e.g., percentage, bytes processed, ETA where meaningful). This carries progress metadata only; bulk data transfer itself is the data-plane's responsibility (Issue #6).
10. **Reconnect and stale-command handling**: a dropped connection does not imply the Agent's in-flight actions were cancelled or should be redispatched. On reconnect (after the handshake in point 2 completes again), the Server issues an explicit `StatusQuery(action_id)` for any action it still considers in-flight and acts only on the Agent's authoritative reported status. The protocol must never auto-redispatch a destructive action merely because a new session was established; whether/how to act on the reported status is Job-lifecycle policy (Issue #4).

No concrete architectural blocker was identified in this direction. Bamep's requirement for low-latency, bidirectional, server-initiated dispatch with streamed progress does not fit REST+polling or long-polling without effectively reconstructing an ad hoc push channel. WebSocket is well-supported in Rust's async ecosystem (ADR-0002, ADR-0003), and the provisioning LAN being a dedicated, controlled network (already-accepted assumption) reduces the firewall-traversal concerns that typically motivate polling-based designs on the open Internet.

## Alternatives considered

- **REST + short polling**: rejected as the default — dispatch latency is bounded by the polling interval, and frequent polling from 20+ concurrent Agents adds request overhead for a channel that must also carry progress updates.
- **REST + long polling**: reduces dispatch latency versus short polling, but progress streaming still requires either very frequent re-polling or a second channel, and does not cleanly unify with acknowledgement/progress/cancellation in one connection. Not rejected as categorically unsuitable — remains a fallback if a future environment cannot sustain persistent WebSocket connections — but not chosen as the M0 default.
- **SSE for Agent + HTTP commands**: SSE supports Server→Agent push but is one-directional; Agent-originated messages (ack, progress, result) would need a separate HTTP channel, splitting one logical conversation across two channels and complicating correlation. Discovery's SSE mention was specifically for the Browser case, not the Agent case; not adopted here.
- **Full PKI/mTLS for Server authentication**: rejected for the same reasons ADR-0004 rejected it for Endpoint identity — certificate lifecycle management (CA, issuance, rotation, revocation) is more machinery than M0's install profiles justify. Fingerprint pinning via the already-trusted boot chain achieves equivalent Server-authentication assurance with less operational overhead.

## Consequences

- Agent and Server both depend on a WebSocket-capable Rust stack (consistent with ADR-0002, ADR-0003; no new language constraint introduced).
- The Boot Orchestrator's security responsibility (established in ADR-0004) extends to distributing the Server's certificate fingerprint alongside the enrollment credential.
- Per ADR-0003's contract-independence constraint, this protocol (message types, envelope fields, versioning) must be specified explicitly in `docs/specifications/m0-agent-protocol-contract.md` and version-numbered ("Agent Protocol v1") independent of any Rust type definition — a non-Rust Agent or tooling must remain implementable from that Specification alone.
- The concrete typed-action catalog will grow incrementally as other M0 Work Packages are implemented; this ADR does not freeze it.
- Job/action authorization semantics (whether a given action is currently allowed) and destructive-step resumption policy after reconnect remain owned by Issue #4, which references this protocol's `StatusQuery`/`ActionDispatch` mechanism rather than having it redefined here.

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Control plane", "Backend and Agent".
- `docs/discovery/adr-triage.md` — candidates 3, 4.
- ADR-0002, ADR-0003 — Rust across Server/Worker/Agent; contract-independence constraint this ADR satisfies.
- ADR-0004 — Endpoint identity/credential model this handshake establishes.
- `docs/specifications/m0-stack-and-boundaries-baseline.md` — this ADR is a Runtime Services "Agent Control Gateway" responsibility.
- `docs/specifications/m0-agent-protocol-contract.md` — the concrete message-level contract ("Agent Protocol v1").

## Related work

- Issue #3 — `[WP] Define Agent control and action contracts`.
- Issue #2 — `[WP] Define endpoint identity and trust model` (ADR-0004, consumed by the handshake).
- Issue #4 — `[WP] Define Job lifecycle and scheduling model` (owns retry/resumption policy and action authorization).
- Issue #6 — `[WP] Define data-plane and storage contracts` (bulk transfer bytes, distinct from progress metadata).
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (must simulate this protocol's connect/dispatch/ack/progress/cancel/reconnect scenarios).
