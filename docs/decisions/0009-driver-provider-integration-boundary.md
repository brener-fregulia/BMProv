# ADR-0009: Driver-provider integration boundary

Status: Accepted

## Context

`docs/specifications/m0-architecture-baseline.md` names "driver-provider integrations" as a required M0 Technical Spike, and `docs/discovery/architecture-redesign.md` lists it among the explicitly isolated questions that "must not be silently decided during implementation." Neither document defines what a "driver provider" is or how Bamep should integrate with one. Issue #11 (`[Spike] Evaluate driver-provider integration`) investigated this, producing the empirical findings in `docs/reference/driver-provisioning.md`.

Bamep needs to apply Windows/WinPE drivers in two situations:

1. **Boot-critical drivers for WinPE itself** — a NIC or storage/RAID controller not covered by WinPE's inbox driver set can block PXE boot, network reachability to the Bamep Server, or visibility of the target disk, independent of what the target Windows installation will eventually need (`docs/reference/driver-provisioning.md` Finding 2).
2. **Target-OS drivers for the deployed Windows installation** — drivers the installed Windows 11 environment needs for its own hardware.

Bamep must not bundle proprietary driver packs directly (Issue #11's stated constraint, confirmed by `docs/reference/driver-provisioning.md` Finding 4: OEM and Microsoft driver-licensing terms generally restrict third-party redistribution, though exact terms vary by vendor and were not exhaustively reviewed).

## Decision

Bamep defines a **driver-provider boundary** as a Port, in the sense already accepted by `docs/specifications/m0-stack-and-boundaries-baseline.md` (Ports: repositories, Agent transport, boot, discovery, storage, infrastructure metrics — Domain and Application code depend on the Port, never on a concrete mechanism):

- **Driver source (operator-managed, local, out of Bamep's own licensing scope)**: an operator stages driver packages (standard Windows `.inf`-based driver package layout) into a locally accessible driver repository, sourced by the operator from wherever they have rights to obtain them (OEM download sites, the Microsoft Update Catalog, a vendor's own driver-automation tool, or a general offline-capable driver-matching tool such as Snappy Driver Installer Origin used in its fully offline mode — `docs/reference/driver-provisioning.md` Finding 6 — Bamep does not care which) — this directly follows the established industry pattern found in `docs/reference/driver-provisioning.md` Finding 5 (MDT's "Out-of-Box Drivers" model: the deployment tool never fetches, bundles, or redistributes the driver packages itself; it only consumes an already-staged local repository).
- **Driver injection (Bamep-owned mechanism)**: Bamep applies drivers from that repository using the standard DISM offline-servicing mechanism (`docs/reference/driver-provisioning.md` Finding 1) for target-OS image injection and for WinPE build-time injection, and `drvload`-equivalent runtime loading for WinPE boot-critical drivers where build-time injection is insufficient (Finding 2).
- **Bamep does not bundle, embed, fetch on the operator's behalf, or redistribute proprietary OEM/Microsoft driver packages in V1.** This is the direct consequence of Finding 4 and keeps Bamep out of the driver-licensing relationship entirely — the operator remains the party with rights to whatever drivers they stage.
- **Bamep does not depend on Windows Setup's online "Dynamic Update" driver mechanism** (Finding 3) for its own provisioning phase, because it requires Internet access during provisioning, which contradicts the already-accepted product boundary that Bamep V1 "does not depend on Internet access once required artifacts are available locally" (`docs/specifications/m0-stack-and-boundaries-baseline.md`). Any online driver activity Windows itself performs after Bamep's provisioning workflow has completed (if that machine later has Internet access) is outside Bamep's control and outside this decision's scope.

This decision establishes the **boundary and responsibility split**, not the concrete repository directory layout, an in-product driver-management UI, or a specific vendor-catalog-fetching adapter — those remain open (see "Open questions").

## Alternatives considered

- **Bamep bundles/embeds driver packages in its own release artifacts.** Rejected: directly creates the licensing exposure Finding 4 evidences (OEM/Microsoft terms generally restrict third-party redistribution), and was already excluded by the Issue's own stated constraint.
- **Bamep fetches driver packages from vendor catalogs or the Microsoft Update Catalog on the operator's behalf at runtime.** Rejected for V1: still places Bamep inside the driver-licensing relationship (fetching/caching third-party copyrighted content under terms Bamep itself would need to satisfy), requires per-vendor integration not evidenced as required by any current M0 Work Package, and was not the pattern found in established practice (Finding 5) — vendor-specific driver-automation tools exist precisely so the *operator* performs this step under the vendor's own terms. The one documented real-world example of this exact pattern — an MDT task sequence invoking Snappy Driver Installer Origin live, with Internet access, to fetch fresh driver indexes and packs during deployment (`docs/reference/driver-provisioning.md` Finding 6) — is a concrete instance of what this alternative rejects, not a counterexample to it. Not designed here; recorded as an open question in case a concrete future requirement justifies it.
- **Bamep relies on Windows Setup's Dynamic Update / post-deployment Windows Update for driver acquisition instead of local injection.** Rejected: requires Internet access during the provisioning phase, contradicting the already-accepted offline-capable product boundary (Finding 3).

## Consequences

- The operator is responsible for sourcing and staging drivers they have rights to use; Bamep never becomes a redistribution party for OEM/Microsoft driver content.
- Bamep's Boot Orchestration and Provisioning/Recovery Orchestration responsibilities (`docs/specifications/m0-stack-and-boundaries-baseline.md` "Component responsibilities and boundaries") gain a driver-provider Port dependency, consumed through an Adapter that reads the operator-staged local repository — Domain code must not assume any specific repository layout, vendor tool, or DISM/`drvload` invocation detail, consistent with the Domain/Adapter boundary already accepted for storage.
- Hardware unsupported by WinPE's inbox drivers and with no operator-supplied driver in the repository will fail to PXE-boot, network, or see its target disk through Bamep — this is a hard dependency the operator must resolve by staging the needed driver, not a gap Bamep works around automatically.
- No dependency on Internet access is introduced during Bamep's own provisioning phase for driver acquisition.

## Related architecture

- `docs/specifications/m0-stack-and-boundaries-baseline.md` — Ports/Adapters boundary model this decision extends; **recommended future amendment** (not made by this ADR): add a "driver source/repository" Port and Adapter to that already-Approved document's Ports/Adapters lists. That document belongs to Issue #1, already `Done` — amending it requires separate, explicit owner authorization; this ADR only records the recommendation.
- `docs/discovery/architecture-redesign.md` — "Explicitly isolated technical spikes" (this Spike) and "Backend and Agent" (Boot Orchestration responsibility).

## Related work

- Issue #11 — `[Spike] Evaluate driver-provider integration` (this ADR's origin).
- `docs/reference/driver-provisioning.md` — empirical findings this decision applies, including the Snappy Driver Installer Origin (SDIO) case study (Finding 6) evaluated during owner review.
- Issue #1 / `docs/specifications/m0-stack-and-boundaries-baseline.md` — Ports/Adapters model; product boundary's no-Internet-dependency requirement.
- Issue #8 — `[Spike] Validate WinPE boot mechanism` — WinPE build/boot mechanism this decision's build-time/runtime driver injection applies to; not resolved by this ADR.

## Open questions

1. Exact local driver-repository directory layout/organization (by OS/platform/model, or otherwise) — implementation-time, not decided here.
2. Whether a future first-party adapter integrating with a specific vendor's driver-catalog automation tool — including a general tool such as Snappy Driver Installer Origin, evaluated as a named case study in `docs/reference/driver-provisioning.md` Finding 6 — is ever justified — no current requirement evidences it; not designed here.
3. Whether/how an operator-facing UI or workflow for staging drivers is needed — not designed here, a future Work Package concern if required.
4. The recommended amendment to `m0-stack-and-boundaries-baseline.md`'s Ports/Adapters lists (see "Related architecture") — requires separate owner authorization to actually apply, not executed by this ADR.

Status: Accepted.
