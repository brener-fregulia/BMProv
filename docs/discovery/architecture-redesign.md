# Bamep — Architecture Discovery Baseline

Status: **Discovery baseline updated with owner decisions**

## Context

Bamep is a clean implementation with no inherited Git history or source code from FORGE. The previous PoC is technical evidence: validated behavior, limitations, workarounds, and architectural mistakes. It is not a compatibility constraint.

## Product boundary

Bamep is a standalone bare-metal provisioning and recovery platform for controlled local networks. It should discover and identify endpoints, coordinate boot and maintenance environments, collect inventory, execute provisioning and recovery workflows, transfer and manage artifacts, schedule concurrent resources, and provide secure, observable, auditable operation through an API and web interface.

Bamep V1:

- provisions Windows, with Windows 11 as the primary modern target;
- supports UEFI x86-64 endpoints;
- initially operates as a single-server deployment;
- assumes a dedicated provisioning interface/VLAN/network where Bamep may control DHCP/PXE;
- does not depend on Internet access once required artifacts are available locally;
- does not require MikroTik hardware, dedicated hot cache, dedicated archive storage, RAID, PostgreSQL, or WebSocket.

Bamep is not an ERP, CRM, financial system, general-purpose RMM, NAS, general switch manager, or V1 multi-site platform. A future ERP must integrate through a public/versioned API and domain events, never through Bamep's internal database.

## Proposed component boundaries

Before final technology choices, responsibilities are separated into:

- Presentation: Web Administration and Administrative API;
- Application: Endpoint Management, Provisioning/Recovery Orchestration, Boot Orchestration, and Artifact Management;
- Domain: Endpoint, Job, JobStep, Attempt, Inventory, Artifact/Snapshot, Transfer, Storage Target, and Domain Events;
- Runtime Services: Scheduler/Resource Arbiter, Agent Control Gateway, Transfer Coordinator, and Runtime Presence Registry;
- Ports: repositories, Agent transport, boot, discovery, storage, and infrastructure metrics;
- Adapters: persistence, PXE/GRUB, switch integration, filesystem/storage, and protocol transports;
- Workers: transfer, compression, verification, and artifact movement.

The domain must not know about GRUB, MikroTik, `/dev/sda`, `snmpwalk`, WebSocket, SQLite, or zstd.

## Accepted runtime direction

The initial direction is **modular monolith first**, with explicit internal boundaries and process/worker isolation for heavy workloads when required.

Microservices, clustering, Redis, leader election, and a distributed scheduler are not V1 requirements.

Workers initially belong to the Bamep Server release and do not receive independent versioning.

## Development architecture

The physical laboratory is an Integration Environment.

Normal development must work without real PXE, MikroTik hardware, real clients, or destructive disks through:

- simulated agents;
- fake boot, discovery, and storage adapters;
- temporary local storage;
- deterministic fixtures;
- simulation of 20–24+ endpoints;
- scenarios for latency, throughput, disconnect, reconnect, retries, failures, and storage pressure.

## Frontend

Accepted direction unless a concrete blocker is discovered later:

- TypeScript;
- Svelte;
- Vite;
- Vitest;
- browser-first administration.

Bamep Web is independently deployable and updateable from Bamep Server. A Web-only bugfix should not require restarting Server jobs.

## Backend and Agent

Still undecided.

Primary candidates are Rust, Go, and Python. Before accepting a polyglot architecture, M0 should evaluate whether one language can reasonably serve both Server and Agent, considering that the project currently has one primary maintainer.

The permanent Agent must not accept arbitrary `sh -c` execution from the Server. The direction is a supervisor with typed actions, authentication, a state machine, retries, cancellation, and process supervision, while still being able to invoke fixed tools available in the Alpine maintenance environment.

## Control plane

The protocol choice remains open and requires an ADR.

Browser and Agent do not need to use the same mechanism.

Relevant candidates include:

- REST + polling;
- REST + long polling;
- WebSocket with a typed application protocol;
- SSE for browser events + HTTP commands.

Any Agent Protocol must define correlation, acknowledgement, duplicate handling, timeout, reconnect, cancellation, progress, protocol version, and idempotency semantics.

## Data plane

Large transfers remain separate from the control plane.

HTTP streaming or chunk-oriented transfer is a strong direction, but resumability must not be faked through byte offsets when the source cannot reproduce the stream from an arbitrary offset.

Production V1 should support resume/checkpoint for large transfers where technically possible. Volume/image backup and selective/chunked backup may require different strategies.

## Persistence

Durable domain state and history must be separated from runtime connection and presence state.

SQLite is a strong candidate for standalone single-node deployments. PostgreSQL remains an alternative if concrete requirements for heavier write concurrency, remote database operation, HA, or multi-site emerge. The final choice remains an M0 ADR.

## Storage

Accepted logical roles:

- `SYSTEM`;
- `CACHE`;
- `ARCHIVE`.

Dedicated CACHE and ARCHIVE storage are optional. A single SSD/NVMe may fulfill multiple roles in a Small installation. Physical hardware layout is installation configuration, not domain architecture.

Storage providers should expose capabilities rather than assumptions about RAID layouts or device names.

## Capacity and scheduling

Initial installation profiles:

- Small: approximately 3–5 active endpoints;
- Medium: approximately 8–10;
- High-density: approximately 20–24.

8 GB is the intended minimum complete-host baseline for Small, subject to measurement before 1.0.

Concurrency must not be a single fixed global number. JobSteps should compete for resource leases or tokens representing endpoint exclusivity, network capacity, storage read/write capacity, CPU/worker capacity, and other relevant constrained resources.

## Security invariants

The provisioning LAN is controlled but not inherently trustworthy.

- MAC addresses are not authentication or permanent identity;
- Server and Agent must authenticate each other appropriately;
- destructive operations must validate Endpoint identity, inventory revision, and disk identity/fingerprint;
- reconnect must not blindly replay destructive commands;
- Agent actions must be typed;
- critical backups must pass integrity verification before destructive provisioning proceeds;
- remote PTY/shell access, if retained, is break-glass functionality and disabled by default;
- boot-chain integrity is a requirement even though Secure Boot itself is not an M0 implementation requirement.

## Endpoint identity

Endpoint identity must survive NIC or MAC replacement.

Direction to evaluate through ADR:

1. Boot Orchestrator creates a short-lived enrollment context or credential;
2. Agent authenticates the Server;
3. Agent redeems the short-lived credential;
4. a runtime Agent identity/session credential is established;
5. MAC addresses and hardware fingerprints remain inventory signals rather than trust anchors.

## Backup model

There is no generic `backup=true` semantic.

Minimum strategies to specify independently:

- Volume/Image backup;
- Selective backup.

Every completed artifact requires metadata, expected size when applicable, a cryptographic digest, explicit incomplete state, atomic completion/commit semantics, and an explicit verification state.

## Durable workflow

Each relevant provisioning stage is a JobStep with preconditions, execution state, result, postconditions, retry semantics, and cancellation semantics.

After power loss or reconnect, the Server must reconcile actual endpoint state with durable workflow state. Destructive operations must never be automatically retried merely because a generic retry policy exists.

## Observability

Correlation must make it possible to relate endpoint, job, step, attempt, action, and transfer.

Durable domain events should exist when they are useful to the product itself and future integrations, for example provisioning completed/failed, artifact created/verified, and inventory updated.

High-frequency telemetry does not need to be persisted indefinitely.

## Open-source and commercial boundary

Standalone Bamep remains genuinely useful as open-source software, including Server, Agent, Web, orchestration, scheduler, backup/recovery, artifact handling, Simulator, API, basic adapters, and essential observability.

Future commercial differentiation may exist above or around the engine, including ERP integration, multi-site management, centralized management, advanced reporting, hosted services, support, and specialized integrations.

Do not create customer-conditional forks or code paths such as `if customer == X`.

## Packaging and versioning

Accepted direction:

- Linux Server, with Debian as the initial production target;
- native `.deb` packages and a signed APT repository as the eventual distribution model;
- no silent application-level self-updater for the Server;
- independent SemVer for Server, Web, and Agent;
- contracts versioned separately, for example Administrative API v1 and Agent Protocol v1;
- no lockstep releases between independently deployable components.

## Explicitly isolated technical spikes

The following questions must not be silently decided during implementation:

- definitive WinPE mechanism;
- transfer/snapshot resumability when the producer cannot support arbitrary restart;
- Secure Boot or a hardened boot-chain strategy;
- driver-provider integration.

## Future: pre/post provisioning diagnostics (not M0 scope)

Recorded during Issue #6 (data-plane and storage contracts) owner review as a future product use case, not as part of the M0 data-plane contract or any current Work Package.

Bamep should eventually support an automated diagnostic/benchmark workflow running in the client's installed OS:

- run diagnostics/performance measurements on the original Windows installation;
- persist a pre-service baseline;
- reboot into the maintenance/provisioning workflow;
- perform backup/provisioning as required;
- boot the newly installed/configured Windows environment;
- run equivalent post-service diagnostics;
- compare pre/post results;
- produce an operator/customer-facing report.

This should also allow aggregate historical comparison across Windows versions/builds, driver versions, Bamep provisioning-process changes, and hardware migrations such as HDD → SATA SSD/NVMe.

This is a future workflow/use case only. It is not designed here: the Windows-side execution component, benchmark suite, reporting schema, and telemetry architecture are all undecided and out of scope for any current Work Package.

The existing linear Job/JobStep model (`docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md`) is expected to remain compatible with a future workflow conceptually resembling:

```text
PreflightDiagnostics → Backup → Provision → Configure → PostflightDiagnostics → Report
```

This is a forward-compatibility expectation, not a change to the accepted Job/JobStep model — ADR-0006 is not modified to add these future JobStep names, and no Work Package currently implements them.
