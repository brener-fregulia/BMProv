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

## Conclusion

The evidence supports a clean separation Bamep can adopt without licensing exposure: an operator-managed, locally staged driver repository as the driver *source*, and DISM (offline image servicing) plus WinPE build-time injection or `drvload` (runtime) as the driver *injection* mechanism Bamep itself owns. This is recorded as an architectural decision in `docs/decisions/0009-driver-provider-integration-boundary.md`.

## Remaining uncertainty

- Exact licensing terms for every OEM Bamep may eventually need to support were not exhaustively reviewed — the general redistribution-restriction posture is evidenced, not a per-vendor legal clearance.
- Whether Bamep should ever build a first-party adapter that fetches from a specific vendor's driver catalog (as Lenovo's/HP's own tools do) on the operator's behalf, under that vendor's terms, is not evaluated here — no requirement for it was evidenced, and it is not designed by this Spike.
- Exact local driver-repository directory layout/format is not decided here — see the ADR's open questions.
