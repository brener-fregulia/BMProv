# M0 — Agent Protocol Contract (Agent Protocol v1)

Status: **Approved**

## Context

This Specification defines the concrete message-level contract for the Agent control-plane protocol accepted in ADR-0005, executing Issue #3 (`[WP] Define Agent control and action contracts`). Per ADR-0003's contract-independence constraint, this Specification — not any Rust type definition — is the authoritative definition of "Agent Protocol v1."

## Transport and handshake

- WebSocket over TLS (WSS).
- **Server authentication (pinned TLS), Agent authentication (credential) — not mTLS.** The Server's certificate fingerprint must be delivered to the Agent through an authenticated, integrity-protected trusted bootstrap — the `trusted bootstrap established` security property accepted in ADR-0010, with Secure Boot as the V1 baseline mechanism for establishing executable boot-chain integrity on UEFI x86-64. Secure Boot alone does not authenticate the fingerprint or enrollment data itself; the fingerprint is authenticated via a nonce-bound signed bootstrap assertion, and its Server-observable representation is carried post-authentication by `BootstrapEvidence` (see "Trusted bootstrap evidence" below) — both accepted in `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`. That Specification's site trust-anchor provisioning question (how an Endpoint comes to trust the signing key in the first place) remains separately unresolved and is not decided here. The Agent does not present a client certificate.
- The Agent verifies the pinned Server fingerprint **before** Agent Protocol authentication begins. A fingerprint mismatch is a local TLS/Server-authentication failure, not an Agent Protocol event: the Agent aborts the connection immediately at the TLS layer, and no `AuthError` (or any other Agent Protocol message) is exchanged for it. There is no trust-on-first-use fallback and no acceptance of an unverified Server certificate under any circumstance.
- The Agent must fail closed when trustworthy bootstrap material — including the expected Server fingerprint — cannot be established through the trusted-bootstrap path: it must never fall back to an unverified Server certificate or proceed without one. This requirement, and the WSS/pinned-TLS/typed Agent Protocol decisions themselves, are unchanged by ADR-0010; ADR-0005 is not reopened.
- Handshake sequence: Agent connects → Agent verifies the Server's pinned TLS fingerprint (mismatch aborts the connection immediately, no Agent Protocol message exchanged) → *(TLS Server-authentication succeeded)* → Agent sends `AuthRequest{credential}` → Server validates the credential against the Endpoint identity/credential state model (`docs/specifications/m0-endpoint-identity-lifecycle.md`) → Server responds `SessionEstablished{protocol_version, session_id}` or `AuthError{reason}`.
- `AuthError` is exchanged only for application-level Agent Protocol authentication/handshake failures that occur **after** the TLS Server identity check has already succeeded — a rejected enrollment/runtime credential, or an incompatible `protocol_version` during the Agent Protocol handshake. It is never used for a fingerprint mismatch.

## Trusted bootstrap evidence

After Agent Protocol authentication succeeds (`SessionEstablished`), the Agent sends a one-way `BootstrapEvidence` report carrying the Server-observable representation of the locally-established trusted-bootstrap fact (ADR-0010; `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`).

**This does not change WSS, pinned-TLS verification, `AuthRequest` credential semantics, or `SessionEstablished` semantics, and does not reopen ADR-0005.** `CredentialActive` and `trusted bootstrap established` remain independent facts (ADR-0010); this message is what lets the Server observe the latter without inferring it from the former, from a fingerprint match alone, from a TCP/WSS connection, or from mere possession of a valid assertion without performing independent verification.

**What Server-side verification of this evidence proves, and does not prove** (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` "(D) Server-side bootstrap evidence"): independently verifying the signed assertion proves the assertion was produced by the accepted site signer, that its authenticated material is intact, and that it is bound to the declared `boot_nonce`. It does **not** independently prove that firmware Secure Boot was enabled, that the expected executable chain actually executed, or that the current Agent process itself is genuine — those remain covered only by the production trusted-bootstrap baseline (ADR-0010), not re-proven by this message. This is an authenticated Agent report, not cryptographic remote attestation.

**Sequencing and correlation:**

- `BootstrapEvidence` is sent only after TLS pin verification succeeded and `SessionEstablished` has been received — never before authentication, never as part of the handshake itself.
- The Server independently verifies the signed assertion (signature, signer identity against the site trust anchor, schema/version, exact `boot_nonce` match, required field validity) — it never trusts the Agent's `local_boot_trust` claim by itself.
- `boot_nonce` also serves as the boot-context correlator: the Server associates an accepted result with the current Endpoint boot context via this value. A WSS reconnect within the same boot context may re-present the same `boot_nonce`/assertion for the Server to re-correlate/reconfirm; a new one is not required merely because the socket changed. A genuine reboot produces a new `boot_nonce` and requires newly-established evidence.
- Missing, malformed, invalid-signature, nonce-mismatched, or otherwise rejected evidence leaves the Server-side `trusted bootstrap` fact `NotEstablished` for that boot context and fails closed — no destructive dispatch may satisfy destructive-operation precondition 7 (`docs/specifications/m0-endpoint-identity-lifecycle.md`) before this evidence is accepted.

## Transfer authorization

After `SessionEstablished`, and after `ActionAck{outcome: Accepted}` for a data-plane transfer action, the Agent may request the short-lived, transfer-scoped authorization capability required to use the HTTPS data-plane channel (`docs/decisions/0008-data-plane-transport-chunking-and-resumability.md` point 9; `docs/specifications/m0-data-plane-and-storage-contracts.md` "Transfer-session authentication", Issue #15).

**This does not change WSS, pinned-TLS verification, `AuthRequest`/`SessionEstablished` semantics, `BootstrapEvidence`, or the general Agent-action envelope, and does not reopen ADR-0005.** Bulk Artifact bytes are still never carried by Agent Protocol — only the small authorization capability is, over the same pattern already used for any other action-specific data.

- `TransferAuthorizationRequest{transfer_id}` — Agent → Server. May be sent any time the Agent holds a currently-authenticated session and believes `transfer_id` denotes a still-legitimate, non-terminal transfer it is authorized to act on — including as a **renewal** request for a `transfer_id` whose previous authorization has expired. A renewal request is not a retry and does not create a new Attempt.
- `TransferAuthorizationGrant{transfer_id, token, expires_at}` — Server → Agent. Sent only after the Server independently confirms a non-terminal Attempt exists for `transfer_id`, bound to the requesting Endpoint, with `CredentialActive`. `token` is an opaque, Server-signed bearer capability from this protocol's perspective — its internal structure is owned by `docs/specifications/m0-data-plane-and-storage-contracts.md`, not redefined here.
- `TransferAuthorizationDenied{transfer_id, reason}` — Server → Agent. `reason` is intentionally a minimal, non-enumerable value for M0 (a single closed value is sufficient); this message must not let a requester distinguish "unknown transfer_id" from "wrong Endpoint" from "transfer already terminal" from any other internal cause — see `m0-data-plane-and-storage-contracts.md` "Revocation and fail-closed behavior" for why.

The token is presented on the separate HTTPS data-plane channel, never over Agent Protocol itself, exactly as bulk chunk bytes never are.

## Message envelope

Every message includes:

- `message_id` — unique per message;
- `protocol_version` — e.g. `"1"`;
- `type` — message type (see below);
- `timestamp`;
- `correlation_id` — optional; identifies the logical operation or message this message refers to. For action-scoped messages, it equals the relevant `action_id`. For a `ProtocolError` concerning a non-action message, it may instead equal the offending `message_id`. No separate reply/correlation field is introduced in M0.

## Agent-action state vocabulary

`StatusReport.known_state` and `ActionResult.outcome` draw from a small, closed vocabulary describing **only the Agent's own local knowledge of one action** — it is not, and must not be conflated with or absorbed into, the Job/JobStep lifecycle owned by Issue #4:

- `Accepted` — the Agent accepted the dispatch but has not yet started execution.
- `Running` — execution is in progress.
- `Succeeded` — execution completed successfully.
- `Failed` — execution completed unsuccessfully.
- `Cancelled` — execution was cancelled before or during completion.
- `Unknown` — the Agent has no authoritative local state for this `action_id` (e.g., after an Agent restart). Means the execution outcome is unknown, never "not executed."

## Message types

- `AuthRequest` / `SessionEstablished` / `AuthError` — handshake (see above). `AuthError` is used **only** for application-level Agent Protocol authentication/handshake failures occurring after the TLS Server identity check has already succeeded — a rejected enrollment/runtime credential, or an incompatible `protocol_version` during the handshake. A TLS fingerprint mismatch is never reported as `AuthError`; it is a connection-level abort before any Agent Protocol message is exchanged. Protocol-level violations after a session is established use `ProtocolError` instead (see below).
- `BootstrapEvidence{boot_nonce, bootstrap_assertion, local_boot_trust: Established}` — Agent → Server, one-way, sent only after `SessionEstablished`. Carries the Server-observable representation of the locally-established trusted-bootstrap fact; see "Trusted bootstrap evidence" above for full semantics, independent Server-side verification requirements, and boot-context correlation via `boot_nonce`. `bootstrap_assertion` is an opaque value from this protocol's perspective — its internal structure is owned by `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`, not redefined here. `local_boot_trust` is a closed vocabulary with a single defined M0 value, `Established` — a local failure to establish trusted bootstrap already prevents the Agent from reaching this point in the sequence, so no other value is sent in M0.
- `TransferAuthorizationRequest{transfer_id}` / `TransferAuthorizationGrant{transfer_id, token, expires_at}` / `TransferAuthorizationDenied{transfer_id, reason}` — see "Transfer authorization" above.
- `ActionDispatch{action_id, action_type, action_version, parameters, retry_of?}` — Server → Agent; requests execution of a typed action.
- `ActionAck{action_id, outcome: Accepted|Rejected, error?}` — Agent → Server; confirms whether a dispatch was accepted for execution or rejected before execution (e.g., unknown or malformed `action_type`), distinct from completion. A `Rejected` dispatch never executed and must never be represented as an `ActionResult` with outcome `Failed` — no execution occurred, so there is no result to report, only a rejection reason.
- `ActionProgress{action_id, percent?, bytes_processed?, eta?}` — Agent → Server; periodic progress metadata for long-running actions. Bulk transfer bytes are not carried here (data-plane, Issue #6).
- `ActionResult{action_id, outcome: Succeeded|Failed|Cancelled, detail}` — Agent → Server; final result. Uses the Agent-action state vocabulary above.
- `CancelAction{action_id}` — Server → Agent; requests cancellation.
- `CancelAck{action_id, outcome: Cancelled|AlreadyCompleted|CannotCancel|Unknown}` — Agent → Server. `Unknown` means the Agent has no authoritative local state for this `action_id` and cannot claim whether the previous execution completed, failed, or was never executed — it is a distinct outcome from `CannotCancel` (which means the Agent *does* know about the action but it is past the point where cancellation is possible) and must not be collapsed into it.
- `StatusQuery{action_id}` — Server → Agent; requests the Agent's authoritative current knowledge of a previously dispatched action, used on reconnect instead of blind redispatch.
- `StatusReport{action_id, known_state}` — Agent → Server; response to `StatusQuery`. `known_state` uses the Agent-action state vocabulary above, including `Unknown` for an `action_id` the Agent has no local record of. `Unknown` means the execution outcome is unknown — it must never be read as "not executed."
- `Heartbeat` / `HeartbeatAck` — liveness; interval to be defined at implementation time (not an M0 architectural question).
- `ProtocolError{code, message, correlation_id?}` — either direction; represents a post-handshake protocol-level violation (e.g., an unknown top-level message `type`, a malformed envelope, or another violation of this contract that is not an authentication/handshake failure and not a specific action's rejection). `correlation_id` follows the envelope's general semantics: present and equal to the relevant `action_id` when the error concerns a specific action, present and equal to the offending `message_id` when it concerns some other non-action message, and absent for connection-level violations with no single message to point to. Whether a given `ProtocolError` also closes the WebSocket connection is implementation/policy detail, unless a specific case is later found to require closure for safety — not decided by this Specification.

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

## Acknowledgment timeout semantics

Exact acknowledgment, heartbeat, and liveness durations are implementation parameters, not decided here. The contract obligation is behavioral: failure to receive `ActionAck` within the expected acknowledgment window produces an **uncertain delivery outcome** — it is not proof that the Agent did not receive or did not begin the action; the dispatch may have been received and the acknowledgment lost, delayed, or the connection may have dropped after receipt. A missing or timed-out acknowledgment must never by itself cause blind redispatch of a destructive action. As with other uncertain-outcome cases in this Specification (`Unknown` status, `Unknown` cancellation), reconciliation and retry policy for a timed-out acknowledgment remain owned by Job lifecycle (Issue #4), not this Specification.

## Reconnect / stale-command handling

On reconnect (after a fresh handshake), the Server issues `StatusQuery` for any action it still considers in-flight rather than redispatching blindly. The Agent must answer truthfully from its own local state, including by returning `Unknown` when it has no record of the queried `action_id` (e.g., after an Agent restart) — this must be treated as an unknown execution outcome, not as evidence the action did not execute, and must never by itself trigger redispatch of a destructive action merely because the Agent no longer remembers the `action_id`. Whether to redispatch, cancel, or escalate to operator review based on the `StatusReport` (including an `Unknown` result) is Job lifecycle policy (Issue #4), not defined here.

## Wire encoding

Agent Protocol v1 uses **UTF-8 JSON carried in WebSocket text frames**, unless a concrete blocker is found during implementation. Bulk transfer bytes are never carried by this protocol — they belong to the data-plane contract (Issue #6), which may use its own encoding independently.

Cross-language conventions required to make this Specification independently implementable:

- **Timestamps**: RFC 3339 / ISO 8601 UTC string (e.g. `"2026-08-14T21:00:00Z"`). No epoch integers.
- **Identifiers** (`action_id`, `message_id`, `session_id`): UUID (version 4) represented as a lowercase hyphenated string.
- **Absent optional fields**: an optional field that has no value is **omitted from the JSON object entirely**, never sent as `null`. A receiver treats a missing field and a field explicitly absent as the same thing; senders must not send `null` for an optional field.
- **Unknown message types**: a message whose `type` is not one of the types defined in this Specification is rejected explicitly — via `AuthError` if received during the handshake, or via `ProtocolError` if received after session establishment — never silently dropped or best-effort interpreted, consistent with the rejection principle already required for unknown `action_type`.
- **Unknown fields within a known message type**: a receiver ignores fields it does not recognize within an otherwise valid, known message type, to allow forward-compatible minor additions. This tolerance applies only to fields, never to the top-level `type`.

## Safety

- The Agent must reject any request that is not one of the closed, versioned `action_type`s — no generic/arbitrary execution path exists at the protocol level (`AGENTS.md`).
- A TLS Server fingerprint mismatch is a connection-level abort before any Agent Protocol message is exchanged — never an `AuthError` — and fails closed with no trust-on-first-use fallback (see "Transport and handshake").
- Reconnect never implies automatic replay of a destructive action, and an `Unknown` `StatusReport` is never treated as proof an action did not execute (see "Reconnect / stale-command handling").
- `CancelAck` must report `CannotCancel` honestly rather than a false `Cancelled` when a destructive action has passed its point of no return, and must report `Unknown` — never a false `CannotCancel` — when the Agent has no authoritative local state for the `action_id`.
- A `Rejected` `ActionAck` must never be reported as a `Failed` `ActionResult` — rejection means no execution occurred.
- A missing or timed-out `ActionAck` is an uncertain delivery outcome, never treated as proof of non-delivery, and never by itself grounds for blind redispatch of a destructive action (see "Acknowledgment timeout semantics").
- Missing, malformed, invalid-signature, nonce-mismatched, or otherwise rejected `BootstrapEvidence` leaves trusted bootstrap `NotEstablished` for the current boot context and fails closed — destructive-operation precondition 7 can never be satisfied without independently Server-verified evidence (see "Trusted bootstrap evidence"). The Server never infers `trusted bootstrap established` from `CredentialActive`, a fingerprint match alone, or a live connection.

## Out of scope

- concrete `action_type` catalog — owned by the Work Packages that introduce each operation;
- Job/action authorization semantics and destructive-step resumption/retry policy (Issue #4);
- bulk data-transfer contract, including the HTTPS data-plane channel itself, chunk transfer, and the internal structure of the transfer-authorization `token` (Issue #6/#15, `docs/specifications/m0-data-plane-and-storage-contracts.md`) — this Specification defines only how the authorization *request/grant/denial* is carried over Agent Protocol, not how the token is used or verified on the data plane;
- Administrative API / Browser-Server protocol — not decided by this Work Package;
- heartbeat interval and other implementation-time tuning parameters;
- the internal structure/format of `bootstrap_assertion`, and how it is produced/authenticated — owned by `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`; this Specification only defines how the already-produced evidence is carried over Agent Protocol, not how it is produced;
- the site trust-anchor provisioning mechanism (how an Endpoint comes to trust the signing key `bootstrap_assertion` is verified against) — remains unresolved in `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`, not designed here;
- Secure Boot / firmware boot-chain mechanics themselves — an Adapter/Boot Port concern (`docs/specifications/m0-stack-and-boundaries-baseline.md`), not this Specification's scope.

## Acceptance criteria

- Message-level contract details are defined (Issue #3 acceptance criterion).
- Contract-test expectations are defined below, satisfying `docs/development/testing.md` "Contract tests".

## Validation expectations (contract tests)

Per `docs/development/testing.md` "Contract tests", expected coverage once implemented includes:

- serialization/deserialization of every message type, including the wire-encoding conventions above (timestamp format, identifier format, omitted-vs-null optional fields);
- required vs. optional field handling (e.g., `ActionProgress` fields are optional);
- `protocol_version` mismatch handling during the Agent Protocol handshake (explicit `AuthError`, not silent acceptance);
- Server TLS fingerprint mismatch handling (connection aborted at the TLS layer before any Agent Protocol message, fail closed, no trust-on-first-use, and specifically no `AuthError` exchanged);
- duplicate `ActionDispatch` handling while Agent state exists (idempotent response, no double execution);
- unknown `action_type` handling (`ActionAck{outcome: Rejected}`, never an `ActionResult{outcome: Failed}`);
- unknown top-level message `type` handling: `AuthError` during handshake, `ProtocolError` after session establishment — never silently dropped;
- `CancelAction` on an already-completed action (`AlreadyCompleted`, not a false `Cancelled`);
- `CancelAction` on an `action_id` the Agent has no record of (`Unknown`, not collapsed into `CannotCancel`);
- `StatusQuery` for an `action_id` the Agent has no record of (`Unknown`, not treated as "not executed");
- missing/timed-out `ActionAck` treated as an uncertain outcome, not as proof of non-delivery, and not triggering automatic redispatch;
- `BootstrapEvidence` handling: a valid signed assertion with matching `boot_nonce` is accepted and correlated to the current boot context; an invalid signature is rejected; a nonce mismatch (replay) is rejected; missing evidence leaves trusted bootstrap `NotEstablished`; a reconnect re-presenting the same `boot_nonce`/assertion within the same boot context is accepted/reconciled without requiring new evidence; evidence sent before `SessionEstablished` is rejected.
- `TransferAuthorizationRequest`/`Grant`/`Denied` handling: a request for a `transfer_id` with a legitimate non-terminal Attempt bound to the requesting Endpoint and `CredentialActive` is granted; a request for an unknown, another-Endpoint's, or already-terminal `transfer_id` is denied with the single generic `reason` value, never a case-distinguishing one; a renewal request for the same `transfer_id` after a prior grant's `expires_at` has passed is accepted and does not require a new Attempt; a request sent before `SessionEstablished` is rejected.

Per `docs/development/testing.md` "Local development environments," these are expected to run in the Linux reference environment (WSL2 or containers from Windows), not asserted as correct from native-Windows execution alone.

Manual: owner approval of this Specification — confirmed (see Status).

## Related ADRs

- ADR-0005 — Agent control-plane protocol and typed-action model (`Accepted`).
- ADR-0004 — Endpoint identity and enrollment/trust bootstrap model (credential this handshake validates; `CredentialActive`/`CredentialRevoked` transfer-authorization issuance checks).
- ADR-0010 — Trusted bootstrap and Secure Boot baseline (`Accepted`) — establishes `trusted bootstrap established` as the security property `BootstrapEvidence` makes Server-observable.
- ADR-0008 — Data-plane transport, chunking, and resumability strategy (`Accepted`) — point 9 is the decision `TransferAuthorizationRequest`/`Grant`/`Denied` carry over this protocol.

## Related work

- Issue #3 — `[WP] Define Agent control and action contracts`.
- Issue #2 — `[WP] Define endpoint identity and trust model` (ADR-0004, credential validated during handshake).
- Issue #4 — `[WP] Define Job lifecycle and scheduling model` (Job/action authorization, retry policy, resumption policy; consumes `StatusQuery`/`ActionDispatch`).
- Issue #6 — `[WP] Define data-plane and storage contracts` (bulk transfer bytes, distinct from `ActionProgress` metadata).
- Issue #15 — `[WP] Define authenticated data-plane transfer-session binding` — added `TransferAuthorizationRequest`/`Grant`/`Denied` additively, without reopening WSS, pinned TLS, `AuthRequest`/`SessionEstablished`, or `BootstrapEvidence`.
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (must simulate this protocol's scenarios, including `BootstrapEvidence` and transfer authorization).
- Issue #10 / ADR-0010 — `[Spike] Validate Secure Boot and hardened boot chain` (complete; established Secure Boot as the V1 `trusted bootstrap established` baseline).
- Issue #13 / ADR-0011 — `[WP] Define trusted bootstrap and Server fingerprint delivery contract` (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`, `Approved`) — accepted the nonce-bound signed bootstrap assertion (C), the authenticated Agent bootstrap report (D) this `BootstrapEvidence` message carries, and site trust-anchor provisioning (B); complete, not affected by this amendment.

## Open questions

1. Heartbeat interval and connection-liveness tuning — implementation-time detail.
2. Whether the Administrative API (Browser-Server) should reuse any envelope conventions from this protocol — not decided, out of scope for M0's Issue #3.
3. The internal structure/format of `bootstrap_assertion`, and the site trust-anchor provisioning mechanism it depends on — owned by `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` (Issue #13), which remains `Proposed` pending resolution of sub-problem (B). Not decided here.

Status: Approved.
