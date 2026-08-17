# M0 — Simulator Contract and Validation Strategy

Status: **Approved**

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
- Administrative API / Web contract design — owned by `docs/specifications/m0-administrative-api-web-read-contract.md` (Issue #12, `Approved`), not this Specification (see "Administrative API / Web contract" below).

## Simulator fidelity boundary

`docs/discovery/architecture-redesign.md` ("Development architecture") already distinguishes, and this Specification restates as the M0 baseline: **simulated agents**, distinct from **fake boot, discovery, and storage adapters**. This wording distinguishes two different fidelity levels used by the same Simulator:

- **Simulated Endpoints/Agents** — participate in real Bamep Server-side behavior as realistically as an M0 environment allows: real Endpoint identity/enrollment flow, real Agent Protocol v1 message exchange, real Job/JobStep/Attempt orchestration, real scheduler/resource-lease contention, real persistence and reconciliation. Only the underlying physical device is not real.
- **Faked boundaries** — boot mechanism (PXE/DHCP/UEFI/GRUB/WinPE), discovery/inventory hardware probing, and storage devices are stubbed with deterministic fixtures and temporary local storage (`docs/development/testing.md` "Fakes and test boundaries"; "Destructive-operation safety"). These are hardware/OS-boundary concerns the Simulator does not need to represent faithfully to validate Bamep's own orchestration logic.

**Accepted (owner decision)**: at the Simulator level, a Simulated Endpoint's Agent-side participant MUST use the real Agent Protocol v1 transport path end-to-end — a real WSS connection to the real Server-side Agent Control Gateway, real Agent Protocol v1 UTF-8 JSON serialization, the real protocol handshake and credential validation, and real reconnect/disconnect behavior at the transport boundary. An in-process fake Agent transport does not satisfy Simulator-level acceptance for any scenario in the "Required scenarios" table below.

This does not change `docs/development/testing.md` "Fakes and test boundaries", which separately and explicitly permits faking Agent connections for narrower Unit/Component/Integration tests where the real transport is not the behavior under test — that remains available below the Simulator layer; it is only Simulator-level scenarios that require the real transport.

For Simulator execution, the production boot chain itself remains faked (per "Faked boundaries" above) — this includes production PXE/UEFI/Secure Boot, which the Simulator does not exercise. The Simulator may receive deterministic fixture material substituting for the result of that boundary: `trusted bootstrap established` (`docs/decisions/0010-trusted-bootstrap-and-secure-boot-baseline.md`), plus whatever authenticated Server fingerprint / enrollment context the trusted-bootstrap contract requires. This fixture substitution must not be represented as validating the production Secure-Boot-backed trusted-bootstrap mechanism itself. Issue #10 is complete and produced ADR-0010 (`trusted bootstrap established` accepted as the V1 baseline security property); the production fingerprint-delivery mechanism is not "owned by Issue #10" — it is owned by the now-Approved dedicated M0 trusted-bootstrap contract, `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` (Issue #13, `Approved`; ADR-0011). The concrete fixture field/schema/token/manifest representation is **not** defined by this Specification — that dedicated contract (Section 8, "Simulator contract") defines the semantic fixture contract (what production fact the fixture substitutes for); Simulator implementation later chooses only its concrete implementation/configuration technique within that contract (see "What the Simulator cannot represent").

Given the fixture boundary above, the Simulator must still exercise real Agent Protocol behavior after it, including at minimum:

- valid pinned Server fingerprint (successful TLS Server-authentication);
- fingerprint mismatch failing closed before Agent Protocol authentication begins (`m0-agent-protocol-contract.md` "Transport and handshake" — a connection-level abort, never an `AuthError`);
- valid and rejected Agent credentials (`SessionEstablished` vs. `AuthError`);
- reconnect (fresh handshake after disconnect, `StatusQuery` on any Attempt the Server still considers in-flight, per `m0-agent-protocol-contract.md` "Reconnect / stale-command handling");
- uncertain delivery / acknowledgment timeout scenarios where applicable (`m0-agent-protocol-contract.md` "Acknowledgment timeout semantics").

Fault injection (delay, duplicate messages, disconnect, restart, etc.) may be controlled by the Simulated Agent or the test harness driving it, but the resulting Simulator-level scenario must still cross the real WSS/Agent Control Gateway boundary — fault injection controls timing and sequencing, it does not substitute for the transport itself.

This is a Simulator fidelity decision and does not require a separate ADR: it does not establish a new durable architectural boundary beyond what ADR-0005 and `docs/specifications/m0-agent-protocol-contract.md` already define — it decides how the Simulator exercises that existing contract, not what the contract is.

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

**Required trusted-bootstrap independence scenario (added following ADR-0010):** a destructive dispatch must be rejected when an Endpoint is otherwise `Enrolled`, credential is `CredentialActive`, inventory is fresh, target disk is valid, hardware confidence is `Consistent`, and workflow/scheduler authorization holds, but trusted bootstrap (`docs/specifications/m0-endpoint-identity-lifecycle.md` precondition 7) is not established — destructive dispatch must never occur in this case. This scenario validates precondition 7's independence from the other six preconditions, and specifically from `CredentialActive` (precondition 2): a valid credential must never be treated as proof that the current boot path was itself trusted.

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

Three Technical Spikes previously listed as explicitly isolated are now complete, each feeding this Specification without resolving or narrowing it:

- **WinPE boot mechanism (Issue #8)** — complete; WinPE UEFI boot viability is established, but the network-delivered boot mechanism remains isolated as future Integration Environment validation work (`docs/reference/winpe-boot-mechanism-spike.md`).
- **Secure Boot / hardened boot chain (Issue #10)** — complete; produced ADR-0010's `trusted bootstrap established` baseline (`docs/reference/secure-boot-hardened-chain-spike.md`).
- **Driver-provider integration (Issue #11)** — complete; produced ADR-0009's operator-managed driver-repository boundary (`docs/reference/driver-provisioning.md`).

Their remaining production/hardware concerns still belong to the Integration Environment — the network-delivered boot mechanism for Issue #8 is the one still-isolated follow-up item; Issue #10's own follow-up contract (trusted-bootstrap/Server-fingerprint-delivery) is no longer outstanding, having been completed by Issue #13/ADR-0011 (see "Simulator fidelity boundary" above). None of the three block this Specification's own scope, and none require the Simulator's fidelity boundary above to change. The Simulator does not validate real Secure Boot, real PXE/network-delivered boot, or real driver injection — these remain Integration Environment concerns per "What the Simulator cannot represent" above.

## Administrative API / Web contract

The first implementation vertical slice (`docs/specifications/m0-architecture-baseline.md` "First implementation slice after M0") ends with "Web reflects result." When this Specification was first drafted, no Administrative API or Server↔Web contract had been specified by any M0 Work Package, and that absence was recorded here as an M0 architecture-planning gap.

**That gap is resolved.** It was materialized as Issue #12 (`[WP] Define minimum Administrative API and Web read contract`), which is complete: `docs/specifications/m0-administrative-api-web-read-contract.md` is `Approved` and is the authoritative minimum Server↔Web read contract for the first vertical slice — defining the Server-side query/read surface (Endpoint status, Job/JobStep/Attempt progress and terminal outcome, etc.) needed for Web to observe the slice's result. The vertical slice's final step, "Web reflects result," now has an explicit contract to implement against and is no longer an architectural gap.

This Specification does not design or duplicate that contract's details — it remains fully owned by `docs/specifications/m0-administrative-api-web-read-contract.md`. Authentication and command/write semantics were intentionally left out of that contract's scope and remain future concerns (per that Specification's own "Unresolved findings"), but they do not block or narrow the minimum read contract already approved, and they do not reopen this gap. The Simulator scenarios defined above validate Server-side orchestration independent of how Web consumes it, and remain valid regardless of Administrative API implementation timing.

## Acceptance criteria

- A Specification defines the Simulator contract, required scenarios, and the M0 validation strategy (Issue #7 acceptance criterion 1) — satisfied by this document.
- The simulated vertical slice has defined behavior, contracts, and failure scenarios (Issue #7 acceptance criterion 2; `docs/specifications/m0-architecture-baseline.md` acceptance criterion 4) — satisfied by the "Simulated Endpoint contract" and "Required scenarios" sections.
- Automated validation boundaries are explicit per M0 concept (`docs/specifications/m0-architecture-baseline.md` acceptance criterion 6) — satisfied by "Automated validation boundary."
- The persistence-load validation required by ADR-0007 is defined as an explicit Simulator scenario, not silently left unaddressed.
- No required architectural decision is hidden inside the first post-M0 implementation Work Package (`docs/specifications/m0-architecture-baseline.md` acceptance criterion 7) — the Simulator/Server transport coupling fork identified during drafting was resolved by explicit owner decision (see "Simulator fidelity boundary"); the Administrative API/Web contract gap identified during drafting was not hidden, was materialized as Issue #12, and is now resolved by the approved `docs/specifications/m0-administrative-api-web-read-contract.md` (see "Administrative API / Web contract").

## Validation expectations

Automated: none produced directly by this Work Package — it is decision/specification work, consistent with Issue #7's own stated validation ("this Work Package defines the validation strategy other work will implement against").

Manual: owner approval of this Specification — confirmed (see Status).

## Related ADRs

No new ADR is introduced by this Work Package. This Specification consolidates and applies the destructive-operation, protocol, scheduling, persistence, data-plane, driver-provider, and trusted-bootstrap decisions already `Accepted` in ADR-0004 through ADR-0011, including the Simulator-transport fidelity decision recorded above, which does not itself establish a new durable architectural boundary beyond what ADR-0005 already defines (`docs/development/documentation-policy.md` "Architectural Decision Records").

## Related work

- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy`.
- Issues #2–#6 and their ADRs/Specifications — consumed directly (see table above).
- Issue #8 — `[Spike] Validate WinPE boot mechanism` (complete; feeds, does not block — network-delivered boot mechanism isolated for future Integration Environment validation).
- Issue #10 / ADR-0010 — `[Spike] Validate Secure Boot and hardened boot chain` (complete; feeds, does not block — source of the trusted-bootstrap fixture boundary above).
- Issue #13 / ADR-0011 — `[WP] Define trusted bootstrap and Server fingerprint delivery contract` (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`, `Approved`; complete) — the concrete Server-fingerprint delivery contract and Simulator fixture semantics (Section 8), consumed above.
- Issue #11 / ADR-0009 — `[Spike] Evaluate driver-provider integration` (complete; feeds, does not block).
- Issue #12 — `[WP] Define minimum Administrative API and Web read contract` (complete; `docs/specifications/m0-administrative-api-web-read-contract.md` resolves the Administrative API/Web contract gap identified during this Specification's drafting — see "Administrative API / Web contract").

## Open questions

None of the following are blocking for owner approval of Issue #7 — each is explicitly deferred evidence-driven or implementation-time detail, not an unresolved architectural fork. The Administrative API/Web contract is no longer tracked as an open gap here — it is resolved by the approved `docs/specifications/m0-administrative-api-web-read-contract.md` (see "Administrative API / Web contract" above); that Specification's own out-of-scope items (authentication, command/write semantics) remain future concerns owned by that document, not by this one.

1. Concrete persistence-load acceptance thresholds — established when the first post-M0 vertical slice actually runs the measurement; not decided here.
2. Simulator internal implementation architecture (process/thread model, configuration format, crate/module structure) — implementation-time, not an M0 architectural question.

Status: Approved.
