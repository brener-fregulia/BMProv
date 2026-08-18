# M1 — Simulated Vertical Slice & Baseline Validation

Status: **Approved**

## Classification

Type: Feature/Epic-level Specification (post-M0 first implementation milestone).

Execution grouping: native GitHub Milestone #2
("M1 — Simulated Vertical Slice & Baseline Validation").

## Context

M0 (`docs/specifications/m0-architecture-baseline.md`) closed the architecture and
contract phase only: all 11 ADRs are `Accepted`, all 11 M0 Specifications are
`Approved`, and the owner has explicitly approved that baseline
(`m0-architecture-baseline.md`, Status line). No product implementation exists yet —
the repository contains no Cargo workspace and no source code
(`docs/architecture/README.md` — "no implemented application architecture yet").
M0's own "First implementation slice after M0" section
(`m0-architecture-baseline.md` "First implementation slice after M0") already
defines the required scope of the milestone that follows it; this Specification
turns that already-approved scope into an executable Feature/Epic-level
Specification.

## Goal

Implement, and empirically validate, the first integrated, executable Bamep system —
a Simulated Endpoint connects, authenticates/enrolls, reports inventory, has a Job
created, is scheduled, receives a dispatched typed action, executes a simulated
transfer, has progress/events persisted, survives disconnect/reconnect, reaches a
terminal Job state, and has that result observable through Bamep Web — entirely
without physical endpoint hardware, and empirically validate the ADR-0007
persistence-load expectation and the 20–24 concurrent Simulated Endpoint scenario
required by M0.

## Scope

- minimum repository and development-tooling bootstrap required by the functional
  slice, including Rust components and Bamep Web tooling as needed, introduced only
  to the extent each functional Work Package actually requires it. This
  Specification does not mandate a particular crate, package, or binary structure.
  The Simulator's Agent-side participant must use the real Agent Protocol v1
  transport path (`m0-simulator-contract-and-validation-strategy.md`), but that
  requirement alone does not determine physical crate/binary boundaries
  (`m0-stack-and-boundaries-baseline.md`);
- Endpoint identity/credential/hardware-confidence lifecycle, including
  operator-approval-gated first enrollment as the M0 default path
  (`m0-endpoint-identity-lifecycle.md`);
- Agent Protocol v1 real transport, handshake, `BootstrapEvidence`, action
  envelope (`m0-agent-protocol-contract.md`);
- the trusted-bootstrap invariant (destructive-operation precondition 7)
  represented through the Simulator fixture semantics owned by
  `m0-trusted-bootstrap-and-server-fingerprint-contract.md` Section 8, consistent
  with ADR-0010's explicit allowance for a deterministic non-production fixture;
- Job/JobStep/Attempt persistence, scheduling, and resource leases, including the
  full destructive-operation precondition gate
  (`m0-job-lifecycle-and-scheduling.md`, `m0-endpoint-identity-lifecycle.md`);
- durable/transient persistence split, domain events, and audit records
  (`m0-persistence-observability-and-domain-events.md`), on SQLite as the accepted
  entering baseline (ADR-0007);
- data-plane chunked transfer, transfer-session authentication, and Artifact
  lifecycle (`m0-data-plane-and-storage-contracts.md`);
- Administrative API v1 read surface and a minimal Bamep Web read view
  (`m0-administrative-api-web-read-contract.md`);
- the Bamep Simulator, at real-Agent-Protocol-transport fidelity
  (`m0-simulator-contract-and-validation-strategy.md`);
- execution of the full required Simulator scenario table, at deterministic small
  scale or at the 20–24 concurrent Simulated Endpoint target as specifically
  required per scenario (see "Non-functional requirements" and the Work Package
  decomposition owned by the M1 Milestone).

## Out of scope

Physical PXE/DHCP/UEFI/GRUB/WinPE/Alpine boot delivery; real firmware Secure Boot
execution; the real operator-verified site-key pairing ceremony of ADR-0011 (its
concrete UX/transport is explicitly deferred to the Integration Environment by
`m0-trusted-bootstrap-and-server-fingerprint-contract.md`); production Secure Boot
deployment; real Windows deployment or destructive real-disk operations;
MikroTik-specific production integration; production administrative
authentication/RBAC; a generic Web command/write API; live-Windows backup; final
production backup/snapshot format; HA or multi-site; ERP integration — all already
excluded by M0 (`m0-architecture-baseline.md` "Out of scope";
`m0-administrative-api-web-read-contract.md` "Unresolved findings surfaced for
owner review").

M1 does not introduce any Web write capability — no Job creation, no
enrollment-approval action, no cancellation — through Bamep Web merely to
demonstrate the vertical slice. Where an approved contract requires an action that
Web cannot originate (Job creation, operator enrollment approval), that action is
originated by the Simulator/test harness or another internal implementation
mechanism, distinct from Bamep Web.

## Functional requirements

- RF-001: A Simulated Endpoint's trusted-bootstrap stage first establishes the
  Simulator fixture equivalent of `trusted bootstrap established`
  (`m0-trusted-bootstrap-and-server-fingerprint-contract.md` Section 6, "Agent
  bootstrap sequence"): a nonce-bound signed bootstrap assertion is verified
  locally, which makes an authenticated expected Server TLS certificate fingerprint
  available. Only after that authenticated fingerprint is available does the Agent
  open a real WSS connection to the Server; the Agent verifies the Server's
  presented TLS certificate fingerprint against the already-authenticated expected
  fingerprint (pinning), and only on match does Agent Protocol v1 authentication
  (credential redemption/validation via `AuthRequest`) proceed. Trusted-bootstrap
  verification strictly precedes the WSS connection and fingerprint pinning — it is
  never performed after, or as part of, Agent Protocol v1 authentication. On
  successful credential validation, the Server responds `SessionEstablished` and,
  per `m0-endpoint-identity-lifecycle.md` ("first successful credential exchange"),
  the Endpoint identity record is created in `PendingEnrollment` at this point —
  independent of, and strictly before, any `BootstrapEvidence` exchange. Only after
  `SessionEstablished` does the Agent report authenticated `BootstrapEvidence`
  (`boot_nonce`, the assertion, `local_boot_trust: Established`), which the Server
  independently verifies before recording the trusted-bootstrap fact for that boot
  context. A `PendingEnrollment` session resulting from a valid credential exchange
  remains intact and unaffected even when `BootstrapEvidence` is absent, malformed,
  or rejected — only the trusted-bootstrap fact for that boot context becomes/
  remains `NotEstablished` (`m0-agent-protocol-contract.md` "Trusted bootstrap
  evidence"; `m0-trusted-bootstrap-and-server-fingerprint-contract.md` "Failure
  semantics").
- RF-002: An explicit, distinct operator-approval action transitions the Endpoint
  from `PendingEnrollment` to `Enrolled` (`m0-endpoint-identity-lifecycle.md`,
  ADR-0004 default path). This action is originated by a control path separate
  from the Simulated Agent participant — a harness command, CLI, or development
  fixture standing in for the operator decision point. The Simulated
  Endpoint/Agent participant must not, and semantically cannot, approve its own
  enrollment: the mechanism that records this decision is invoked independently
  of the Agent's own protocol messages, even though its concrete technique
  remains implementation-time. The underlying state machine and the requirement
  that approval be a real, explicit, durable, auditable step must not be
  simplified into automatic enrollment merely because the Endpoint is simulated.
- RF-003: Inventory is durably recorded on change only.
- RF-004: A Job/JobStep/Attempt is created (via Simulator/harness, not a Web write
  API), scheduled, dispatched, and reaches a terminal state, including
  reconciliation after disconnect or Server restart. The full 7-precondition
  destructive-operation gate (`m0-endpoint-identity-lifecycle.md`
  "Destructive-operation authorization preconditions") is implemented and tested
  at this small deterministic scale, including the negative case where
  preconditions 1–6 hold and precondition 7 (trusted bootstrap) alone fails. All
  destructive-labeled operations exercised in M1 remain simulated and do not touch
  physical hardware or disks — their effect is represented within the
  Simulator/fake storage adapter boundary.
- RF-005: A simulated data-plane transfer completes end-to-end with
  transfer-session authentication, chunk resume, and Artifact verification,
  without touching physical storage hardware.
- RF-006: Bamep Web observes Endpoint/Job/JobStep/Attempt/Transfer state
  exclusively through Administrative API v1 reads.
- RF-007: The Simulator orchestrates 20–24 concurrent Simulated Endpoints
  specifically to exercise the three scenario categories M0 explicitly ties to
  that concurrency target: scheduler contention, the ADR-0007 persistence-load
  measurement, and data-plane chunked transfer at scale
  (`m0-simulator-contract-and-validation-strategy.md` "Concurrency target"). The
  remaining required Simulator scenarios (duplicate/delayed messages, stale
  inventory, endpoint disappearance, Agent restart, Server restart, resource
  exhaustion, partial failure, cancellation, recovery after interruption,
  trusted-bootstrap independence) are proven correct at deterministic smaller
  scale and are not required individually or simultaneously at the 20–24 target,
  unless combining a given scenario with concurrency is necessary to exercise the
  specific behavior under test.

## Non-functional requirements

- NF-001 (persistence-load empirical validation): SQLite is the accepted baseline
  entering M1 (ADR-0007). M1 must execute the empirical measurement ADR-0007
  already requires — durable write volume, contention, latency, and backpressure
  under sustained concurrent Job/JobStep/Attempt activity at the 20–24 endpoint
  target — and record the actual observed result. No numeric acceptance threshold
  is invented before that measurement runs
  (`m0-simulator-contract-and-validation-strategy.md` "Persistence-load
  validation"). If the representative 20–24-endpoint measurement shows
  unacceptable contention, latency, write pressure, or backpressure, ADR-0007 must
  be explicitly revisited — a pre-declared contingency, not something to be
  silently worked around in implementation.
- NF-002 (reference environment): Linux is Bamep's development and production
  reference environment (`AGENTS.md`). Automated validation targeting
  Linux-specific responsibilities must be executed in a genuinely Linux
  environment — either native Linux development, or, when validating from Windows
  11, WSL2 when it faithfully represents the responsibility under test. WSL2 or
  containers are a means of reaching the Linux reference environment from
  Windows, not the reference environment itself; native Linux execution is
  equally valid reference evidence. Native-Windows-only execution is not
  sufficient evidence for Linux-specific responsibilities.

## Safety invariants

The full 7 destructive-operation preconditions
(`m0-endpoint-identity-lifecycle.md` "Destructive-operation authorization
preconditions") are implemented and independently tested at small deterministic
scale in the Work Package that introduces destructive-action dispatch — not first
demonstrated at Simulator/concurrency scale. This includes precondition 7's
independence from precondition 2: a valid Agent credential must never be treated
as proof the current boot path was itself trusted
(`m0-simulator-contract-and-validation-strategy.md`, the required
trusted-bootstrap-independence scenario). No blind redispatch of destructive
actions on reconnect, timeout, or `Unknown`/`Indeterminate` outcomes.
Transfer-session authentication fails closed with a single non-enumerable denial
reason. All destructive-labeled operations exercised anywhere in M1 remain
simulated — none touch physical hardware or physical disks.

## Architecture constraints

Rust modular monolith with Worker process isolation (ADR-0001); Server in Rust
(ADR-0002); Agent/Worker in Rust with contract-independence from wire protocols
(ADR-0003); SQLite persistence behind the `repositories` Port as the accepted
entering baseline, subject to NF-001 (ADR-0007); Presentation / Application /
Domain / Runtime Services / Ports / Adapters / Workers as dependency boundaries,
not a mandated crate/package/module layout
(`m0-stack-and-boundaries-baseline.md`). M1 is not responsible for empirically
revalidating every M0 decision — see "Traceability" below.

## Traceability

Directly exercised by M1: `m0-architecture-baseline.md`,
`m0-stack-and-boundaries-baseline.md`, `m0-endpoint-identity-lifecycle.md`,
`m0-agent-protocol-contract.md`, `m0-job-lifecycle-and-scheduling.md`,
`m0-persistence-observability-and-domain-events.md`,
`m0-data-plane-and-storage-contracts.md`,
`m0-simulator-contract-and-validation-strategy.md`,
`m0-administrative-api-web-read-contract.md`,
`m0-trusted-bootstrap-and-server-fingerprint-contract.md`; ADR-0004 through
ADR-0008; ADR-0010 (fixture-level trusted-bootstrap substitution only, per its own
allowance for a deterministic non-production fixture — not real Secure
Boot/firmware mechanics).

Architectural constraints preserved (followed, not empirically retested by M1):
ADR-0001 (modular monolith, Worker process isolation), ADR-0002 (Server: Rust),
ADR-0003 (Agent/Worker: Rust, contract independence from wire protocols).

Relevant M0 decisions outside M1 execution: ADR-0009 (driver-provider integration
boundary — out of scope for the vertical slice); ADR-0011 (operator-verified
site-key pairing — its real ceremony is explicitly deferred to the Integration
Environment by `m0-trusted-bootstrap-and-server-fingerprint-contract.md`; M1
satisfies destructive-operation precondition 7 only through the ADR-0010 fixture
substitution, not through ADR-0011's real ceremony).

## Validation expectations

M1 uses the following layers of `docs/development/testing.md`'s general test-layer
model: Unit/Domain, Contract, Component/Integration, Simulator, and Owner manual
validation — per the per-concept itemization already present in each M0
Specification's own "Validation expectations" section, not restated here.
Integration Environment remains a layer of the overall Bamep testing strategy but
is not a completion requirement of M1 for behaviors explicitly deferred to
physical hardware (see "Integration Environment boundary" below).

## Integration Environment boundary

None of M1's required scenarios need physical hardware. PXE, DHCP, UEFI, GRUB,
Alpine boot, physical NIC behavior, MikroTik integration, real disk tooling,
Windows deployment, WinPE, the real ADR-0011 operator-verified site-key pairing
ceremony, and hardware-specific compatibility remain deferred to the Integration
Environment (`m0-simulator-contract-and-validation-strategy.md` "What the
Simulator cannot represent"; `m0-trusted-bootstrap-and-server-fingerprint-contract.md`
"Validation expectations").

## Open questions

1. Concrete technique for the operator-approval control path separate from the
   Simulated Agent participant (harness command / CLI / development fixture /
   other) — implementation-time; the semantic separation from the Agent
   participant is already decided (see RF-002).
2. Concrete technique for Job-creation origination (harness / CLI / other) —
   implementation-time, same category as above.
3. Numeric persistence-load acceptance thresholds — established only once
   NF-001's measurement runs, not decided here.

Status: **Approved**
