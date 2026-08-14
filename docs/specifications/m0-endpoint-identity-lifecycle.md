# M0 — Endpoint Identity Lifecycle

Status: **Proposed - awaiting owner approval**

## Context

This Specification defines the Endpoint identity lifecycle states, transitions, and the destructive-operation identity precondition required by M0, executing Issue #2 (`[WP] Define endpoint identity and trust model`) together with ADR-0004 (Endpoint identity and enrollment/trust bootstrap model).

ADR-0004 contains one open decision (auto-trust vs. operator-approval-gated first enrollment). This Specification's `PendingEnrollment` state is conditional on that decision; everything else below does not depend on it.

## Identity model

- Durable Endpoint identity is a Server-assigned identifier, independent of MAC/hardware.
- Inventory signals (MAC, disk fingerprint, hardware serials when available) are evidence attached to an Endpoint record, never the identity itself (`AGENTS.md`; `docs/discovery/architecture-redesign.md` "Security invariants").

## Lifecycle states

- **(no record)** — no boot/enrollment attempt has been observed yet; not a persisted state.
- **PendingEnrollment** — a first-seen endpoint has completed the credential exchange but is not yet trusted. Reachable only if the owner accepts operator-approval-gated enrollment in ADR-0004; if auto-trust is accepted instead, this state is skipped and endpoints move directly to `Enrolled`.
- **Enrolled** — the Endpoint identity record exists and is trusted for inventory purposes. Does not by itself authorize destructive operations.
- **CredentialActive** — a runtime Agent identity/session credential is currently valid for this Endpoint.
- **CredentialExpired** / **CredentialRevoked** — the runtime credential is no longer valid; the Agent must re-redeem before further action is accepted from it.
- **StaleHardwareSignal** — an observed inventory signal (MAC, disk fingerprint) no longer matches the Endpoint's recorded signals. Destructive operations must not proceed against this Endpoint until an operator resolves the discrepancy.
- **Retired** — an operator has explicitly retired the Endpoint (e.g., decommissioned hardware); no further Jobs may target it.

## Transition rules

Illustrative and sufficient to bound implementation; the exhaustive transition table is implementation-time work once ADR-0004's open decision is resolved.

- `(no record)` → `PendingEnrollment` or `Enrolled`: on successful credential redemption (branch depends on ADR-0004's accepted enrollment mode).
- `PendingEnrollment` → `Enrolled`: only via explicit operator approval action.
- `PendingEnrollment` → discarded: if never approved (retention/cleanup policy is an implementation-time detail).
- `Enrolled` → `CredentialActive`: on successful runtime credential issuance.
- `CredentialActive` → `CredentialExpired`: on credential expiry (TTL is an implementation-time detail).
- `CredentialActive` / `CredentialExpired` → `CredentialActive`: on successful re-redemption/reconnect.
- `Enrolled` / `CredentialActive` → `StaleHardwareSignal`: on detecting a mismatched inventory signal against the recorded record.
- `StaleHardwareSignal` → `Enrolled`: only via explicit operator confirmation, never automatically.
- `Enrolled` / `CredentialActive` / `StaleHardwareSignal` → `Retired`: explicit operator action only.

## Destructive-operation identity precondition

Before any destructive operation executes against an Endpoint, all of the following must hold:

1. the Endpoint is not in `StaleHardwareSignal` or `Retired`;
2. the Endpoint's runtime credential is in `CredentialActive` at authorization time;
3. the inventory revision the operation was authorized against matches the Endpoint's current inventory revision;
4. the target disk/volume identity/fingerprint matches what the operation was authorized against.

Any of these failing must block the destructive operation and surface a clear reason — never a silent retry or silent override. This precondition is normative for Issues #4 and #6, which should reference it directly rather than re-derive it.

## Reconnect / stale-command handling

- A reconnecting Agent must re-authenticate; the Server must not resume a previous runtime credential automatically merely because MAC/hardware signals match (ADR-0004 "Reconnect handling").
- A destructive command issued before a disconnect must not be blindly replayed on reconnect. Whether and how an interrupted destructive JobStep resumes is owned by the Job lifecycle Work Package (Issue #4); that Work Package must treat "Agent reconnected" and "destructive step may safely resume" as separate questions, both gated by the identity precondition above.

## Out of scope

- the concrete enrollment-mode decision (auto-trust vs. operator-approval-gated) — open in ADR-0004; this Specification's `PendingEnrollment` state is conditional on it;
- Agent/Server mutual authentication mechanism (Issue #3);
- Job/JobStep resumption semantics after reconnect (Issue #4) — this Specification defines only the identity precondition those semantics must satisfy;
- production enrollment UX/workflow implementation.

## Acceptance criteria

- Identity lifecycle states and transition principles are defined, satisfying Issue #2's acceptance criterion for "a Specification defining the identity lifecycle."
- The destructive-operation identity precondition is explicit and referenceable by Issues #4 and #6 without re-derivation.
- Stale-state and reconnect handling are explicit, consistent with the safety constraint that reconnect must not blindly re-establish trust.

## Validation expectations

Automated: none produced directly by this Work Package (decision/specification work). Once identity/enrollment is implemented, expected validation includes domain tests for the state transitions above (valid and rejected transitions, per `docs/development/testing.md` "Unit and domain tests") and negative cases demonstrating that `StaleHardwareSignal` and `Retired` endpoints reject destructive operations. Per `docs/development/testing.md` "Local development environments," such domain and integration tests are expected to run in the Linux reference environment (WSL2 or containers from Windows), not asserted as correct from native-Windows execution alone.

Manual: owner approval of this Specification and ADR-0004.

## Related ADRs

- ADR-0004 — Endpoint identity and enrollment/trust bootstrap model (`Proposed`; this Specification's `PendingEnrollment` branch depends on its open decision).

## Related work

- Issue #2 — `[WP] Define endpoint identity and trust model`.
- Issue #3 — `[WP] Define Agent control and action contracts` (mutual-authentication mechanism).
- Issue #4 — `[WP] Define Job lifecycle and scheduling model` (consumes the destructive-operation identity precondition).
- Issue #6 — `[WP] Define data-plane and storage contracts` (consumes the destructive-operation identity precondition).

## Open questions

1. Same open decision as ADR-0004: auto-trust vs. operator-approval-gated first enrollment — determines whether `PendingEnrollment` is a real state or is skipped.
2. Exact credential TTL/renewal policy — an implementation-time detail, not an M0 architectural blocker, intentionally left unresolved here.

Status: Proposed - awaiting owner approval.
