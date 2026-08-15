# Driver Provisioning — Windows/WinPE Driver Injection and Licensing Posture

## Context

This reference document records the empirical findings of the M0 Technical Spike "Evaluate driver-provider integration" (Issue #11), investigating viable boundaries and integration approaches for obtaining and applying driver-provider artifacts without Bamep bundling proprietary driver packs.

Neither `docs/specifications/m0-architecture-baseline.md` nor `docs/discovery/architecture-redesign.md` defined what a "driver provider" is or how it should integrate — this was genuinely open before this Spike, and no prior reference material in `docs/reference/` addressed driver provisioning.

## Method

Research/desk investigation (per the Issue's own expected method — "primarily research/desk-based initially"), using current Microsoft Learn documentation and OEM/Microsoft licensing material as of August 2026. No physical hardware or Integration Environment work was required to answer this question; the mechanisms below are well-documented, stable Windows deployment technology, not something requiring novel experimentation to validate.

## Finding 1: DISM offline driver injection is the standard mechanism

`DISM /Add-Driver` adds a driver package to an offline-mounted Windows image (or a WinPE image at build time). A single `.inf` or a whole folder (with `/Recurse`) can be added; the driver package is placed in the image's driver store, and Plug and Play associates it to the matching device the next time the image boots. This requires a technician computer or WinPE running Windows 10/Server 2016 or later. Unsigned drivers require the explicit `/ForceUnsigned` override — signature enforcement is the default.

Source: [Add and Remove Driver packages to an Offline Windows Image](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/add-and-remove-drivers-to-an-offline-windows-image?view=windows-11), [Add-WindowsDriver (Dism)](https://learn.microsoft.com/en-us/powershell/module/dism/add-windowsdriver?view=windowsserver2025-ps).

## Finding 2: WinPE ships only a limited inbox driver set; boot-critical drivers may need separate handling

The default WinPE image includes only the network/storage drivers Microsoft bundles with the Windows ADK's WinPE add-on. Two distinct injection points exist:

- **Build-time**: drivers can be added into the WinPE boot image itself via DISM before the image is deployed to boot media, exactly as for an offline target-OS image (Finding 1).
- **Runtime**: while WinPE is already running, `drvload` can load an additional driver on demand (documented via "Drvload Command-Line Options").

This matters specifically for Bamep because a NIC or storage/RAID controller not covered by WinPE's inbox set can block PXE boot, network reachability to the Bamep Server, or visibility of the target disk — i.e., it can prevent Bamep from operating on that hardware at all, independent of whatever drivers the target Windows installation itself will eventually need.

Source: [WinPE Network Drivers: Initializing and adding drivers](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/winpe-network-drivers-initializing-and-adding-drivers?view=windows-11), [Windows PE (WinPE)](https://learn.microsoft.com/en-us/windows-hardware/manufacture/desktop/winpe-intro?view=windows-11).

## Finding 3: Windows Setup's own online driver mechanism ("Dynamic Update") requires Internet access and is not a fit for Bamep's offline provisioning phase

Windows Setup can contact a Microsoft endpoint during installation ("Dynamic Update") to fetch drivers targeted for Dynamic Update release, among other content, and apply them to the installation. This is enabled by default in Windows feature-update workflows and can be explicitly suppressed for drivers (`/dynamicupdate NoDrivers`).

This mechanism depends on Internet access during the provisioning phase itself. Bamep's already-accepted product boundary states V1 "does not depend on Internet access once required artifacts are available locally" (`docs/specifications/m0-stack-and-boundaries-baseline.md` "Product boundary and domain vocabulary"). Dynamic Update is therefore not a mechanism Bamep can rely on for its own offline provisioning phase; it is not designed against here, and any post-deployment Windows Update driver activity on the resulting machine (after Bamep's own provisioning workflow has completed, if that machine later has Internet access) is outside Bamep's control and outside this Spike's scope.

Source: [Update Windows installation media with Dynamic Update](https://learn.microsoft.com/en-us/windows/deployment/update/media-dynamic-update), [What are Dynamic Updates in Windows](https://www.thewindowsclub.com/what-are-dynamic-updates-in-windows-10).

## Finding 4: OEM and Microsoft driver licensing terms generally restrict third-party redistribution

The Issue asked to confirm, rather than assume, the licensing constraint. Evidence gathered supports the general constraint the Issue anticipated, though exact terms vary by vendor and were not exhaustively reviewed for every OEM:

- Dell's End User License Agreement governs software delivered on Dell systems (including firmware/BIOS) and includes compliance/audit provisions for over-deployment beyond licensed use; "Free Software" (e.g. install-enabling scripts) is licensed only for the equipment/environments Dell designed it for. Source: [Dell EULA](https://i.dell.com/sites/csdocuments/Legal_Docs/en/DellEULA_English.pdf), [Dell License Agreements](https://www.dell.com/en-us/lp/legal/terms-of-sale-commercial-and-public-sector-license-agreements).
- Lenovo's License Agreement states preinstalled software is licensed only for use on the Lenovo hardware it shipped with and "may not be transferred independent of that hardware product," and that third-party components are governed by their own separate terms, which "solely govern" their use. Source: [Lenovo License Agreement](https://support.lenovo.com/us/en/solutions/ht100141-lenovo-license-agreement-l505-0009-06).
- Microsoft's own driver-distribution guidance distinguishes "normal business uses" from redistribution of driver software to third parties — the latter is explicitly called out as outside normal business use. Source found via search of Microsoft driver clarification guidance (see "Understanding Windows Update rules for driver distribution", [Microsoft Learn](https://learn.microsoft.com/en-us/windows-hardware/drivers/dashboard/understanding-windows-update-automatic-and-optional-rules-for-driver-distribution)).

**This is a general posture, not a specific legal determination for every vendor/license combination.** A Technical Spike is not a substitute for legal review; before any Bamep feature that would have Bamep itself download, cache, or redistribute vendor driver packages on an operator's behalf, explicit legal/procurement review of the specific vendor terms in force at that time is required.

## Finding 5: established industry practice separates driver *sourcing* from driver *injection*

Microsoft Deployment Toolkit's "Out-of-Box Drivers" model is the established pattern this Spike found for exactly this problem: the administrator/operator sources driver packages themselves (from OEM download sites, the Microsoft Update Catalog, or a vendor's own driver-automation tool) and imports them into a locally staged repository, organized however the operator chooses (e.g. by OS/platform/model); the deployment tool's own responsibility is limited to selecting and injecting matching drivers from that already-staged local repository — it does not fetch, bundle, or redistribute the driver packages itself.

Source: [Populating the Out-of-Box Drivers node of MDT](https://subscription.packtpub.com/book/cloud-and-networking/9781782172499/6/ch06lvl1sec31/populating-the-out-of-box-drivers-node-of-mdt), [MDT Lite Touch Driver Management](https://www.deploymentresearch.com/mdt-2013-lite-touch-driver-management/).

Vendor-specific driver-automation tools (e.g. Lenovo's Driver Automation Tool, HP's Client Management Script Library) exist specifically to help an operator populate that local repository from the OEM's own catalog under the OEM's own terms — they are the operator's tooling choice, not something this Spike found evidence Bamep itself should embed or reimplement.

## Finding 6: Snappy Driver Installer Origin (SDIO) case study

Following owner review, Snappy Driver Installer Origin (SDIO) was evaluated as a named real-world example against the boundary above, across the roles it could plausibly occupy.

**Project status and evidence base.** SDIO is a portable, open-source Windows driver-matching/installation tool, hosted on SourceForge, with source under Subversion; the project appears actively maintained (latest known package version 2.0.0.875 via the Chocolatey community package). It was created as a community fork preserving the original "Snappy Driver Installer" project's open-source intent after trust concerns with that project's direction. Sources: [SDIO SourceForge](https://sourceforge.net/projects/snappy-driver-installer-origin/), [snappy-driver-installer.org](https://www.snappy-driver-installer.org/), [Chocolatey package](https://community.chocolatey.org/packages/sdio).

**Offline capability and driver-pack/update model.** SDIO explicitly supports a fully offline mode: driver packs can be downloaded once on a machine with Internet access, copied to portable/local storage, and used to match and install drivers on a disconnected machine. SDIO consumes aggregated driver packs that can be updated/downloaded through the tool. Primary project material — including project-maintainer discussion showing acceptance of contributed drivers sourced from the Microsoft Update Catalog or the original OEM — supports that pack content may originate from those sources. **This Spike did not establish a complete, authoritative provenance chain for every driver pack SDIO distributes or consumes.** Search results also surfaced DriverPacks.net, a separately-run community driver collection, as a name associated with SDIO's broader ecosystem, but no primary SDIO project source was found confirming it as one of exactly two authoritative upstream sources — it is mentioned here only to that limited, unconfirmed extent, not as an established fact. Source: [snappy-driver-installer.org](https://www.snappy-driver-installer.org/).

**Automation/scriptability.** SDIO has a built-in scripting engine (console-mode commands such as `init`, `select`, `install`) and command-line flags (e.g. `-autoinstall -autoclose -nogui -selfdelete`) sufficient for unattended use. A documented real-world integration exists with MDT: a PowerShell wrapper opens a firewall rule, runs SDIO to fetch fresh indexes and install matching drivers, then closes the rule — this pattern runs **live, with Internet access, during the deployment task sequence** rather than against a pre-staged offline pack set. Sources: [Using Snappy Driver Installer Origin with MDT](https://gal.vin/posts/2022/sdio-and-mdt/), SDIO Reference Manual (Glenn Delahoy).

**License: application vs. driver-pack content, kept explicitly distinct.** The SDIO *application* is released under GPLv3 (plus Creative Commons Attribution for some assets) — clearly open-source and unambiguous. This says nothing about the *driver packs* it downloads and installs: those are third-party manufacturer binaries whose complete provenance/licensing chain this Spike did not establish (see "Offline capability and driver-pack/update model" above). No explicit statement of the driver packs' own redistribution license was found on the project's own pages. **The openness of SDIO's own source code must not be read as evidence that the drivers it moves are freely redistributable** — that remains exactly the same unresolved, vendor-by-vendor question already recorded in Finding 4. No definitive legal conclusion is drawn here.

**Evaluation against the six requested angles:**

1. **Operator-side external tooling.** SDIO's offline mode (download once, install elsewhere without Internet) fits ADR-0009's operator-managed-repository boundary directly: an operator may use SDIO, a vendor's own tool, or manual OEM downloads to build the local repository Bamep consumes — Bamep does not need to know or care which. This is a concrete existing example of the pattern already decided, not a reason to change it.
2. **Bamep driver-provider Adapter.** Technically feasible (SDIO is scriptable), but the one concrete integration example found during this Spike (the MDT case study above) fetches live over the Internet during deployment — the same category of mechanism already rejected in ADR-0009 for the same reason (Internet dependency during Bamep's offline provisioning phase). This Spike found no documented example of a narrower integration limited to SDIO's *matching/selection* logic against an already-offline-staged pack set — such a pattern is not shown to exist, not merely unevaluated — and any such integration would still couple Bamep to a third-party project's pack format, index infrastructure, and matching algorithm for no evidenced requirement.
3. **Post-deployment driver installation.** Running SDIO manually or via a script inside the freshly installed Windows environment, after Bamep's own provisioning workflow has completed and the machine may have Internet access, is a distinct, separate use case from both WinPE boot-critical drivers and offline DISM injection into the target image. No requirement evidences Bamep needs to orchestrate this; it would be a genuinely separate future capability, not a replacement for the V1 boundary.
4. **Licensing and packaging boundary.** Addressed above — application license (GPLv3/CC-BY) and driver-pack content licensing are different questions; SDIO being open-source does not establish that its bundled/downloaded drivers are redistributable by Bamep.
5. **Architectural coupling.** Keeping Bamep's driver-provider Port generic (a local directory of `.inf`-based packages, format-agnostic) and treating SDIO as only one of several possible operator-side tools avoids coupling Bamep to SDIO's specific pack format, matching algorithm, index/update infrastructure, or single-project continuity risk — with no loss of capability, since DISM alone already satisfies Bamep's own injection responsibility.
6. **Limitations.** The complete provenance/licensing chain of the driver packs SDIO distributes and consumes was not established; primary project material supports Microsoft Update Catalog and/or direct-OEM origin in at least some cases, but this is not shown to be an exhaustive or exclusive list of sources. Whether DriverPacks.net (or any other aggregation path) has any explicit redistribution agreements with OEMs could not be confirmed or denied from available sources.

## Conclusion

The evidence supports a clean separation that reduces Bamep's direct role in acquiring and redistributing third-party driver packages: an operator-managed, locally staged driver repository as the driver *source*, and DISM (offline image servicing) plus WinPE build-time injection or `drvload` (runtime) as the driver *injection* mechanism Bamep itself owns. This is recorded as an architectural decision in `docs/decisions/0009-driver-provider-integration-boundary.md`. The SDIO case study (Finding 6) does not change this conclusion: it confirms the operator-side-tooling pattern is real and available today, and confirms — via SDIO's own documented live-fetch integration pattern — exactly why a Bamep-invoked online acquisition adapter was correctly rejected.

## Remaining uncertainty

- Exact licensing terms for every OEM Bamep may eventually need to support were not exhaustively reviewed — the general redistribution-restriction posture is evidenced, not a per-vendor legal clearance.
- Whether Bamep should ever build a first-party adapter that fetches from a specific vendor's driver catalog (as Lenovo's/HP's own tools do) on the operator's behalf, under that vendor's terms, is not evaluated here — no requirement for it was evidenced, and it is not designed by this Spike.
- Exact local driver-repository directory layout/format is not decided here — see the ADR's open questions.
- Complete provenance/licensing chain of the driver packs SDIO distributes/consumes was not established by this Spike; primary evidence supports Microsoft Update Catalog and/or direct-OEM origin in at least some cases, not an exhaustive source list — see Finding 6.
