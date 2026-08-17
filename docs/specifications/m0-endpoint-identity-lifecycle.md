# M0 — Endpoint Identity Lifecycle

Status: **Approved**

## Context

This Specification defines the Endpoint identity lifecycle, credential/session lifecycle, hardware-confidence state, and the destructive-operation authorization preconditions required by M0, executing Issue #2 (`[WP] Define endpoint identity and trust model`) together with ADR-0004 (Endpoint identity and enrollment/trust bootstrap model, `Accepted`).

Operator-approval-gated first enrollment is the accepted M0 default (ADR-0004): a newly observed device must not automatically become a trusted Endpoint merely because it can reach the Bamep Server.

## Identity model

- Durable Endpoint identity is a Server-assigned identifier, independent of MAC/hardware.
- Inventory signals (MAC, disk fingerprint, hardware serials when available) are evidence attached to an Endpoint record, never the identity itself (`AGENTS.md`; `docs/discovery/architecture-redesign.md` "Security invariants").

## State dimensions

An Endpoint's trust and operational readiness are governed by **three independent, Endpoint-owned state dimensions**, not one mutually exclusive machine. An earlier draft of this Specification collapsed persistent identity, credential/session validity, and hardware-signal confidence into a single state list (e.g., a `StaleHardwareSignal` state alongside `CredentialActive`); that conflated concerns that legitimately coexist — an `Enrolled` Endpoint can simultaneously have an expired credential and lowered hardware confidence, and forcing that into one mutually exclusive machine would make the combination unrepresentable or misleading. The three dimensions below may combine freely.

These three dimensions are all facts *about the Endpoint identity record itself*. **`trusted bootstrap established`** (`docs/decisions/0010-trusted-bootstrap-and-secure-boot-baseline.md`; destructive-operation precondition 7 below) is a different kind of fact — a security property of the **current boot/session context**, not a fourth Endpoint identity-lifecycle dimension, and it is not modeled or represented here as one. Its concrete representation/state machine is owned by the now-Approved trusted-bootstrap contract, `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` (Issue #13, ADR-0011), not by this Specification's three-dimension model.

### 1. Endpoint identity lifecycle (persistent record)

- **(no record)** — no enrollment attempt has been observed yet; not a persisted state.
- **PendingEnrollment** — a first-seen endpoint has completed the credential exchange but is not yet trusted. This is the M0 default path (ADR-0004: operator-approval-gated enrollment).
- **Enrolled** — an operator has approved the Endpoint. This is the durable trusted-identity state; it persists across reconnects, reboots, and credential renewal (see "Credential/session lifecycle") and is not re-derived on every connection.
- **Retired** — an operator has explicitly retired the Endpoint (e.g., decommissioned hardware); no further Jobs may target it.

The identity lifecycle intentionally has exactly these four states. Future pre-authorized enrollment (see "Future capability: pre-authorized enrollment" below) is an enrollment-authorization mechanism, modeled separately — it is not a fifth identity-lifecycle state.

Transitions:

- `(no record)` → `PendingEnrollment`: on first successful credential exchange with no matching prior record and no valid pre-authorization token redeemed (M0 default path).
- `(no record)` → `Enrolled`: on first successful credential exchange that redeems a valid, unexpired pre-authorization token (future capability, not required in M0; see below).
- `PendingEnrollment` → `Enrolled`: explicit operator approval action only.
- `PendingEnrollment` → discarded: not approved (retention/cleanup policy is an implementation-time detail).
- `Enrolled` → `Retired`: explicit operator action only.
- `Enrolled` does **not** revert to `PendingEnrollment` on reconnect, reboot, or credential renewal — see "Reconnect / credential renewal handling."

### 2. Credential/session lifecycle (independent of identity lifecycle)

- **NoActiveCredential** — no runtime credential currently issued (e.g., no runtime credential has yet been issued; a prior credential was explicitly removed and no replacement is active; or enrollment has not yet reached the point where a runtime credential exists). Being offline/disconnected is **not** an example of `NoActiveCredential` — current Agent presence (connectivity) and credential validity are independent facts (`docs/specifications/m0-administrative-api-web-read-contract.md` already establishes this for the `agent_presence` read representation): an Endpoint may hold a valid, unexpired `CredentialActive` credential while currently disconnected.
- **CredentialActive** — a runtime Agent identity/session credential is currently valid.
- **CredentialExpired** — the runtime credential's validity period has elapsed.
- **CredentialRevoked** — the runtime credential was explicitly invalidated (operator action, or Server-driven revocation, e.g. as a consequence of a `Conflict` hardware-confidence state).

This dimension cycles repeatedly and independently across an Endpoint's lifetime (every reconnect, every renewal) and does not by itself change the Endpoint identity lifecycle state. The concrete authentication mechanism that establishes `CredentialActive` is owned by the Agent control-protocol Work Package (Issue #3); this Specification only defines the resulting state, not the mechanism.

### 3. Hardware/identity-confidence state (continuously evaluated, not a lifecycle)

- **Consistent** — observed inventory signals match the Endpoint's recorded signals; no discrepancy. Required for destructive execution.
- **LoweredConfidence** — a hardware change was observed (e.g., one of several NICs replaced) that requires surfacing for operator review, but that does not by itself indicate the Endpoint is a different physical device. Permits normal connection, authentication (when otherwise valid), credential/session renewal, and non-destructive inventory or diagnostic activity. **Blocks destructive execution** (see precondition 6 below) until the confidence issue is resolved through operator review or explicit revalidation.
- **Conflict** — a discrepancy serious enough that continuity of the trusted identity cannot be assumed. Blocks destructive operations (see precondition 6 below), and — unlike `LoweredConfidence` — is treated as breaking continuity for reconnect/renewal purposes as well (see "Reconnect / credential renewal handling").

This dimension can change at any time based on newly observed inventory signals, independent of the other two dimensions. It is resolved back to `Consistent` only through explicit operator review/confirmation or explicit revalidation — never automatically. The exact thresholds distinguishing a "significant" hardware change (`LoweredConfidence`) from a `Conflict`, and the exact mechanics of "explicit revalidation," are implementation-time policy, intentionally not decided here (see "Open questions").

## Future capability: pre-authorized enrollment (not an identity-lifecycle state)

An operator may, in a future capability not required for M0, explicitly authorize an enrollment context/token before a specific endpoint's first connection. This is an **enrollment-authorization mechanism**, conceptually separate from and prior to the Endpoint identity lifecycle — it is not a state an Endpoint identity occupies.

A pre-authorization artifact has its own minimal lifecycle (e.g., issued → redeemed → expired/revoked) scoped to the authorization context/token itself, not to any Endpoint identity record — no Endpoint identity record exists yet when the token is issued.

When a booting endpoint successfully redeems a valid, unexpired pre-authorization token during the enrollment/credential exchange (ADR-0004), the resulting Endpoint identity record is created directly in the `Enrolled` state, skipping `PendingEnrollment`, because the required operator approval already happened — before contact instead of after.

This capability must not be implemented as, or conflated with, unrestricted automatic enrollment: it still requires an explicit prior operator action per endpoint or per authorization context. Its concrete design (token format, scope, expiry, issuance UX) is out of scope for M0.

## Destructive-operation authorization preconditions

Before any destructive operation executes against an Endpoint, **all** of the following independent preconditions must hold. None may be inferred from another — in particular, `Enrolled` alone is never sufficient:

1. **Trusted persistent Endpoint identity** — identity-lifecycle dimension is `Enrolled` (not `PendingEnrollment`, not `Retired`).
2. **Authenticated current Agent session** — credential dimension is `CredentialActive`. The authentication mechanism itself is owned by Issue #3, not this Specification.
3. **Authorized Job/action** — the specific Job/action targeting this Endpoint has its own authorization. This precondition dimension is required here; what "authorized" means for a Job/action is owned by the Job lifecycle Work Package (Issue #4) and is not defined by this Specification.
4. **Sufficiently fresh inventory** — the inventory revision the operation was authorized against matches the Endpoint's current inventory revision.
5. **Target disk identity/fingerprint revalidation** — the target disk/volume identity/fingerprint matches what the operation was authorized against, revalidated immediately before execution.
6. **Hardware confidence is sufficiently trusted** — the confidence dimension is `Consistent`. Both `LoweredConfidence` and `Conflict` block destructive execution until the confidence issue has been resolved through explicit operator review or revalidation; for this precondition the two levels are not treated differently, even though they differ for reconnect, renewal, and non-destructive activity (see "Hardware/identity-confidence state" and "Reconnect / credential renewal handling" above).
7. **Trusted current bootstrap context** — the current Agent boot/session must be anchored in a bootstrap context whose integrity/authenticity has been established, per `docs/decisions/0010-trusted-bootstrap-and-secure-boot-baseline.md`'s `trusted bootstrap established` security property (Secure Boot is the V1 baseline mechanism for the executable boot-chain integrity that property depends on). This is independent of precondition 2: a valid, active Agent credential proves the Agent authenticated successfully over the current session — it does not prove that the boot path leading to this session was itself trusted, since credential issuance and boot-chain trust are established by different mechanisms at different times. This Specification does not name this precondition `SecureBootEnabled`, does not require Domain code to inspect firmware state, and does not define the concrete representation/state machine for the trusted-bootstrap fact — that belongs to `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` (Issue #13, `Approved`; ADR-0011), consistent with ADR-0010 "Related work"; this precondition only establishes that the fact must exist and must gate destructive execution.

Any of these failing must block the destructive operation and surface a clear reason — never a silent retry or silent override. This seven-item precondition set is normative for Issues #4 and #6, which reference it directly rather than re-deriving or narrowing it to a single check. **Precondition 7 was added by ADR-0010** and is already fully composed: `docs/specifications/m0-job-lifecycle-and-scheduling.md` "Destructive dispatch preconditions" lists and revalidates all seven preconditions before destructive dispatch, and `docs/specifications/m0-data-plane-and-storage-contracts.md` treats its Artifact-specific gates as additive to this complete set, including precondition 7, without duplicating it. No follow-up amendment for this alignment remains open.

## Reconnect / credential renewal handling

Owner decision: once an Endpoint has been explicitly enrolled, normal reconnects, reboots, and credential renewal must not require repeated operator approval when continuity of the trusted identity can be established.

- Continuity, for reconnect/renewal purposes, is established when the identity dimension is `Enrolled` and the confidence dimension is not `Conflict`. `LoweredConfidence` alone does not interrupt this continuity or require re-approval — it surfaces for operator awareness and review without blocking reconnect, renewal, authentication, or non-destructive activity. Whether a `LoweredConfidence` condition should ever escalate to `Conflict` automatically past some severity, or only through operator judgment, is implementation-time policy, not decided here.
- Continuity for reconnect/renewal is evaluated separately from eligibility for destructive execution: `LoweredConfidence` does not block reconnect/renewal, but — like `Conflict` — it does block destructive operations under precondition 6 above until resolved.
- A reconnecting Agent still re-authenticates and redeems a fresh runtime credential each time (mechanism owned by Issue #3); this does not require re-running operator approval as long as continuity holds.
- A destructive command issued before a disconnect must never be blindly replayed on reconnect. Whether and how an interrupted destructive JobStep resumes is owned by the Job lifecycle Work Package (Issue #4); that Work Package must treat "Agent reconnected" and "destructive step may safely resume" as separate questions, both gated by the destructive-operation preconditions above.

## Out of scope

- exact confidence thresholds distinguishing `LoweredConfidence` from `Conflict`, and the exact mechanics of "explicit revalidation" — implementation-time policy;
- the pre-authorized enrollment mechanism (token format, scope, expiry, issuance UX) — future capability, not required in M0;
- Agent/Server mutual authentication mechanism (Issue #3);
- Job/action authorization semantics (Issue #4);
- Job/JobStep resumption semantics after reconnect (Issue #4) — this Specification defines only the precondition those semantics must satisfy;
- production enrollment UX/workflow implementation.

## Acceptance criteria

- Endpoint identity lifecycle, credential/session lifecycle, and hardware-confidence state are defined as independent dimensions, satisfying Issue #2's acceptance criterion for "a Specification defining the identity lifecycle" and correcting the earlier single-state-machine conflation identified during owner review.
- The Endpoint identity lifecycle contains exactly four states (`(no record)`, `PendingEnrollment`, `Enrolled`, `Retired`); pre-authorized enrollment is modeled as a separate future enrollment-authorization mechanism, not a fifth identity state.
- Destructive-operation authorization preconditions are independent, explicit, and not collapsed into a single `Enrolled` check; both `LoweredConfidence` and `Conflict` block destructive execution; trusted-bootstrap context (precondition 7) is independent of credential validity — a valid Agent credential does not prove the current boot path was itself trusted.
- Reconnect/credential-renewal behavior matches the owner's continuity decision (no repeated approval when continuity holds, and `LoweredConfidence` alone does not interrupt continuity even though it blocks destructive execution).

## Validation expectations

Automated: none produced directly by this Work Package (decision/specification work). Once identity/enrollment is implemented, expected validation includes domain tests for each dimension's transitions independently (valid and rejected transitions, per `docs/development/testing.md` "Unit and domain tests"), tests covering combined states (e.g., `Enrolled` + `CredentialExpired` + `LoweredConfidence`), and negative cases demonstrating that a `Conflict` confidence state or a non-`Enrolled`/non-`CredentialActive` Endpoint rejects destructive operations. Future negative tests must also cover destructive-operation rejection when trusted bootstrap (precondition 7) is not established, including the case of an otherwise fully valid `Enrolled` + `CredentialActive` + `Consistent` Endpoint whose current session cannot be shown to have a trusted bootstrap context — destructive execution must still be blocked. Per `docs/development/testing.md` "Local development environments," such domain and integration tests are expected to run in the Linux reference environment (WSL2 or containers from Windows), not asserted as correct from native-Windows execution alone.

Manual: owner approval of this Specification and ADR-0004 — both confirmed (see Status).

## Related ADRs

- ADR-0004 — Endpoint identity and enrollment/trust bootstrap model (`Accepted`; operator-approval-gated first enrollment is the M0 default).
- ADR-0010 — Trusted bootstrap and Secure Boot baseline (`Accepted`) — source of precondition 7 (`trusted bootstrap established`).
- ADR-0011 — V1 site trust-anchor establishment and operator-verified first-key pairing (`Accepted`) — resolves the concrete representation/state machine for the trusted-bootstrap fact this Specification's precondition 7 depends on.

## Related work

- Issue #2 — `[WP] Define endpoint identity and trust model`.
- Issue #3 — `[WP] Define Agent control and action contracts` (mutual-authentication mechanism establishing `CredentialActive`).
- Issue #4 — `[WP] Define Job lifecycle and scheduling model` (owns Job/action authorization semantics and destructive-step resumption; already composes and revalidates the full seven-item precondition set above).
- Issue #6 — `[WP] Define data-plane and storage contracts` (already treats its Artifact-specific gates as additive to the full seven-item precondition set above, without duplicating it).
- Issue #10 / ADR-0010 — `[Spike] Validate Secure Boot and hardened boot chain` (complete; source of precondition 7).
- Issue #13 / ADR-0011 — `[WP] Define trusted bootstrap and Server fingerprint delivery contract` (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`, `Approved`; complete) — defines the concrete representation/state machine for precondition 7 that this Specification leaves to that contract.

## Open questions

The concrete representation/state machine for the trusted-bootstrap fact (precondition 7) is no longer tracked as open here: it is resolved by `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` (Issue #13, `Approved`) and ADR-0011.

1. Exact thresholds distinguishing `LoweredConfidence` from `Conflict`, whether escalation between them can ever be automatic, and the exact mechanics of "explicit revalidation" — implementation-time policy, not an M0 architectural blocker.
2. Exact credential TTL/renewal policy — implementation-time detail, intentionally left unresolved here.
3. Design of the future pre-authorized enrollment mechanism (token format, scope, expiry, issuance UX) — explicitly not required for M0.

Status: Approved.
