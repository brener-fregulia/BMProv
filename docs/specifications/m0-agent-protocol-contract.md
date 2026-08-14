# M0 — Agent Protocol Contract (Agent Protocol v1)

Status: **Proposed - awaiting owner approval**

## Context

This Specification defines the concrete message-level contract for the Agent control-plane protocol accepted in ADR-0005, executing Issue #3 (`[WP] Define Agent control and action contracts`). Per ADR-0003's contract-independence constraint, this Specification — not any Rust type definition — is the authoritative definition of "Agent Protocol v1."

## Transport and handshake

- WebSocket over TLS (WSS).
- The Server's certificate fingerprint is distributed to the Agent via the boot chain (Boot Orchestrator, ADR-0004) prior to connection.
- Handshake sequence: Agent connects → Agent verifies the Server's TLS fingerprint → Agent sends `AuthRequest{credential}` → Server validates the credential against the Endpoint identity/credential state model (`docs/specifications/m0-endpoint-identity-lifecycle.md`) → Server responds `SessionEstablished{protocol_version, session_id}` or `AuthError{reason}`.
- A `protocol_version` mismatch at handshake is an explicit `AuthError`, never silently downgraded or ignored.

## Message envelope

Every message includes:

- `message_id` — unique per message;
- `protocol_version` — e.g. `"1"`;
- `type` — message type (see below);
- `timestamp`;
- `correlation_id` — present where the message relates to a specific action; equals the relevant `action_id`.

## Message types

- `AuthRequest` / `SessionEstablished` / `AuthError` — handshake (see above).
- `ActionDispatch{action_id, action_type, action_version, parameters, retry_of?}` — Server → Agent; requests execution of a typed action.
- `ActionAck{action_id}` — Agent → Server; confirms receipt/acceptance of a dispatch, distinct from completion.
- `ActionProgress{action_id, percent?, bytes_processed?, eta?}` — Agent → Server; periodic progress metadata for long-running actions. Bulk transfer bytes are not carried here (data-plane, Issue #6).
- `ActionResult{action_id, outcome: Succeeded|Failed|Cancelled, detail}` — Agent → Server; final result.
- `CancelAction{action_id}` — Server → Agent; requests cancellation.
- `CancelAck{action_id, outcome: Cancelled|AlreadyCompleted|CannotCancel}` — Agent → Server.
- `StatusQuery{action_id}` — Server → Agent; requests the Agent's authoritative current knowledge of a previously dispatched action, used on reconnect instead of blind redispatch.
- `StatusReport{action_id, known_state}` — Agent → Server; response to `StatusQuery`.
- `Heartbeat` / `HeartbeatAck` — liveness; interval to be defined at implementation time (not an M0 architectural question).

This is the illustrative v1 message set required to satisfy Issue #3's scope (correlation, acknowledgement, duplicate handling, timeout, reconnect, cancellation, progress, protocol version, idempotency). It is not a claim that no additional message types will ever be needed as other M0 Work Packages are implemented.

## Typed action catalog

Not enumerated by this Specification. Each M0 Work Package that introduces a concrete Agent-executed operation (e.g., inventory collection, data-plane transfer steps) defines its own `action_type`, parameter schema, and result shape conforming to the `ActionDispatch`/`ActionResult` envelope above. Only fixed tools available in the Alpine maintenance environment may be invoked by any `action_type`; the Agent must reject an unknown or malformed `action_type` explicitly rather than attempting a best-effort interpretation.

## Idempotency and duplicate handling

`action_id` is the idempotency key. On receiving an `ActionDispatch` with an `action_id` it has already seen:

- if completed: respond immediately with the stored `ActionResult`, without re-executing;
- if in progress: respond with current status (`ActionAck` and/or `ActionProgress`), without starting a second execution.

## Retry (mechanism, not policy)

A retry is a new `ActionDispatch` with a fresh `action_id` and `retry_of` referencing the prior attempt, for audit/correlation. This Specification defines only the mechanism; deciding when a retry is appropriate — especially for destructive actions — is Job lifecycle policy (Issue #4) and must never be automatic merely because this mechanism exists.

## Reconnect / stale-command handling

On reconnect (after a fresh handshake), the Server issues `StatusQuery` for any action it still considers in-flight rather than redispatching blindly. The Agent must answer truthfully from its own local state, including when it has no record of the queried `action_id` (e.g., after an Agent restart). Whether to redispatch, cancel, or escalate to operator review based on the `StatusReport` is Job lifecycle policy (Issue #4), not defined here.

## Safety

- The Agent must reject any request that is not one of the closed, versioned `action_type`s — no generic/arbitrary execution path exists at the protocol level (`AGENTS.md`).
- Reconnect never implies automatic replay of a destructive action (see "Reconnect / stale-command handling").
- `CancelAck` must report `CannotCancel` honestly rather than a false `Cancelled` when a destructive action has passed its point of no return.

## Out of scope

- concrete `action_type` catalog — owned by the Work Packages that introduce each operation;
- Job/action authorization semantics and destructive-step resumption/retry policy (Issue #4);
- bulk data-transfer contract (Issue #6);
- Administrative API / Browser-Server protocol — not decided by this Work Package;
- heartbeat interval and other implementation-time tuning parameters.

## Acceptance criteria

- Message-level contract details are defined (Issue #3 acceptance criterion).
- Contract-test expectations are defined below, satisfying `docs/development/testing.md` "Contract tests".

## Validation expectations (contract tests)

Per `docs/development/testing.md` "Contract tests", expected coverage once implemented includes:

- serialization/deserialization of every message type;
- required vs. optional field handling (e.g., `ActionProgress` fields are optional);
- `protocol_version` mismatch handling (explicit `AuthError`, not silent acceptance);
- duplicate `ActionDispatch` handling (idempotent response, no double execution);
- unknown `action_type` handling (explicit rejection);
- `CancelAction` on an already-completed action (`AlreadyCompleted`, not a false `Cancelled`);
- `StatusQuery` for an `action_id` the Agent has no record of.

Per `docs/development/testing.md` "Local development environments," these are expected to run in the Linux reference environment (WSL2 or containers from Windows), not asserted as correct from native-Windows execution alone.

Manual: owner approval of this Specification.

## Related ADRs

- ADR-0005 — Agent control-plane protocol and typed-action model (`Accepted`).
- ADR-0004 — Endpoint identity and enrollment/trust bootstrap model (credential this handshake validates).

## Related work

- Issue #3 — `[WP] Define Agent control and action contracts`.
- Issue #2 — `[WP] Define endpoint identity and trust model` (ADR-0004, credential validated during handshake).
- Issue #4 — `[WP] Define Job lifecycle and scheduling model` (Job/action authorization, retry policy, resumption policy; consumes `StatusQuery`/`ActionDispatch`).
- Issue #6 — `[WP] Define data-plane and storage contracts` (bulk transfer bytes, distinct from `ActionProgress` metadata).
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (must simulate this protocol's scenarios).

## Open questions

1. Heartbeat interval and connection-liveness tuning — implementation-time detail.
2. Whether the Administrative API (Browser-Server) should reuse any envelope conventions from this protocol — not decided, out of scope for M0's Issue #3.

Status: Proposed - awaiting owner approval.
