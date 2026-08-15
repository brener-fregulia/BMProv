# M0 — Simulator Contract and Validation Strategy

Status: **Proposed - awaiting owner approval**

## Context

This Specification defines the Bamep Simulator's behavioral contract and the M0 validation strategy, executing Issue #7 (`[WP] Define Simulator contract and M0 validation strategy`). It does not implement the Simulator or the post-M0 vertical slice; it defines what the Simulator must represent, which scenarios it must support, and which validation layer (`docs/development/testing.md`) applies to which M0 behavior, so the vertical slice described in `docs/specifications/m0-architecture-baseline.md` ("First implementation slice after M0") can be built and validated without physical hardware.

It draws on the already-approved contracts from Issues #2–#6 (Endpoint identity, Agent Protocol, Job lifecycle/scheduling, persistence/observability, data-plane/storage) without duplicating their content — each required scenario below links to the Specification section it validates rather than restating its reasoning.

## Goal

Define the Simulator's fidelity boundary, required scenarios, concurrency target, and the mapping from M0 domain behavior to test layer, so no required architectural decision remains hidden inside the first post-M0 implementation Work Package (`docs/specifications/m0-architecture-baseline.md` acceptance criterion 7).

## Scope

- Simulated Endpoint behavioral contract;
- Simulator fidelity boundary (what is simulated at protocol-level realism vs. faked at the port/adapter level);
- required scenarios: enrollment/inventory/job/action/transfer/reconnect, and the failure-scenario list from Issue #7;
- concurrency target: 20–24 concurrent Simulated Endpoints (`docs/specifications/m0-architecture-baseline.md`);
- the persistence-load validation this Specification's scope requires per `docs/specifications/m0-persistence-observability-and-domain-events.md` / ADR-0007;
- mapping of M0 domain behavior to the test-layer model in `docs/development/testing.md`;
- what the Simulator cannot represent and must defer to the Integration Environment or owner manual validation;
- the contract required by the first implementation vertical slice after M0.

## Out of scope

- actual Simulator or vertical-slice implementation code (Issue #7's own stated out-of-scope);
- Simulator internal implementation architecture — process/thread model, crate/module structure, concrete configuration format — implementation-time detail, not an M0 architectural question;
- concrete numeric acceptance thresholds for the persistence-load measurement (write latency/contention limits) — these depend on an actual implementation to observe, and are established when the first vertical slice runs the scenario (see "Persistence-load validation" below), not invented here without evidence;
- production provisioning implementation, real disk formatting, real Windows installation, WinPE implementation, MikroTik-specific production adapter — all already excluded from M0 (`docs/specifications/m0-architecture-baseline.md` "Out of scope");
- Administrative API / Web contract — not yet a dedicated M0 Work Package (see "Forward dependency: Web" below).

## Simulator fidelity boundary

`docs/discovery/architecture-redesign.md` ("Development architecture") already distinguishes, and this Specification restates as the M0 baseline: **simulated agents**, distinct from **fake boot, discovery, and storage adapters**. This wording distinguishes two different fidelity levels used by the same Simulator:

- **Simulated Endpoints/Agents** — participate in real Bamep Server-side behavior as realistically as an M0 environment allows: real Endpoint identity/enrollment flow, real Agent Protocol v1 message exchange, real Job/JobStep/Attempt orchestration, real scheduler/resource-lease contention, real persistence and reconciliation. Only the underlying physical device is not real.
- **Faked boundaries** — boot mechanism (PXE/DHCP/UEFI/GRUB/WinPE), discovery/inventory hardware probing, and storage devices are stubbed with deterministic fixtures and temporary local storage (`docs/development/testing.md` "Fakes and test boundaries"; "Destructive-operation safety"). These are hardware/OS-boundary concerns the Simulator does not need to represent faithfully to validate Bamep's own orchestration logic.

**Proposed, not yet separately owner-confirmed**: a Simulated Endpoint's Agent-side participant connects to the real Server-side Agent Control Gateway over the real Agent Protocol v1 transport (WSS, `docs/specifications/m0-agent-protocol-contract.md`), rather than substituting an in-process fake transport that only approximates the protocol's message contract. Rationale: several required scenarios (duplicate/delayed messages, Agent restart, disconnect/reconnect, acknowledgment timeout) are, per the Agent Protocol Specification itself, timing- and delivery-outcome-sensitive behaviors of the real transport — an in-process fake transport risks silently deciding delivery/timing semantics the real transport does not guarantee, which is exactly the kind of hidden architectural decision `docs/development/sdd.md` requires to be surfaced rather than assumed. `docs/development/testing.md` "Fakes and test boundaries" separately allows "Agent connections" as a fake boundary for Component/Integration-layer tests below the Simulator layer, which remains available for narrower tests that are not exercising Simulator-level orchestration realism; this does not conflict with the Simulator itself using the real transport for its own scenarios. This point determines Simulator/Server coupling architecture and is flagged for explicit owner confirmation rather than asserted as already decided.

The Simulator does not replace physical integration testing for hardware-specific behavior (`docs/development/testing.md` "Simulator").

## Simulated Endpoint contract

A Simulated Endpoint provides, at minimum, the configurable characteristics already required by `docs/development/testing.md` ("Simulator"): connection and reconnection; latency; throughput; CPU constraints; storage characteristics; operation duration; failures; retries; interruptions; inventory changes; storage pressure.

Its behavioral surface follows the accepted contracts:

- **Identity/enrollment** — a Simulated Endpoint presents configurable inventory signals (MAC, disk fingerprint, hardware serials) and exercises the `(no record)` → `PendingEnrollment` → `Enrolled` flow, including operator-approval-gated enrollment as the M0 default (`docs/specifications/m0-endpoint-identity-lifecycle.md`).
- **Agent session** — a Simulated Endpoint's Agent participant performs the real handshake (TLS fingerprint verification, `AuthRequest`/`SessionEstablished`/`AuthError`) and credential/session lifecycle transitions (`docs/specifications/m0-agent-protocol-contract.md`).
- **Inventory reporting** — configurable inventory content and change frequency, exercising the write-on-change-only durable boundary (`docs/specifications/m0-persistence-observability-and-domain-events.md` "Inventory persistence boundary").
- **Action execution** — accepts `ActionDispatch` for whichever `action_type`s the exercised scenario requires and responds through the full Agent-action state vocabulary (`Accepted`/`Running`/`Succeeded`/`Failed`/`Cancelled`/`Unknown`), including configurable delay, failure injection, and simulated restart (loss of local action state).
- **Data-plane participation** — produces or consumes chunk manifests against simulated source bytes held in temporary local storage, including configurable chunk-level digest mismatch, chunk loss, and source mutation between chunk identification and (re)transfer, to exercise `docs/specifications/m0-data-plane-and-storage-contracts.md`.

## Required scenarios

The scenario categories below are the failure-scenario list from Issue #7's approved scope, mapped to the Specification each validates. This Specification does not restate each target Specification's own reasoning — see the linked section for the accepted behavior being exercised.

| Scenario | Validates |
|---|---|
| Duplicate/delayed messages | Agent Protocol idempotency and duplicate-`ActionDispatch` handling (`m0-agent-protocol-contract.md` "Idempotency and duplicate handling") |
| Stale inventory | Destructive-operation precondition 4 (`m0-endpoint-identity-lifecycle.md` "Destructive-operation authorization preconditions") |
| Endpoint disappearance | Attempt `AwaitingReconciliation`/`Indeterminate` (`m0-job-lifecycle-and-scheduling.md` "Attempt lifecycle", "Reconciliation procedure") |
| Agent restart | `StatusQuery`/`Unknown` handling (`m0-agent-protocol-contract.md` "Reconnect / stale-command handling"); Attempt reconciliation (`m0-job-lifecycle-and-scheduling.md`) |
| Server restart (where supported) | Reconciliation procedure on restart, `Dispatched`/`InProgress` → `AwaitingReconciliation` (`m0-job-lifecycle-and-scheduling.md`) |
| Scheduler contention | Resource-lease acquisition/contention, Job-scoped endpoint exclusivity (`m0-job-lifecycle-and-scheduling.md` "Endpoint-exclusivity lease", "Other resource leases") |
| Resource exhaustion | Attempt-scoped storage/network/CPU lease behavior under exhaustion (`m0-job-lifecycle-and-scheduling.md`); storage-target capacity (`m0-data-plane-and-storage-contracts.md` "Storage capability model") |
| Partial failure | JobStep `Failed`; Artifact atomic integrity/completeness unit — no partial success within one Artifact (`m0-data-plane-and-storage-contracts.md` "Artifact lifecycle") |
| Cancellation | Job `Cancelling` → `Cancelled`, `CancelAction`/`CancelAck` handling (`m0-job-lifecycle-and-scheduling.md`) |
| Recovery after interruption | Chunk resume (already-held chunks skipped) and source-mutation failure (`m0-data-plane-and-storage-contracts.md` "Chunk transfer", "Source reproducibility"); `AwaitingReconciliation` recovery (`m0-job-lifecycle-and-scheduling.md`) |

Additional data-plane-specific Simulator scenarios already required by `docs/specifications/m0-data-plane-and-storage-contracts.md` "Validation expectations" (Simulator) apply directly and are not restated here: chunked transfer at the concurrency target with interrupted/corrupted-chunk scenarios; a simulated source-mutation scenario reproducing `docs/reference/transfer-resumability-spike.md` Experiment E; rejection of a destructive JobStep when `capture_consistency` is `NotEstablished` even if the Artifact is `Verified`; a simulated disk-replacement scenario (source Artifact provenance differing from the destructive target's disk identity) succeeding without requiring the two to match.

## Concurrency target

The Simulator must support a scenario with 20–24 concurrent Simulated Endpoints (`docs/specifications/m0-architecture-baseline.md` "First implementation slice after M0"), consistent with the High-density installation profile (`docs/discovery/architecture-redesign.md` "Capacity and scheduling"). Smaller counts (Small/Medium profiles, 3–10 endpoints) remain useful for faster-iterating scenario development and are not excluded — the 20–24 target is the minimum ceiling the Simulator must reach for M0 acceptance, not the only supported scale.

Scenarios requiring this concurrency target specifically: scheduler contention (`m0-job-lifecycle-and-scheduling.md` "Validation expectations" — Simulator); the persistence-load validation below; the data-plane chunked-transfer scenario at scale (`m0-data-plane-and-storage-contracts.md`).

## Persistence-load validation

`docs/specifications/m0-persistence-observability-and-domain-events.md` requires, per ADR-0007, that "the Simulator/validation strategy must exercise representative persistence load at the M0 20–24 concurrent-endpoint target, measuring actual durable write volume, contention, latency, and backpressure against the expectation in ADR-0007, before the M0 architecture is considered validated," and states this as "a requirement on Issue #7's scope."

This Specification defines that requirement as a required Simulator scenario category: sustained, concurrent Job/JobStep/Attempt activity across 20–24 Simulated Endpoints, generating a realistic durable-write pattern (state transitions, domain events, audit records, inventory-on-change, artifact/manifest metadata — per the durable/transient boundary in `m0-persistence-observability-and-domain-events.md`), with the resulting write volume, contention, latency, and backpressure measured and recorded.

Concrete acceptable thresholds are **not** defined by this Specification — no implementation exists yet to derive them from, and inventing numeric limits without evidence would repeat the mistake this session has consistently avoided elsewhere (see, e.g., ADR-0008's deferred `digest_algorithm` selection). Thresholds are established when the first post-M0 implementation vertical slice actually runs this scenario and observed behavior is evaluated against ADR-0007's expectation; if the measurement shows unacceptable results, ADR-0007 must be revisited (per its own text), not silently patched.

## Automated validation boundary

`docs/development/testing.md` "Test layers" already defines the general layer model (Unit/Domain → Contract → Component/Integration → Simulator → Integration Environment → Owner manual validation). This table applies that general model to the M0 domain concepts already specified, so no M0 Work Package leaves its own validation layer undecided:

| Layer | M0 behavior it validates |
|---|---|
| Unit / Domain | Endpoint identity/credential/confidence transitions; Job/JobStep/Attempt transitions; resource-lease acquisition/release; Artifact lifecycle transitions; chunk-manifest verification logic; domain-event emission; inventory write-on-change (each already itemized in its own Specification's "Validation expectations") |
| Contract | Agent Protocol v1 message serialization, versioning, and error handling (`m0-agent-protocol-contract.md`); domain-event schema/versioning (`m0-persistence-observability-and-domain-events.md`) |
| Component / Integration | Persistence + domain-state atomicity; scheduler + resource leases; Agent session management against a real or faked transport (`docs/development/testing.md` "Fakes and test boundaries" permits faking Agent connections at this layer specifically, distinct from the Simulator layer above) |
| Simulator | Realistic multi-endpoint orchestration at the 20–24 concurrency target; the required-scenario table above; persistence-load validation |
| Integration Environment | See "What the Simulator cannot represent" below |
| Owner manual validation | Acceptance of this Specification and every M0 Specification/ADR it depends on; final acceptance of the first post-M0 vertical slice once implemented |

## What the Simulator cannot represent

Per `docs/development/testing.md` "Integration Environment", the following remain deferred to the physical Bamep laboratory or owner manual validation, and are not claimed as Simulator-covered by this Specification: PXE; DHCP behavior; UEFI firmware; GRUB; Alpine boot; physical NIC behavior; MikroTik integration; real disk tooling; Windows deployment; WinPE; hardware-specific compatibility; destructive end-to-end provisioning.

Three explicitly isolated Technical Spikes feed, but are not resolved by, this Specification: the WinPE boot mechanism (Issue #8), Secure Boot / hardened boot chain (Issue #10), and driver-provider integration (Issue #11). Their results may later require the Simulator's fidelity boundary to be revisited (for example, if Issue #10 finds the Agent Protocol Server-fingerprint delivery mechanism must change, per `m0-agent-protocol-contract.md`), but none of the three block this Specification's own scope.

## Forward dependency: Web

The first implementation vertical slice (`docs/specifications/m0-architecture-baseline.md`) ends with "Web reflects result." No Administrative API or Web Specification exists yet as a dedicated M0 Work Package — several already-approved Specifications note operator-identity/authentication and Administrative API design as unresolved (`m0-persistence-observability-and-domain-events.md` "Out of scope"; `m0-agent-protocol-contract.md` "Open questions"). This Specification records that the vertical slice's final step has an open contract dependency on future Web/API work; it does not design that contract and does not block on it, since the Simulator scenarios above validate Server-side orchestration independent of how Web later consumes it.

## Acceptance criteria

- A Specification defines the Simulator contract, required scenarios, and the M0 validation strategy (Issue #7 acceptance criterion 1) — satisfied by this document.
- The simulated vertical slice has defined behavior, contracts, and failure scenarios (Issue #7 acceptance criterion 2; `docs/specifications/m0-architecture-baseline.md` acceptance criterion 4) — satisfied by the "Simulated Endpoint contract" and "Required scenarios" sections.
- Automated validation boundaries are explicit per M0 concept (`docs/specifications/m0-architecture-baseline.md` acceptance criterion 6) — satisfied by "Automated validation boundary."
- The persistence-load validation required by ADR-0007 is defined as an explicit Simulator scenario, not silently left unaddressed.
- No required architectural decision is hidden inside the first post-M0 implementation Work Package (`docs/specifications/m0-architecture-baseline.md` acceptance criterion 7) — the one genuine open fork identified (Simulator/Server transport coupling) is flagged explicitly for owner decision rather than assumed.

## Validation expectations

Automated: none produced directly by this Work Package — it is decision/specification work, consistent with Issue #7's own stated validation ("this Work Package defines the validation strategy other work will implement against").

Manual: owner approval of this Specification.

## Related ADRs

No new ADR is introduced by this Work Package. This Specification consolidates and applies the destructive-operation, protocol, scheduling, persistence, and data-plane decisions already `Accepted` in ADR-0004 through ADR-0008; it does not itself establish a new durable architectural boundary with meaningful alternatives requiring a separate ADR (`docs/development/documentation-policy.md` "Architectural Decision Records"), aside from the one flagged open fork above, which the owner may resolve directly in this Specification without a dedicated ADR unless the resulting decision proves durable and contested enough to warrant one.

## Related work

- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy`.
- Issues #2–#6 and their ADRs/Specifications — consumed directly (see table above).
- Issue #8 — `[Spike] Validate WinPE boot mechanism` (feeds, does not block).
- Issue #10 — `[Spike] Validate Secure Boot and hardened boot chain` (feeds, does not block; may affect Agent Protocol fingerprint delivery).
- Issue #11 — `[Spike] Evaluate driver-provider integration` (feeds, does not block).

## Open questions

1. Whether Simulated Agents connect over the real Agent Protocol v1 transport (WSS) or an in-process fake transport approximating the same message contract — proposed direction stated above ("Simulator fidelity boundary"), not yet separately owner-confirmed.
2. Concrete persistence-load acceptance thresholds — established when the first post-M0 vertical slice actually runs the measurement; not decided here.
3. Simulator internal implementation architecture (process/thread model, configuration format, crate/module structure) — implementation-time, not an M0 architectural question.
4. The Administrative API / Web contract for the vertical slice's final "Web reflects result" step — not yet a dedicated M0 Work Package.

Status: Proposed - awaiting owner approval.
