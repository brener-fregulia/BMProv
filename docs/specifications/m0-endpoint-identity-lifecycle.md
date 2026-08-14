# M0 — Endpoint Identity Lifecycle

Status: **Approved**

## Context

This Specification defines the Endpoint identity lifecycle, credential/session lifecycle, hardware-confidence state, and the destructive-operation authorization preconditions required by M0, executing Issue #2 (`[WP] Define endpoint identity and trust model`) together with ADR-0004 (Endpoint identity and enrollment/trust bootstrap model, `Accepted`).

Operator-approval-gated first enrollment is the accepted M0 default (ADR-0004): a newly observed device must not automatically become a trusted Endpoint merely because it can reach the Bamep Server.

## Identity model

- Durable Endpoint identity is a Server-assigned identifier, independent of MAC/hardware.
- Inventory signals (MAC, disk fingerprint, hardware serials when available) are evidence attached to an Endpoint record, never the identity itself (`AGENTS.md`; `docs/discovery/architecture-redesign.md` "Security invariants").

## State dimensions

An Endpoint's trust and operational readiness are governed by **three independent state dimensions**, not one mutually exclusive machine. An earlier draft of this Specification collapsed persistent identity, credential/session validity, and hardware-signal confidence into a single state list (e.g., a `StaleHardwareSignal` state alongside `CredentialActive`); that conflated concerns that legitimately coexist — an `Enrolled` Endpoint can simultaneously have an expired credential and lowered hardware confidence, and forcing that into one mutually exclusive machine would make the combination unrepresentable or misleading. The three dimensions below may combine freely.

### 1. Endpoint identity lifecycle (persistent record)

- **(no record)** — no enrollment attempt has been observed yet; not a persisted state.
- **PendingEnrollment** — a first-seen endpoint has completed the credential exchange but is not yet trusted. This is the M0 default path (ADR-0004: operator-approval-gated enrollment).
- **Enrolled** — an operator has approved the Endpoint. This is the durable trusted-identity state; it persists across reconnects, reboots, and credential renewal (see "Credential/session lifecycle") and is not re-derived on every connection.
- **Retired** — an operator has explicitly retired the Endpoint (e.g., decommissioned hardware); no further Jobs may target it.

**Future extension, not required in M0**: a **PreAuthorized** state, where an operator explicitly authorizes an enrollment context/token before the endpoint's first connection. On first successful connection with a valid pre-authorized token, the record moves directly to `Enrolled` without a separate post-connection approval step. This is still gated by an explicit prior operator action — authorizing before contact rather than approving after — and must not be implemented as, or conflated with, unrestricted automatic enrollment. Designing this mechanism is out of scope for M0.

Transitions:

- `(no record)` → `PendingEnrollment`: on first successful credential exchange with no matching prior record (M0 default path).
- `(no record)` → `Enrolled` *(future, not M0)*: on first successful connection that redeems a valid pre-authorized token.
- `PendingEnrollment` → `Enrolled`: explicit operator approval action only.
- `PendingEnrollment` → discarded: not approved (retention/cleanup policy is an implementation-time detail).
- `Enrolled` → `Retired`: explicit operator action only.
- `Enrolled` does **not** revert to `PendingEnrollment` on reconnect, reboot, or credential renewal — see "Reconnect / credential renewal handling."

### 2. Credential/session lifecycle (independent of identity lifecycle)

- **NoActiveCredential** — no runtime credential currently issued (e.g., Endpoint offline, or not yet past enrollment).
- **CredentialActive** — a runtime Agent identity/session credential is currently valid.
- **CredentialExpired** — the runtime credential's validity period has elapsed.
- **CredentialRevoked** — the runtime credential was explicitly invalidated (operator action, or Server-driven revocation, e.g. as a consequence of a `Conflict` hardware-confidence state).

This dimension cycles repeatedly and independently across an Endpoint's lifetime (every reconnect, every renewal) and does not by itself change the Endpoint identity lifecycle state. The concrete authentication mechanism that establishes `CredentialActive` is owned by the Agent control-protocol Work Package (Issue #3); this Specification only defines the resulting state, not the mechanism.

### 3. Hardware/identity-confidence state (continuously evaluated, not a lifecycle)

- **Consistent** — observed inventory signals match the Endpoint's recorded signals; no discrepancy.
- **LoweredConfidence** — a hardware change was observed (e.g., one of several NICs replaced) that the owner's direction requires surfacing for operator review, but that does not by itself indicate the Endpoint is a different physical device. Does not block reconnect, credential renewal, or destructive operations on its own.
- **Conflict** — a discrepancy serious enough that continuity of the trusted identity cannot be assumed. Blocks destructive operations (see precondition 6 below) until resolved.

This dimension can change at any time based on newly observed inventory signals, independent of the other two dimensions. It is resolved back to `Consistent` only through explicit operator review/confirmation — never automatically. The exact thresholds distinguishing a "significant" hardware change (`LoweredConfidence`) from a `Conflict` are implementation-time policy, intentionally not decided here (see "Open questions").

## Destructive-operation authorization preconditions

Before any destructive operation executes against an Endpoint, **all** of the following independent preconditions must hold. None may be inferred from another — in particular, `Enrolled` alone is never sufficient:

1. **Trusted persistent Endpoint identity** — identity-lifecycle dimension is `Enrolled` (not `PendingEnrollment`, not `Retired`).
2. **Authenticated current Agent session** — credential dimension is `CredentialActive`. The authentication mechanism itself is owned by Issue #3, not this Specification.
3. **Authorized Job/action** — the specific Job/action targeting this Endpoint has its own authorization. This precondition dimension is required here; what "authorized" means for a Job/action is owned by the Job lifecycle Work Package (Issue #4) and is not defined by this Specification.
4. **Sufficiently fresh inventory** — the inventory revision the operation was authorized against matches the Endpoint's current inventory revision.
5. **Target disk identity/fingerprint revalidation** — the target disk/volume identity/fingerprint matches what the operation was authorized against, revalidated immediately before execution.
6. **No unresolved identity or hardware-confidence conflict** — the confidence dimension is not `Conflict`.

Any of these failing must block the destructive operation and surface a clear reason — never a silent retry or silent override. This precondition set is normative for Issues #4 and #6, which should reference it directly rather than re-derive or narrow it to a single check.

## Reconnect / credential renewal handling

Owner decision: once an Endpoint has been explicitly enrolled, normal reconnects, reboots, and credential renewal must not require repeated operator approval when continuity of the trusted identity can be established.

- Continuity is established when the identity dimension is `Enrolled` and the confidence dimension is not `Conflict`. `LoweredConfidence` alone does not interrupt continuity or require re-approval — it surfaces for operator awareness and review without blocking reconnect or renewal. Whether a `LoweredConfidence` condition should ever escalate to `Conflict` automatically past some severity, or only through operator judgment, is implementation-time policy, not decided here.
- A reconnecting Agent still re-authenticates and redeems a fresh runtime credential each time (mechanism owned by Issue #3); this does not require re-running operator approval as long as continuity holds.
- A destructive command issued before a disconnect must never be blindly replayed on reconnect. Whether and how an interrupted destructive JobStep resumes is owned by the Job lifecycle Work Package (Issue #4); that Work Package must treat "Agent reconnected" and "destructive step may safely resume" as separate questions, both gated by the destructive-operation preconditions above.

## Out of scope

- exact confidence thresholds distinguishing `LoweredConfidence` from `Conflict` — implementation-time policy;
- the pre-authorized enrollment mechanism — future extension, not required in M0;
- Agent/Server mutual authentication mechanism (Issue #3);
- Job/action authorization semantics (Issue #4);
- Job/JobStep resumption semantics after reconnect (Issue #4) — this Specification defines only the precondition those semantics must satisfy;
- production enrollment UX/workflow implementation.

## Acceptance criteria

- Endpoint identity lifecycle, credential/session lifecycle, and hardware-confidence state are defined as independent dimensions, satisfying Issue #2's acceptance criterion for "a Specification defining the identity lifecycle" and correcting the earlier single-state-machine conflation identified during owner review.
- Destructive-operation authorization preconditions are independent, explicit, and not collapsed into a single `Enrolled` check.
- Reconnect/credential-renewal behavior matches the owner's continuity decision (no repeated approval when continuity holds).

## Validation expectations

Automated: none produced directly by this Work Package (decision/specification work). Once identity/enrollment is implemented, expected validation includes domain tests for each dimension's transitions independently (valid and rejected transitions, per `docs/development/testing.md` "Unit and domain tests"), tests covering combined states (e.g., `Enrolled` + `CredentialExpired` + `LoweredConfidence`), and negative cases demonstrating that a `Conflict` confidence state or a non-`Enrolled`/non-`CredentialActive` Endpoint rejects destructive operations. Per `docs/development/testing.md` "Local development environments," such domain and integration tests are expected to run in the Linux reference environment (WSL2 or containers from Windows), not asserted as correct from native-Windows execution alone.

Manual: owner approval of this Specification and ADR-0004 — both confirmed (see Status).

## Related ADRs

- ADR-0004 — Endpoint identity and enrollment/trust bootstrap model (`Accepted`; operator-approval-gated first enrollment is the M0 default).

## Related work

- Issue #2 — `[WP] Define endpoint identity and trust model`.
- Issue #3 — `[WP] Define Agent control and action contracts` (mutual-authentication mechanism establishing `CredentialActive`).
- Issue #4 — `[WP] Define Job lifecycle and scheduling model` (owns Job/action authorization semantics and destructive-step resumption; consumes the preconditions above).
- Issue #6 — `[WP] Define data-plane and storage contracts` (consumes the destructive-operation authorization preconditions).

## Open questions

1. Exact thresholds distinguishing `LoweredConfidence` from `Conflict`, and whether escalation between them can ever be automatic — implementation-time policy, not an M0 architectural blocker.
2. Exact credential TTL/renewal policy — implementation-time detail, intentionally left unresolved here.
3. Design of the future pre-authorized enrollment mechanism — explicitly not required for M0.

Status: Approved.
