# M0 — Agent Protocol Contract (Agent Protocol v1)

Status: **Proposed - awaiting owner approval**

## Context

This Specification defines the concrete message-level contract for the Agent control-plane protocol accepted in ADR-0005, executing Issue #3 (`[WP] Define Agent control and action contracts`). Per ADR-0003's contract-independence constraint, this Specification — not any Rust type definition — is the authoritative definition of "Agent Protocol v1."

## Transport and handshake

- WebSocket over TLS (WSS).
- **Server authentication (pinned TLS), Agent authentication (credential) — not mTLS.** The Server's certificate fingerprint must be delivered to the Agent through an authenticated, integrity-protected boot mechanism. This Specification does not assume the current boot chain already provides that assurance; the concrete delivery mechanism is informed by the Secure Boot / hardened boot-chain Technical Spike (Issue #10) and is not decided here. The Agent does not present a client certificate.
- A fingerprint mismatch fails closed: the Agent refuses the connection outright. There is no trust-on-first-use fallback and no acceptance of an unverified Server certificate under any circumstance.
- If Issue #10 finds the boot mechanism cannot provide sufficient authenticated fingerprint delivery, this Server-authentication mechanism must be revisited (see ADR-0005) without changing the WSS/typed-protocol transport decision automatically.
- Handshake sequence: Agent connects → Agent verifies the Server's pinned TLS fingerprint (fail closed on mismatch) → Agent sends `AuthRequest{credential}` → Server validates the credential against the Endpoint identity/credential state model (`docs/specifications/m0-endpoint-identity-lifecycle.md`) → Server responds `SessionEstablished{protocol_version, session_id}` or `AuthError{reason}`.
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
- `ActionAck{action_id, outcome: Accepted|Rejected, error?}` — Agent → Server; confirms whether a dispatch was accepted for execution or rejected before execution (e.g., unknown or malformed `action_type`), distinct from completion. A `Rejected` dispatch never executed and must never be represented as an `ActionResult` with outcome `Failed` — no execution occurred, so there is no result to report, only a rejection reason.
- `ActionProgress{action_id, percent?, bytes_processed?, eta?}` — Agent → Server; periodic progress metadata for long-running actions. Bulk transfer bytes are not carried here (data-plane, Issue #6).
- `ActionResult{action_id, outcome: Succeeded|Failed|Cancelled, detail}` — Agent → Server; final result.
- `CancelAction{action_id}` — Server → Agent; requests cancellation.
- `CancelAck{action_id, outcome: Cancelled|AlreadyCompleted|CannotCancel}` — Agent → Server.
- `StatusQuery{action_id}` — Server → Agent; requests the Agent's authoritative current knowledge of a previously dispatched action, used on reconnect instead of blind redispatch.
- `StatusReport{action_id, known_state}` — Agent → Server; response to `StatusQuery`. `known_state` includes an explicit `Unknown` value for an `action_id` the Agent has no local record of (e.g., after an Agent restart). `Unknown` means the execution outcome is unknown — it must never be read as "not executed."
- `Heartbeat` / `HeartbeatAck` — liveness; interval to be defined at implementation time (not an M0 architectural question).

This is the illustrative v1 message set required to satisfy Issue #3's scope (correlation, acknowledgement, duplicate handling, timeout, reconnect, cancellation, progress, protocol version, idempotency). It is not a claim that no additional message types will ever be needed as other M0 Work Packages are implemented.

## Typed action catalog

Not enumerated by this Specification. Each M0 Work Package that introduces a concrete Agent-executed operation (e.g., inventory collection, data-plane transfer steps) defines its own `action_type`, parameter schema, and result shape conforming to the `ActionDispatch`/`ActionResult` envelope above. Only fixed tools available in the Alpine maintenance environment may be invoked by any `action_type`; the Agent must reject an unknown or malformed `action_type` explicitly rather than attempting a best-effort interpretation.

## Idempotency and duplicate handling

`action_id` is the idempotency key, and duplicate suppression holds **only while the Agent retains authoritative local state for that action**. On receiving an `ActionDispatch` with an `action_id` it has already seen and still has state for:

- if completed: respond immediately with the stored `ActionResult`, without re-executing;
- if in progress: respond with current status (`ActionAck` and/or `ActionProgress`), without starting a second execution.

This is **not** a claim of exactly-once execution across an Agent restart or any other loss of local action state. If the Agent no longer has state for a previously dispatched `action_id` (e.g., after a restart) and receives a fresh `ActionDispatch` or `StatusQuery` for it, it must answer truthfully from its actual current state — `StatusQuery` gets `Unknown`; a fresh `ActionDispatch` is treated as a new dispatch, since the Agent cannot know whether the prior attempt executed. Deciding whether it is safe to proceed in that situation, especially for a destructive action, is Job lifecycle policy (Issue #4), not this Specification.

## Retry (mechanism, not policy)

A retry is a new `ActionDispatch` with a fresh `action_id` and `retry_of` referencing the prior attempt, for audit/correlation. This Specification defines only the mechanism; deciding when a retry is appropriate — especially for destructive actions — is Job lifecycle policy (Issue #4) and must never be automatic merely because this mechanism exists.

## Reconnect / stale-command handling

On reconnect (after a fresh handshake), the Server issues `StatusQuery` for any action it still considers in-flight rather than redispatching blindly. The Agent must answer truthfully from its own local state, including by returning `Unknown` when it has no record of the queried `action_id` (e.g., after an Agent restart) — this must be treated as an unknown execution outcome, not as evidence the action did not execute, and must never by itself trigger redispatch of a destructive action merely because the Agent no longer remembers the `action_id`. Whether to redispatch, cancel, or escalate to operator review based on the `StatusReport` (including an `Unknown` result) is Job lifecycle policy (Issue #4), not defined here.

## Wire encoding

Agent Protocol v1 uses **UTF-8 JSON carried in WebSocket text frames**, unless a concrete blocker is found during implementation. Bulk transfer bytes are never carried by this protocol — they belong to the data-plane contract (Issue #6), which may use its own encoding independently.

Cross-language conventions required to make this Specification independently implementable:

- **Timestamps**: RFC 3339 / ISO 8601 UTC string (e.g. `"2026-08-14T21:00:00Z"`). No epoch integers.
- **Identifiers** (`action_id`, `message_id`, `session_id`): UUID (version 4) represented as a lowercase hyphenated string.
- **Absent optional fields**: an optional field that has no value is **omitted from the JSON object entirely**, never sent as `null`. A receiver treats a missing field and a field explicitly absent as the same thing; senders must not send `null` for an optional field.
- **Unknown message types**: a message whose `type` is not one of the types defined in this Specification is rejected explicitly (e.g., via `AuthError` at handshake time, or connection closure with a reason after handshake) — never silently dropped or best-effort interpreted, consistent with the rejection principle already required for unknown `action_type`.
- **Unknown fields within a known message type**: a receiver ignores fields it does not recognize within an otherwise valid, known message type, to allow forward-compatible minor additions. This tolerance applies only to fields, never to the top-level `type`.

## Safety

- The Agent must reject any request that is not one of the closed, versioned `action_type`s — no generic/arbitrary execution path exists at the protocol level (`AGENTS.md`).
- A fingerprint mismatch during the handshake fails closed; there is no trust-on-first-use fallback (see "Transport and handshake").
- Reconnect never implies automatic replay of a destructive action, and an `Unknown` `StatusReport` is never treated as proof an action did not execute (see "Reconnect / stale-command handling").
- `CancelAck` must report `CannotCancel` honestly rather than a false `Cancelled` when a destructive action has passed its point of no return.
- A `Rejected` `ActionAck` must never be reported as a `Failed` `ActionResult` — rejection means no execution occurred.

## Out of scope

- concrete `action_type` catalog — owned by the Work Packages that introduce each operation;
- Job/action authorization semantics and destructive-step resumption/retry policy (Issue #4);
- bulk data-transfer contract (Issue #6);
- Administrative API / Browser-Server protocol — not decided by this Work Package;
- heartbeat interval and other implementation-time tuning parameters;
- the concrete boot mechanism delivering an authenticated Server fingerprint to the Agent — informed by, not decided by, Issue #10.

## Acceptance criteria

- Message-level contract details are defined (Issue #3 acceptance criterion).
- Contract-test expectations are defined below, satisfying `docs/development/testing.md` "Contract tests".

## Validation expectations (contract tests)

Per `docs/development/testing.md` "Contract tests", expected coverage once implemented includes:

- serialization/deserialization of every message type, including the wire-encoding conventions above (timestamp format, identifier format, omitted-vs-null optional fields);
- required vs. optional field handling (e.g., `ActionProgress` fields are optional);
- `protocol_version` mismatch handling (explicit `AuthError`, not silent acceptance);
- Server TLS fingerprint mismatch handling (fail closed, connection refused, no trust-on-first-use);
- duplicate `ActionDispatch` handling while Agent state exists (idempotent response, no double execution);
- unknown `action_type` handling (`ActionAck{outcome: Rejected}`, never an `ActionResult{outcome: Failed}`);
- unknown top-level message `type` handling (explicit rejection, not silently dropped);
- `CancelAction` on an already-completed action (`AlreadyCompleted`, not a false `Cancelled`);
- `StatusQuery` for an `action_id` the Agent has no record of (`Unknown`, not treated as "not executed").

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
- Issue #10 — `[Spike] Validate Secure Boot and hardened boot chain` (informs whether the boot mechanism can deliver an authenticated Server fingerprint to the Agent).

## Open questions

1. Heartbeat interval and connection-liveness tuning — implementation-time detail.
2. Whether the Administrative API (Browser-Server) should reuse any envelope conventions from this protocol — not decided, out of scope for M0's Issue #3.
3. The concrete authenticated boot mechanism for fingerprint delivery — depends on Issue #10's evidence, not decided here.

Status: Proposed - awaiting owner approval.
