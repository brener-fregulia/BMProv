# WinPE Boot Mechanism — UEFI Boot Viability (Local Virtualized Evidence)

## Question

Determine the viable WinPE boot mechanism for Bamep without treating FORGE PoC behavior as accepted Bamep architecture (Issue #8, `[Spike] Validate WinPE boot mechanism`).

## Why existing evidence was insufficient

`docs/reference/hardware-compatibility.md` records only bootloader-chain experiments from the FORGE PoC (pxelinux/ipxe/snponly/GRUB), explicitly not extended to WinPE and explicitly not binding for Bamep. WinPE-specific UEFI boot had never been exercised for Bamep at all before this Spike.

## Constraints and assumptions

- FORGE PoC bootloader choices are not treated as Bamep requirements.
- UEFI x86-64 is the V1 target; Legacy BIOS is out of scope and was not exercised.
- WinPE production implementation/customization is out of M0 scope; this Spike investigates boot *viability*, not a production build.
- Secure Boot is explicitly out of this Spike's scope (owned by Issue #10) — the test environment had Secure Boot **off**.

## Environment scoping decision

The Issue anticipated this likely requires Integration Environment access. Before running any experiment, the local environment was inventoried and the scoping choice was put to the owner explicitly (per `docs/development/testing.md`'s caution that local virtualization must not be treated as a faithful substitute for firmware/UEFI/PXE behavior). The owner explicitly authorized a **local virtualized approximation** using this machine's existing Windows ADK tooling and VirtualBox, with the explicit instruction that results be recorded as virtualized-environment evidence, not as a substitute for future physical Integration Environment validation.

## Method

**Toolchain installed/used (exact versions):**

- Windows ADK: **10.1.26100.2454** (installed via `adksetup.exe /quiet /features OptionId.DeploymentTools`, confirmed via install log `WixBundleVersion`).
- Windows PE add-on for the ADK: **10.1.26100.2454** (installed via `adkwinpesetup.exe /quiet /features OptionId.WindowsPreinstallationEnvironment`), matching the ADK version.
- Resulting WinPE build, as reported by DiskPart inside the booted environment: **10.0.26100.1**.
- Oracle VirtualBox: **7.2.14r174565** (`VBoxManage --version`).
- VM firmware: `EFI64` (VirtualBox's UEFI implementation), chipset `piix3`, `SecureBoot=off`.

**Media construction (worked around a real environment constraint, recorded as a limitation):** `copype.cmd` (the ADK's standard WinPE workspace tool) failed with DISM error 740 ("elevated privileges required") because this session's shell is not running elevated, and no interactive UAC elevation is available in this non-interactive tool environment. `copype.cmd`'s only use of the elevated DISM mount step is to extract `bootmgfw.efi`/`bootmgfw_EX.efi` copies for later dual-signature servicing scenarios — not required for a basic boot-viability test, since the ADK's static `Media` template already ships a working `EFI\Boot\bootx64.efi` (the default UEFI removable-media boot path) and the `Deployment Tools\amd64\Oscdimg` folder already ships `efisys_noprompt.bin` (the UEFI El Torito boot descriptor), neither of which requires mounting the WIM. The WinPE media was therefore assembled manually, without any DISM mount and without administrator elevation:

1. Copied the ADK's static `Windows Preinstallation Environment\amd64\Media` template verbatim to a working directory.
2. Copied the stock, **unmodified** `amd64\en-us\winpe.wim` to `media\sources\boot.wim` (no customization, no injected drivers, no `startnet.cmd` changes beyond the ADK default).
3. Built a **UEFI-only** bootable ISO with `oscdimg.exe -m -o -u2 -udfver102 -bootdata:1#pEF,e,b"efisys_noprompt.bin" <media_dir> <iso>` — a single El Torito boot entry, platform `EF` (UEFI), no Legacy/BIOS boot record, consistent with Bamep's UEFI-only V1 scope.

**VM configuration:** `VBoxManage createvm --ostype Windows11_64`, `modifyvm --firmware efi64 --boot1 dvd --nic1 nat`, IDE DVD drive with the built ISO attached. A second boot added a SATA/AHCI controller with a blank 2048 MB virtual disk to test storage-controller visibility.

**Observation method:** the VM was started headless; behavior was captured via `VBoxManage controlvm <vm> screenshotpng` at timed intervals, and via `keyboardputstring`/`keyboardputscancode` to drive simple diagnostic commands (`ipconfig /all`, `diskpart` → `list disk`) into the booted environment's console.

## Evidence observed

1. **UEFI boot succeeded** using the unmodified stock `winpe.wim` and the ADK's static `EFI\Boot\bootx64.efi`, with no DISM-mounted/customized content. At ~15 seconds after power-on, a screenshot showed the environment already at `X:\Windows\System32>` running `wpeinit` (WinPE's standard PnP/network initialization). At ~25–30 seconds, a second screenshot showed a fully initialized, elevated (`Administrator:`) command shell at `X:\Windows\System32>`.
2. **Reproducibility**: the boot sequence was repeated (VM power-cycled to add a disk) and reached the same `wpeinit`-running state at the same ~15-second mark on the second boot, with identical console title-bar progression.
3. **Network stack and inbox NIC driver**: `ipconfig /all` showed the VM's emulated "Intel(R) PRO/1000 MT Desktop Adapter" fully recognized by WinPE's **inbox** driver set (no injection needed for this emulated NIC), with a successful DHCP lease (IPv4 `10.0.2.15`, gateway `10.0.2.2`, plus IPv6 addresses) obtained automatically via `wpeinit`.
4. **Storage stack and inbox AHCI driver**: after attaching a SATA/AHCI virtual disk and rebooting, `diskpart` → `list disk` showed `Disk 0, Online, 2048 MB` correctly recognized via WinPE's inbox AHCI driver, confirming disk visibility without injection for this emulated controller.
5. **WinPE build identification**: DiskPart self-reported version `10.0.26100.1` inside the booted environment, consistent with the installed ADK/WinPE add-on version.

Screenshot evidence (not committed to the repository; local artifacts from this session) is retained at `C:\BamepSpike\vm\screenshot_*.png` on the development machine where this Spike was executed, alongside the built ISO (`BamepWinPE-amd64.iso`) and VM definition, for reproducibility reference.

## Conclusion

**WinPE boots successfully under UEFI firmware** (VirtualBox's `EFI64` implementation) from a UEFI-only El Torito ISO built entirely from stock ADK 10.1.26100.2454 / WinPE 10.0.26100.1 components, reaching a fully initialized, network- and disk-capable shell in well under a minute, reproducibly. This is the first empirical evidence Bamep has for WinPE boot viability at all — prior evidence (`docs/reference/hardware-compatibility.md`) covered only pre-WinPE bootloader-chain behavior from the unrelated FORGE PoC.

This evidence supports concluding that **UEFI boot of WinPE is architecturally viable for Bamep** and is not blocked by any fundamental incompatibility, at least under the tested virtualized firmware.

## Remaining uncertainty (not established by this Spike)

- **This is virtualized-environment evidence, not physical Integration Environment evidence**, per the owner's explicit scoping decision. Real UEFI firmware from different hardware vendors, real physical NIC/storage controllers not covered by WinPE's inbox driver set (the actual production concern `docs/decisions/0009-driver-provider-integration-boundary.md` and `docs/reference/driver-provisioning.md` Finding 2 address), and real firmware quirks are not exercised here.
- **The production PXE → bootloader (e.g. GRUB) → WinPE chainload path was not tested.** This experiment booted WinPE directly from its own El Torito UEFI boot record on optical/ISO media, not via network PXE boot or via chainloading from a bootloader such as `grubx64.efi` (the FORGE PoC's previously-stable choice, `docs/reference/hardware-compatibility.md`). Whether a GRUB-mediated chainload to WinPE's `bootmgfw.efi`/`bootx64.efi` behaves identically is not established here and is a natural, low-cost follow-up experiment in the same local environment if the owner wants it before physical validation.
- **Secure Boot was off.** Secure Boot / hardened boot-chain behavior remains entirely owned by Issue #10, not addressed here.
- **DISM elevation limitation**: `copype.cmd`'s standard workflow could not be used as-is in this session's non-elevated environment; the workaround (manual media assembly, bypassing only the `bootmgfw.efi`/`bootmgfw_EX.efi` extraction step) is believed equivalent for boot-viability purposes but was not cross-verified against a `copype`-produced ISO in this session.
- **VirtualBox's UEFI implementation (`EFI64`) is not identical to any specific physical mainboard firmware.** It is a real, spec-following UEFI implementation, not a simulation of DOS/BIOS-only behavior, but vendor-specific UEFI quirks (as already observed across vendors in the FORGE PoC for pre-WinPE bootloaders) are not covered.

## Related work

- Issue #8 — `[Spike] Validate WinPE boot mechanism` (this Spike).
- `docs/reference/hardware-compatibility.md` — prior, unrelated FORGE PoC bootloader-chain findings.
- `docs/decisions/0009-driver-provider-integration-boundary.md`, `docs/reference/driver-provisioning.md` — the driver-injection boundary this boot mechanism's driver needs (if any, on real hardware) would consume.
- `docs/specifications/m0-stack-and-boundaries-baseline.md` "Boot-orchestration architectural boundary" — explicitly deferred the Boot Orchestrator's concrete mechanism pending this Spike's evidence; this document's findings are offered as input to that already-`Done` Work Package's owner-authorized amendment, not applied here (see Issue #1's Specification is already `Done` — any amendment requires separate, explicit owner authorization, consistent with the pattern already used for ADR-0009's recommended amendment).
- Issue #10 — `[Spike] Validate Secure Boot and hardened boot chain` — Secure Boot remains unaddressed here.
