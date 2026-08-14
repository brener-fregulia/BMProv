# M0 — Product, Component, and Packaging Baseline

Status: **Proposed - awaiting owner approval**

## Context

This Specification persists the M0 scope items "product boundary and domain vocabulary," "component responsibilities and boundaries," "boot-orchestration architectural boundary," and "packaging and versioning baseline" (`docs/specifications/m0-architecture-baseline.md`) as durable Specification content, executing Issue #1 (`[WP] Define product, runtime, and stack architecture baseline`).

Runtime topology and language-strategy decisions are recorded separately as ADR-0001, ADR-0002, and ADR-0003 rather than duplicated here.

Most content below restates already Discovery-accepted facts (`docs/discovery/architecture-redesign.md`) in their durable Specification location. The component-boundary section is the one part of this document that goes beyond restating an already-accepted fact — Discovery itself labeled these boundaries "Proposed component boundaries," so their elevation to the M0 baseline here is a proposal requiring explicit owner approval, not a restatement of a prior decision.

## Product boundary and domain vocabulary

Bamep is a standalone bare-metal provisioning and recovery platform for controlled local networks. It discovers and identifies endpoints, coordinates boot and maintenance environments, collects inventory, executes provisioning and recovery workflows, transfers and manages artifacts, schedules concurrent resources, and provides secure, observable, auditable operation through an API and web interface.

Bamep V1:

- provisions Windows, with Windows 11 as the primary modern target;
- supports UEFI x86-64 endpoints;
- initially operates as a single-server deployment;
- assumes a dedicated provisioning interface/VLAN/network where Bamep may control DHCP/PXE;
- does not depend on Internet access once required artifacts are available locally;
- does not require MikroTik hardware, a dedicated hot cache, dedicated archive storage, RAID, PostgreSQL, or WebSocket.

Bamep is not an ERP, CRM, financial system, general-purpose RMM, NAS, general switch manager, or V1 multi-site platform. A future ERP must integrate through a public/versioned API and domain events, never through Bamep's internal database.

(Source: `docs/discovery/architecture-redesign.md`, "Product boundary" — already accepted.)

## Component responsibilities and boundaries

**Proposed for owner approval as the M0 baseline** (elevating Discovery's "Proposed component boundaries"):

- **Presentation**: Web Administration and Administrative API.
- **Application**: Endpoint Management, Provisioning/Recovery Orchestration, Boot Orchestration, and Artifact Management.
- **Domain**: Endpoint, Job, JobStep, Attempt, Inventory, Artifact/Snapshot, Transfer, Storage Target, and Domain Events.
- **Runtime Services**: Scheduler/Resource Arbiter, Agent Control Gateway, Transfer Coordinator, and Runtime Presence Registry.
- **Ports**: repositories, Agent transport, boot, discovery, storage, and infrastructure metrics.
- **Adapters**: persistence, PXE/GRUB, switch integration, filesystem/storage, and protocol transports.
- **Workers**: transfer, compression, verification, and artifact movement (isolation boundary accepted in ADR-0001; language open in ADR-0003).

The Domain must not depend on GRUB, MikroTik, `/dev/sda`, `snmpwalk`, WebSocket, SQLite, or zstd — those are Adapter responsibilities.

These boundaries apply within the modular-monolith runtime topology accepted in ADR-0001: one deployable Server artifact with the internal boundaries above, plus a separate Worker process/isolation boundary for heavy workloads.

## Boot-orchestration architectural boundary

**Principle (already accepted, persisted here)**: the Domain must not know about GRUB, MikroTik, or device paths such as `/dev/sda`; boot mechanics belong to Adapters, coordinated through the Application-level Boot Orchestration responsibility.

**Explicitly open, not resolved by this Specification**: the concrete Boot Orchestrator mechanism and its exact contract with the underlying boot chain depend on the WinPE boot mechanism Technical Spike (Issue #8), which has not yet produced evidence. This Specification records only the boundary principle above; the mechanism-level design must wait for that Spike, consistent with Issue #1's stated dependency ("may reference preliminary findings... before finalizing, but is not blocked from starting").

## Packaging and versioning baseline

Already-accepted direction, persisted here:

- Linux Server, with Debian as the initial production target;
- native `.deb` packages and a signed APT repository as the eventual distribution model;
- no silent application-level self-updater for the Server;
- independent SemVer for Server, Web, and Agent;
- Workers ship with the Server release and do not receive independent versioning (ADR-0001);
- contracts versioned separately, for example Administrative API v1 and Agent Protocol v1;
- no lockstep releases between independently deployable components.

(Source: `docs/discovery/architecture-redesign.md`, "Packaging and versioning" — already accepted.)

## Out of scope

- WinPE implementation, a real production packaging pipeline, or a signed APT repository build (implementation, not M0);
- final production backup/snapshot format;
- Endpoint identity, control-plane/Agent-action contracts, Job/JobStep lifecycle and scheduling, persistence/observability, data-plane/storage, and Simulator decisions — see the sibling M0 Work Packages (Issues #2–#7);
- ERP, licensing enforcement, multi-site management, HA, Tauri (per `docs/specifications/m0-architecture-baseline.md` "Out of scope").

## Acceptance criteria

- Product boundary, vocabulary, and non-goals are persisted (M0 acceptance criterion 1) — satisfied by this document.
- Component responsibilities and boundaries are documented (M0 acceptance criterion 5) — drafted here; becomes settled only once the owner approves the elevation from Discovery's "Proposed" status.
- Packaging and versioning baseline is persisted — satisfied by this document.
- The boot-orchestration boundary principle is persisted, and the boundary's concrete mechanism is explicitly isolated pending Issue #8 rather than hidden inside a future implementation Work Package (M0 acceptance criterion 7).

## Related ADRs

- ADR-0001 — Runtime topology: modular monolith with worker/process isolation (`Accepted`).
- ADR-0002 — Backend/Server implementation language: Rust (`Accepted`).
- ADR-0003 — Worker and Agent implementation language strategy (`Proposed`, open — owner decision required).

## Related work

- Issue #1 — `[WP] Define product, runtime, and stack architecture baseline` (this Specification and the three related ADRs are its output).
- Issue #8 — `[Spike] Validate WinPE boot mechanism` (feeds the Boot Orchestrator mechanism, not yet complete).

## Open questions

1. Does the owner approve elevating Discovery's "Proposed component boundaries" to the accepted M0 baseline as described above, or is a different layering intended?
2. Does the owner accept ADR-0003's recommendation (Rust for Worker and Agent), or an alternative?

Status: Proposed - awaiting owner approval.
