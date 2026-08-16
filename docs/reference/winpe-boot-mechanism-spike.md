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

## Follow-up: network-delivered UEFI boot path evaluation

Owner review accepted the direct ISO/El-Torito boot result above as useful evidence, but noted it proves only that WinPE boots under UEFI when delivered as removable media — not the actual network-delivered boot mechanism Bamep needs. This follow-up evaluates two candidate network bootstrap paths into WinPE, independently, in the same local virtualized environment, using current upstream sources rather than the historical FORGE PoC's bootloader choices.

### Candidate A: iPXE + wimboot + HTTP

**Provenance:**

- iPXE: built from current upstream source, commit `e6d0a97c05d238c17eeae5116cb6e9c0fc9fdb56` (2026-08-11), cloned from `https://github.com/ipxe/ipxe.git` inside WSL2 Ubuntu 24.04.1, built with `make bin-x86_64-efi/ipxe.efi` (`gcc`/`binutils` toolchain from Ubuntu 24.04's `build-essential`). Two variants were built: a plain shell-drop build, and a build with `EMBED=bamep-embed.ipxe` (an automation script: `dhcp` → `kernel http://.../wimboot` → `initrd http://.../boot.wim boot.wim` → `boot`). SHA-256 of the EMBED build: `0f0475509a27406ee55be0c59c5d9bde5f034260b5c886b2a8ae06a76d148052`.
- wimboot: downloaded from the official current release URL `https://github.com/ipxe/wimboot/releases/latest/download/wimboot` (76,064 bytes). SHA-256: `5f067ccdc4d084d5bf77b6c853bd0f8402dfc2b4cd1b103d358993ae97fae8e3`. Per current upstream documentation (`https://ipxe.org/wimboot`), this build only requires `boot.wim` — it extracts the boot manager and BCD from the WIM automatically, so the unmodified stock `winpe.wim` from the original experiment was reused unchanged.
- HTTP server: Python 3.13.14 `http.server`, serving `wimboot` and `boot.wim` from the host, reachable from the VM at `10.0.2.2:8000` (VirtualBox NAT's host-loopback address).

**How iPXE was entered — two methods attempted:**

1. **PXE bootstrap via VirtualBox NAT's built-in TFTP** (`--nat-enable-tftp1 on --nat-tftp-prefix1 ... --nat-tftp-file1 ipxe.efi`, VM boot order `net`). DHCP succeeded (`Station IP address is 10.0.2.15`), confirmed via the UEFI firmware's own `>>Start PXE over IPv4` output, but the subsequent TFTP transfer of `ipxe.efi` never completed — the firmware silently returned to its Boot Manager menu after roughly 10–15 seconds with no error message and no corresponding read of the served file (confirmed via file-access-time and HTTP-log absence). This was cross-checked carefully after an initial run gave a **false-positive appearance of success**: with the original WinPE ISO still attached as a lower-priority boot device, a failed PXE attempt silently fell through to that ISO and booted WinPE directly — producing a boot that looked identical to a successful iPXE+wimboot chain but had zero corresponding HTTP server log entries. Removing the fallback device (`--boot2 none`, DVD detached) exposed the real, honest result: PXE-via-VirtualBox-built-in-TFTP does not complete in this environment. This is recorded as a harness-specific limitation, not evaluated further.
2. **UEFI El Torito removable-media bootstrap** (our built `ipxe.efi` as `\EFI\Boot\bootx64.efi` on a UEFI-only ISO, built two ways — via `oscdimg` reusing the ADK's `efisys_noprompt.bin`, and via `xorriso`/`mtools` with a generic FAT El Torito image). This substitutes for the PXE step itself (a VirtualBox-NAT-specific limitation, not a Bamep architectural question) while still exercising the part that matters for Bamep's own HTTP-based data-plane direction (ADR-0008): whether current iPXE + current wimboot can deliver WinPE over HTTP at all.
   - The `oscdimg`/`efisys_noprompt.bin`-based ISO **did not boot our EFI application at all**: a minimal ISO containing only `\EFI\Boot\bootx64.efi` failed with firmware error `No mapping` (the disc was not recognized as a mappable boot device), even after padding it to ~66 MB with non-zero filler to rule out a size threshold. Substituting our binary into the same directory tree as the already-proven-working WinPE media (with `\sources\boot.wim`/`\Boot\BCD` present) did produce a "mappable" disc, but the resulting boot **still did not exercise our binary** — it booted WinPE directly via the ISO's own Windows Boot Manager convention, again with zero HTTP log entries. The evidence indicates `efisys.bin`/`efisys_noprompt.bin` (from the ADK's `Oscdimg` folder) is a **Windows Setup/PE-specific El Torito boot sector**, not a generic UEFI application loader — it is unsuitable for booting arbitrary EFI applications such as `ipxe.efi`, regardless of what is placed at `\EFI\Boot\bootx64.efi`. This is itself a useful, previously-undocumented finding about the ADK tooling used in the first experiment.
   - Rebuilding the ISO with `xorriso -as mkisofs -eltorito-alt-boot -e <fat-image> -no-emul-boot` (a generic FAT-based El Torito UEFI boot record, built via `mtools`/`mkfs.vfat` in WSL2) **did successfully load and execute our `ipxe.efi`**: the console showed iPXE's genuine startup banner (`iPXE initialising devices...`, `file:autoexec.ipxe... Not found (https://ipxe.org/7f4de18e)` — `7f4de18e` being iPXE's own build-identifier hash), confirmed present in both the EMBED and plain builds by `strings`-matching the binary.

**Result:** both the EMBED-automated build and the plain shell-drop build **hung indefinitely** at the same point — immediately after the `file:autoexec.ipxe`/`file:/autoexec.ipxe` local-media probe messages, before printing anything from our embedded script's first command (a plain `echo`) and before reaching an interactive shell prompt. Confirmed non-responsive to injected keystrokes after 30+ seconds. This was reproduced identically across the EMBED and non-EMBED builds, ruling out the embedded script itself as the cause. The exact root cause was not further diagnosed within this session (candidates include a VirtualBox EFI64/NAT-specific driver or timing incompatibility with this current iPXE snapshot's network/UEFI protocol probing during autoboot; not evaluated further here).

**Candidate A conclusion:** not demonstrated viable in this local environment with the current-upstream binaries tested. The boot-loading mechanism itself was proven sound (the `xorriso` UEFI El Torito path genuinely executes arbitrary EFI applications — iPXE's own banner is decisive evidence of that), so the failure is attributable to iPXE's own runtime behavior in this environment, not to the test harness's boot-chain mechanics. wimboot itself was never actually exercised, since iPXE never reached the `kernel`/`initrd` commands.

### Candidate B: GRUB x86_64-efi chainload into WinPE

**Provenance:** GRUB `2.12-1ubuntu7.3` (Ubuntu 24.04.1 LTS's current `grub-efi-amd64-bin`/`grub-mkstandalone` package — a current, actively-serviced distribution build, not the historical FORGE PoC's binary). Built as a standalone `grubx64.efi` via `grub-mkstandalone -O x86_64-efi --modules="part_gpt part_msdos iso9660 fat udf normal chain configfile echo sleep [ls]"` with an embedded `grub.cfg`. SHA-256 of the standalone image: `dc3f7377f86d78318359224b4e1e55700be25cad7f25af290d6b7d4738c537e7`.

**How GRUB was booted:** via the same proven-working generic UEFI El Torito mechanism as Candidate A's successful boot-loading step (`xorriso`-built FAT El Torito image containing `grubx64.efi` as `\EFI\Boot\bootx64.efi`), combined on the same optical disc with the outer ISO9660/UDF volume containing the **unmodified, restored** WinPE media tree (`\EFI\Boot\bootx64.efi` = the genuine WinPE Windows Boot Manager, `\Boot\BCD`, `\Boot\boot.sdi`, `\sources\boot.wim` — the same layout Experiment 0 proved bootable directly). Both artifacts are therefore **local to the same disc**, not network-delivered, for this specific test — see "Remaining uncertainty" below for why this scoping choice matters.

**Chainload target:** `(cd0)/EFI/Boot/bootx64.efi`, where `(cd0)` is the outer ISO9660/UDF data track and the target is the genuine WinPE Windows Boot Manager at the same path Experiment 0 already proved bootable directly.

**Result:** GRUB itself booted and ran correctly — `set timeout`, `echo`, and script control flow all executed as expected, proving GRUB 2.12 genuinely boots via UEFI in this environment. The `chainloader (cd0)/EFI/Boot/bootx64.efi` command itself failed with `error: unknown error.`; the subsequent `boot` command then failed too (nothing was loaded), and the script reached its final `echo` line cleanly — a clean, deterministic, non-hanging failure, not a crash or freeze.

A diagnostic follow-up run (`ls`, `ls (cd0)/`, `ls (cd0)/EFI/Boot/`) established that this is **not a path-resolution or device-naming problem**: `(cd0)` correctly enumerated the entire outer WinPE media tree, and `EFI/Boot/bootx64.efi` was listed exactly where expected. What the evidence establishes is limited to this: GRUB successfully resolved and read the target EFI file (proven by `ls` and by `chainloader` reaching the point of acting on it), `chainloader` then failed with `error: unknown error`, and BCD/`boot.sdi`/`boot.wim` processing was never reached. The evidence does **not** establish which specific EFI Boot Service call failed or why — that would require deeper GRUB-internal diagnostics not attempted in this session.

Brief research into this exact error class (`chainloader` + `bootmgfw.efi` + `unknown error`) surfaced it as a documented, recurring category of GRUB report, not an artifact unique to this test: community reports describe a GRUB `chainloader` requirement to export a proper UEFI device handle/DevicePath for the source location so firmware's `LoadImage` can resolve it — a distinct requirement from GRUB's own internal file-reading capability (used by `ls`) — as one plausible explanation, alongside a separate reported uninitialized-variable bug in some GRUB versions producing the same generic message. **Neither is confirmed as the actual cause here** — both are documented possibilities for this error class, offered as context, not as an established root cause for this specific run. Source: GNU GRUB help-list archive and community reports surfaced via search (not independently re-verified against GRUB 2.12's exact source for this session).

**Candidate B conclusion:** GRUB chainloading into the Windows/WinPE EFI boot path **did not succeed** in this specific test layout (GRUB loaded from an El Torito FAT partition, target on the sibling ISO9660/UDF track of the same physical optical disc). The failure occurred inside the EFI chainloader handoff, after file/path resolution had already succeeded — not at path resolution, and not at BCD/artifact discovery, since chainloading never proceeded far enough to reach that stage. The precise failing Boot Service call and its root cause were not established; the DevicePath/handle-export explanation above is recorded as a plausible, documented candidate, not a proven cause. Whether this specific "same-disc, cross-track" test layout's failure would also affect GRUB's much more common and better-supported network-boot chainload path (GRUB loaded via PXE/TFTP, target delivered via TFTP/HTTP, both over a single well-defined network device handle rather than two different optical-disc filesystem tracks) is explicitly **not established by this test** — see "Remaining uncertainty." No further GRUB experimentation was performed in this round, per scope.

### Candidate comparison

| Dimension | Candidate A (iPXE + wimboot + HTTP) | Candidate B (GRUB chainload) |
|---|---|---|
| Empirically viable in this round | No — iPXE hangs before reaching wimboot | No — `chainloader` fails cleanly before reaching BCD/artifact discovery |
| Failure mode | Silent hang, unresolved cause | Clean, deterministic error, plausible documented cause (UEFI handle export) |
| Moving components | iPXE binary + wimboot binary + HTTP server + boot.wim | GRUB binary + target Windows Boot Manager + BCD/boot.sdi/boot.wim, already co-located |
| HTTP/network-delivery suitability | Designed for it (wimboot's whole purpose); never actually exercised here | Not exercised here at all (local chainload only); GRUB's `http`/`tftp` modules exist but weren't tested |
| TFTP dependency after bootstrap | None if HTTP delivery works (wimboot is HTTP-native via iPXE) | Not evaluated — this test used local media, not TFTP/HTTP delivery for the target |
| WinPE-specific complexity | Low once working — wimboot auto-extracts boot manager/BCD from `boot.wim` alone | Low in principle (single `chainloader` call), but blocked before that simplicity could be exercised |
| Consistency with offline-after-local-artifacts boundary | Consistent — HTTP delivery from a local Bamep-controlled server, no external Internet dependency | Consistent in principle — chainload target could equally be local or network-delivered |
| Secure Boot / Issue #10 implications | Not evaluated (Secure Boot off); iPXE's own EFI signing posture would need Issue #10 attention if pursued | Not evaluated (Secure Boot off); GRUB's shim/signing story is the more mature, widely-deployed path for Secure Boot in the wider ecosystem, worth keeping in mind for Issue #10, but not decided here |
| Isolation behind the Boot Port/Adapter boundary | Yes — an Adapter-level mechanism choice | Yes — an Adapter-level mechanism choice |

Both candidates, regardless of outcome, remain Adapter-level mechanism choices; neither result changes the Domain-level Boot Orchestration boundary already accepted (`m0-stack-and-boundaries-baseline.md`).

Neither candidate is chosen or rejected as Bamep's eventual mechanism based on this round — both failures are specific to this local test harness's exact configuration, not evidenced as fundamental incompatibilities with Bamep's requirements.

### Remaining uncertainty (this follow-up)

- **Candidate A's hang cause is undiagnosed.** Whether it is a VirtualBox EFI64/NAT-specific issue, a timing race in current iPXE's network/UEFI protocol probing, or something else was not established. A live interactive-shell debugging session (rather than an embedded/automated script) is the natural next diagnostic step.
- **Candidate B was tested only as a local, same-disc chainload**, not as a network-delivered (PXE/TFTP or HTTP) chainload — the scenario actually closer to Bamep's production need. The documented "UEFI device handle export" failure category is plausibly specific to the unusual same-disc, cross-track layout used here and may not occur when GRUB is loaded via PXE and chainloads a target delivered via TFTP/HTTP instead — this was not tested.
- **VirtualBox's built-in NAT TFTP server did not work reliably** in this environment for PXE bootstrap of a boot loader, independent of which loader was used — this is a harness limitation to note for any future local network-boot testing, not a Bamep architectural finding.
- Neither candidate's Secure Boot posture was evaluated (owned by Issue #10).
- Physical Integration Environment validation (real PXE/DHCP/TFTP infrastructure, real UEFI firmware, real NICs) remains entirely outstanding — not established by this or the prior round.
- A reasonable, low-cost next step if the owner wants further evidence before physical validation: retest Candidate B with GRUB loaded via the same VirtualBox NAT TFTP path (once/if that is made to work) chainloading a target served over HTTP, which would answer the network-delivered question directly.

## Final local diagnostic round: released iPXE control experiment

Purpose: distinguish a failure specific to the self-built git-master iPXE snapshot / its native NIC-driver path, from a broader incompatibility of iPXE itself in this VirtualBox environment. Per scope, this round did not perform further GRUB experimentation, did not debug iPXE source or use extensive `DEBUG=` builds, and did not try alternate hypervisors or older iPXE versions.

**Provenance — official release, independent of the self-built snapshot:** iPXE **v2.0.0**, published 2026-03-06, commit `12798ec` (release build identifier `g12798`, visible in-product as `iPXE 2.0.0 (g12798)`), downloaded from the official GitHub Releases page (`https://github.com/ipxe/ipxe/releases/tag/v2.0.0`):

- `ipxeboot.tar.gz` (`https://github.com/ipxe/ipxe/releases/download/v2.0.0/ipxeboot.tar.gz`, 12,002,760 bytes), from which two prebuilt x86_64 UEFI binaries were extracted:
  - `ipxeboot/x86_64/ipxe.efi` — the native NIC-driver build. SHA-256: `868aa34057ff416ebf2fdfb5781de035e2c540477c04039198a9f8a9c6130034`.
  - `ipxeboot/x86_64/snponly.efi` — the UEFI Simple Network Protocol (SNP)-only build, using the firmware-provided network stack instead of iPXE's own NIC drivers. SHA-256: `f61c2ce34e05d7d857633df2e512d547df75b6aa18b2da152a7c9af222cfe28f`.

Both were booted using the already-proven generic UEFI El Torito/FAT mechanism (`xorriso`/`mtools`), independent of VirtualBox's PXE/TFTP harness, exactly as for the git-master snapshot.

**Result — official `ipxe.efi` (native driver path):** hung at the identical point as the self-built git-master snapshot — immediately after the `file:autoexec.ipxe`/`file:/autoexec.ipxe` probe messages, with no further output observed after 25+ seconds. This is the same failure signature as the earlier self-built binary. **This rules out the self-built snapshot as the cause**: the hang reproduces with an official, independently-provenanced stable release using the native NIC-driver path.

**Result — official `snponly.efi` (SNP path):** did **not** hang. It printed its full startup banner (`iPXE 2.0.0 (g12798) -- Open Source Network Boot Firmware -- https://ipxe.org`, feature list `DNS HTTP HTTPS iSCSI TFTP VLAN AoE EFI Menu`), then reported `No more network devices`, and **exited back to firmware** — confirmed by VirtualBox's firmware log showing a second, independent boot attempt at the same CD-ROM device immediately afterward, settling at `BdsDxe: No bootable option or device was found` / the firmware Boot Manager Menu. This is a clean, deterministic, non-hanging outcome, but it also does not reach a usable interactive iPXE shell — with no network device found via SNP, iPXE terminated rather than presenting a shell.

**Interpretation, precisely bounded to what was observed:** the native-driver x86_64 UEFI build of iPXE (both the self-built git-master snapshot and the official v2.0.0 release) hangs early in this VirtualBox EFI64/NAT environment, before reaching any shell. The SNP-based build does not hang — it runs to a determinate conclusion — but finds no network device exposed via UEFI SNP on this VM's virtual NIC (`82540EM`/Intel PRO/1000 emulation) and exits without a shell. This distinguishes the two failure categories the task asked to separate: **the hang is a native-driver-path behavior, not a general "iPXE cannot run in this VirtualBox environment" failure** — a differently-built iPXE binary genuinely executes and reaches a determinate (if network-less) state in the same environment. Neither path reached a usable, network-capable shell, so per the bounded stop condition, wimboot was **not** exercised in this round (Step 3 was correctly skipped: iPXE never reached a usable shell on either tested path).

**Why SNP found no device is not established.** Plausible, undiagnosed candidates include: VirtualBox's `EFI64` firmware not exposing a Simple Network Protocol instance for the emulated `82540EM` NIC (i.e., only its own native/PXE Base Code interface is present, not a generic SNP handle), a NIC-model-specific limitation, or a VirtualBox-EFI-implementation gap. This was not investigated further, consistent with the bounded scope of this round.

Per the bounded stop condition for this round: the stable/released iPXE paths also failed to reach a usable shell before this round's scope closed. Local investigation stops here — no further versions, alternate hypervisors, source patches, or deep iPXE debugging were attempted. Network-delivered WinPE viability in this VirtualBox environment remains unresolved and requires physical Integration Environment validation.

## Overall conclusion for Issue #8 (all rounds)

- **UEFI boot of WinPE itself is empirically established** as viable, in this virtualized environment, when delivered as removable/optical media (the original experiment) — reproducible, with working inbox network and storage drivers.
- **Network-delivered WinPE boot viability (the mechanism Bamep actually needs) is not established** in this local VirtualBox environment. Both evaluated candidates failed before delivering `boot.wim`:
  - iPXE + wimboot + HTTP: blocked by an iPXE-specific hang in its native NIC-driver path, reproduced identically across a self-built current-upstream snapshot and an official stable v2.0.0 release; an SNP-based release build avoids the hang but finds no usable network device in this environment, so wimboot itself was never reached on any tested path.
  - GRUB chainload: blocked by a clean, deterministic `chainloader` failure occurring after successful file/path resolution but before reaching BCD/artifact discovery; only tested as a same-disc local chainload, not a network-delivered one.
- Both failures are specific to this local VirtualBox test harness as configured; neither is evidenced as a fundamental incompatibility with Bamep's architecture, and neither required inventing or assuming FORGE PoC bootloader behavior.
- This Spike's own evaluation/success criteria (`docs/development/sdd.md` "Technical Spikes") explicitly permit concluding with documented uncertainty rather than a definitive mechanism choice: "Sufficient evidence to determine a viable WinPE boot mechanism, **or to document why viability remains uncertain**." That is the outcome reached here for the network-delivered case.
- Physical Integration Environment validation (real PXE/DHCP/TFTP infrastructure, real UEFI firmware, real NICs) remains required future work before any network-delivered boot mechanism can be considered validated for Bamep, regardless of which candidate (if any) is pursued further.

## Related work

- Issue #8 — `[Spike] Validate WinPE boot mechanism` (this Spike).
- `docs/reference/hardware-compatibility.md` — prior, unrelated FORGE PoC bootloader-chain findings.
- `docs/decisions/0009-driver-provider-integration-boundary.md`, `docs/reference/driver-provisioning.md` — the driver-injection boundary this boot mechanism's driver needs (if any, on real hardware) would consume.
- `docs/specifications/m0-stack-and-boundaries-baseline.md` "Boot-orchestration architectural boundary" — explicitly deferred the Boot Orchestrator's concrete mechanism pending this Spike's evidence; this document's findings are offered as input to that already-`Done` Work Package's owner-authorized amendment, not applied here (see Issue #1's Specification is already `Done` — any amendment requires separate, explicit owner authorization, consistent with the pattern already used for ADR-0009's recommended amendment).
- Issue #10 — `[Spike] Validate Secure Boot and hardened boot chain` — Secure Boot remains unaddressed here.
