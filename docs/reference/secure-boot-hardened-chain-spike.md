# Secure Boot and Hardened Boot Chain — Local Virtualized Evidence

## Question

Determine the practical constraints and viable approaches for Secure Boot or an acceptably hardened boot chain for Bamep's target environment, and help determine whether Secure Boot is required, not only how to implement it (Issue #10, `[Spike] Validate Secure Boot and hardened boot chain`).

## Why existing evidence was insufficient

`docs/discovery/architecture-redesign.md` ("Security invariants") states boot-chain integrity is a requirement even though Secure Boot itself is not an M0 implementation requirement, but no prior evidence established what is practically achievable. This Spike does not revisit or decide the "is Secure Boot required" scope question itself — that remains an owner/architectural decision — but supplies evidence relevant to it.

## Constraints and assumptions

- UEFI x86-64 is the V1 target; Legacy BIOS is out of scope.
- Secure Boot is not assumed mandatory for M0.
- Reuses the artifacts, ADK/WinPE tooling, VM, and build toolchain already prepared for Issue #8 (`docs/reference/winpe-boot-mechanism-spike.md`), per owner authorization.

## Environment scoping decision

As with Issue #8, the owner was asked how to scope this Spike given its own "likely requires Integration Environment access" caveat. The owner authorized a **local virtualized approximation**: VirtualBox 7.2.14 genuinely supports Secure Boot enforcement (`VBoxManage modifynvram`), using the same standard Microsoft certificate template (PK/KEK/db) that real OEM firmware ships by default — not a synthetic or Bamep-specific trust store. All results below are recorded explicitly as **virtualized-firmware evidence**, not a substitute for physical Integration Environment validation, consistent with `docs/development/testing.md`'s caution about UEFI/firmware behavior.

## Method

**Secure Boot configuration (exact commands):**

```text
VBoxManage modifynvram "BamepSpike-WinPE-UEFI" inituefivarstore
VBoxManage modifynvram "BamepSpike-WinPE-UEFI" enrollmssignatures
VBoxManage modifynvram "BamepSpike-WinPE-UEFI" enrollorclpk
VBoxManage modifynvram "BamepSpike-WinPE-UEFI" secureboot --enable
```

`enrollmssignatures` enrolls VirtualBox's built-in copies of the standard Microsoft KEK and db/dbx certificates (the Microsoft Corporation UEFI CA / Windows Production CA chain found in real OEM firmware defaults). `enrollorclpk` enrolls VirtualBox's own default Platform Key (PK) — the PK identifies the platform owner (here, VirtualBox/Oracle's default), not Microsoft; this is analogous to an OEM's own PK on physical hardware, which coexists with the Microsoft-issued KEK/db entries that actually validate Microsoft- and vendor-signed bootloaders. Confirmed active via `VBoxManage showvminfo --machinereadable | grep -i secureboot` → `SecureBoot="on"` before and after every scenario below.

Boot observation used the same method as Issue #8: headless VM, `VBoxManage controlvm screenshotpng` at timed intervals, `keyboardputstring`/`keyboardputscancode` for interactive commands.

## Scenario 1: stock WinPE / Microsoft-signed boot path

**Target:** the unmodified stock WinPE UEFI ISO from Issue #8's original experiment (`BamepWinPE-amd64.iso`) — ADK 10.1.26100.2454, WinPE build 10.0.26100.1, `EFI\Boot\bootx64.efi` = the Windows Boot Manager as shipped by Microsoft's Windows ADK, unmodified.

**Trust store relevant to this scenario:** the enrolled Microsoft KEK/db chain (`enrollmssignatures`) — this is exactly the certificate chain the Windows Boot Manager is signed against in production.

**Result:** boot succeeded, indistinguishable in timing and behavior from the non-Secure-Boot baseline in `docs/reference/winpe-boot-mechanism-spike.md` — `wpeinit` reached at ~15 seconds, fully initialized elevated shell (`Administrator: X:\windows\system32\cmd.exe`) at ~30 seconds. No rejection, no warning, no fail-closed message. **This is the expected, positive result**: Microsoft-signed code validates cleanly against the standard db chain.

**What this proves:** Bamep's currently-assumed WinPE artifact (stock ADK output) is already Secure-Boot-compatible against the standard default trust store, with no additional signing work needed for this specific artifact, in this virtualized environment.

**What this does not prove:** behavior on real OEM firmware, whose exact db/dbx contents and revocation state may differ from VirtualBox's defaults (see "Remaining uncertainty").

## Scenario 2: untrusted / unsigned EFI bootloaders

**Targets:** two unsigned binaries already built for Issue #8, reused unmodified:

- Official iPXE v2.0.0 release `ipxe.efi` (native driver build), SHA-256 `868aa34057ff416ebf2fdfb5781de035e2c540477c04039198a9f8a9c6130034` — not signed by any enrolled authority.
- Self-built standalone GRUB `grubx64.efi` (via `grub-mkstandalone`, GRUB 2.12-1ubuntu7.3), SHA-256 `dc3f7377f86d78318359224b4e1e55700be25cad7f25af290d6b7d4738c537e7` — self-built, not signed.

**Result — both, identically:**

```text
BdsDxe: failed to load Boot0001 "UEFI VBOX CD-ROM ..." from PciRoot(0x0)/Pci(0xD,0x0)/Sata(0x1,0xFFFF,0x0): Access Denied
```

Immediate, clean, deterministic rejection — no hang, no partial execution, no output from either binary (neither iPXE's own startup banner nor GRUB's `echo` commands ever appeared). This is a **distinct error signature** from the ones observed in Issue #8's non-Secure-Boot testing (`No mapping` for a structurally-unrecognized disc, `error: unknown error` for GRUB's own chainloader failure) — `Access Denied` is specifically UEFI firmware's Secure Boot signature-validation rejection, confirming enforcement is genuinely active and discriminating, not merely coincidentally blocking these binaries for an unrelated reason.

**What this proves:** Secure Boot enforcement in this environment is real and fail-closed for unsigned code — exactly the expected, positive safety behavior. Neither iPXE nor GRUB is "rejected as unsuitable"; both are simply unsigned in the exact builds tested here, which is the expected state for a self-built or unmodified upstream binary with no signing step applied.

## Scenario 3: shim + officially-signed GRUB (Ubuntu packages)

**Provenance:**

- `shim-signed` **1.58+15.8-0ubuntu1** (Ubuntu 24.04.1 LTS current package), binary `/usr/lib/shim/shimx64.efi.signed`, SHA-256 `6fe6e1bcbe6cf6baec8e056d40361ca1aa715cc04ddcc2855351de060b84350b`. Per Debian/Ubuntu packaging, this `shimx64.efi` is signed by Microsoft's UEFI CA (the same authority already enrolled via `enrollmssignatures`) — shim's entire purpose is to be the one component every mainstream Secure-Boot-enabled firmware already trusts, which then extends trust to a distribution-specific second stage.
- `grub-efi-amd64-signed` **1.202.5+2.12-1ubuntu7.3** (GRUB 2.12), binary `/usr/lib/grub/x86_64-efi-signed/grubx64.efi.signed`, SHA-256 `a831af01e4fb5e3c9457120e1d08ea13d98a0a47b62728c284b7f502d535965c` — signed by Canonical, validated by shim (not directly by firmware) via shim's embedded vendor certificate/trust database, not the firmware's own db.

**Chain assembled:** `\EFI\Boot\BOOTX64.EFI` = `shimx64.efi.signed` (the firmware-trusted entry point), `\EFI\Boot\grubx64.efi` = `grubx64.efi.signed` (shim's default expected second-stage filename, per upstream/Debian shim convention), `\EFI\Boot\mmx64.efi` = shim's MOK Manager (included for completeness, not exercised). Built via the same `xorriso`/`mtools` generic UEFI El Torito mechanism proven in Issue #8.

**Result:** boot succeeded through the full two-stage signed chain — firmware validated and executed `shimx64.efi.signed` (no `Access Denied`), which validated and chainloaded `grubx64.efi.signed`, which started and reached a genuine interactive `GNU GRUB version 2.12` rescue prompt (`grub>`) — no `grub.cfg` was supplied on this minimal test disc, so GRUB fell back to its rescue shell rather than a full menu, which is expected and does not affect the trust-chain result.

**Attempted follow-up — chainload from the signed GRUB session into WinPE:** with the Issue #8 stock WinPE ISO attached as a second optical device, `ls (cd1)/` and `ls (cd2)/` (the other attached optical/placeholder devices) did not show the WinPE content, and `ls (cd0)/` — the device carrying the WinPE ISO — returned `error: unknown filesystem`, as did `ls (hd0)/` (the blank SATA test disk from Issue #8). **This attempt did not reach the chainload step at all** — it failed earlier, at GRUB's own filesystem recognition of the WinPE disc's UDF/ISO9660 hybrid format, a different and earlier failure point than Issue #8's `chainloader ... error: unknown error` (which occurred *after* successful file listing/resolution). This is recorded as **inconclusive, not attempted further** within this round's bounded scope — plausible candidates include a different module set in the official signed GRUB build versus the custom `grub-mkstandalone` build used in Issue #8 (e.g., missing or differently-loaded `udf`/`iso9660` modules), not diagnosed further here.

**What this proves:** a fully signature-verified two-stage boot chain (Microsoft-trusted shim → Canonical-signed GRUB) is achievable and does pass Secure Boot validation end-to-end in this environment, using off-the-shelf, officially-signed distribution packages — Bamep is not required to obtain its own Microsoft signing arrangement merely to have *a* working signed chain primitive available. Whether that specific chain can then reach WinPE was not established in this round.

## Trust-bootstrap implications (evidence-informed, not a decision)

The owner asked whether an accepted Secure Boot chain could provide a trustworthy point to deliver the Server TLS fingerprint / enrollment context to the Agent (`docs/specifications/m0-agent-protocol-contract.md` "Transport and handshake": *"The Server's certificate fingerprint must be delivered to the Agent through an authenticated, integrity-protected boot mechanism. This Specification does not assume the current boot chain already provides that assurance."*). This Spike does not choose a PKI, signing strategy, or production mechanism, and does not reopen ADR-0005's WSS transport decision — it only records what the evidence above does and does not support.

- **What Secure Boot verifies**: only the *code* identity/integrity of each executable stage as it is loaded (Scenario 2's `Access Denied` vs. Scenarios 1/3's clean execution demonstrate this concretely). It does **not** itself authenticate or protect any *data* payload (such as a TLS fingerprint) carried alongside or embedded within that code, beyond whatever integrity the code's own signature already covers.
- **Where the verified chain of trust would terminate**: at the last signature-verified executable stage that actually runs. Scenario 3 shows this can be a distribution-signed GRUB reached via a Microsoft-trusted shim; Scenario 1 shows it can be the Microsoft-signed Windows Boot Manager itself. Anything that stage subsequently loads or reads *without its own additional verification* (an unsigned script, an unsigned data file, an unsigned next-stage binary) is no longer covered by Secure Boot's guarantee — chaining trust further requires either another signed stage (as shim → GRUB demonstrates) or an application-level integrity check performed by the last trusted stage itself.
- **What Bamep would eventually need to sign or verify, if this direction is pursued**: whatever component is the last stage responsible for making the Server TLS fingerprint / enrollment context available to the Agent — e.g., a signed first-stage loader or signed Agent-launching component, or a signed/verified data artifact that a trusted loader reads and hands to the Agent. This Spike does not specify which.
- **Non-decision, explicitly recorded**: the evidence here shows the underlying trust-chain *primitive* (firmware → shim → signed second stage) is available and functions in this environment. It does **not** establish a specific mechanism for delivering the Server fingerprint through that chain to the Agent — that remains undesigned. This is flagged as an open implication for later review (feeding Issue #2's endpoint identity/trust model and Issue #3's Agent Protocol contract, per their own already-recorded open questions), not resolved by this Spike, and this Spike does not claim the currently-assumed bootstrap is either sufficient or insufficient — only that a viable building block exists.

## Conclusion

Secure Boot enforcement, using the standard default Microsoft trust store, behaves correctly and predictably in this virtualized environment: it cleanly accepts already-Microsoft-signed code (Scenario 1), cleanly and unambiguously rejects unsigned code with a distinct fail-closed error (Scenario 2), and cleanly accepts a legitimate two-stage signed chain built from off-the-shelf distribution packages (Scenario 3). This is evidence that Secure Boot is **practically viable** for Bamep's UEFI x86-64 target, should the owner decide to require it — no fundamental obstacle was found. Whether Secure Boot should be *required* for Bamep remains an owner/architectural decision this Spike does not make; the evidence here is offered as input to that decision, not a substitute for it.

## Remaining uncertainty

- **This is virtualized-firmware evidence, not physical Integration Environment evidence.** Real OEM firmware's exact db/dbx contents, revocation lists, and Secure Boot implementation quality vary by vendor and are not represented by VirtualBox's default template.
- **The signed shim+GRUB chain's ability to reach WinPE was not established** — the attempt failed earlier (GRUB filesystem recognition of the WinPE disc) than Issue #8's own GRUB chainload finding, for reasons not diagnosed in this round.
- **MOK (Machine Owner Key) enrollment** — the mechanism by which a party other than Microsoft/Canonical could get their own signed second-stage binary trusted by shim without going through Microsoft's signing service — was not evaluated; `mmx64.efi` (MOK Manager) was included on the test disc but not exercised.
- **No mechanism for delivering the Server TLS fingerprint through a verified boot chain to the Agent was designed** — see "Trust-bootstrap implications" above.
- **Revocation and update handling** (dbx updates, shim/GRUB CVE response) was not evaluated.
- Physical Integration Environment validation (real UEFI firmware's Secure Boot implementation, real OEM trust store contents) remains required future work before any production decision.

## Related work

- Issue #10 — `[Spike] Validate Secure Boot and hardened boot chain` (this Spike).
- `docs/reference/winpe-boot-mechanism-spike.md` — Issue #8's boot-mechanism evidence and tooling this Spike reused.
- `docs/specifications/m0-agent-protocol-contract.md` — the Server-fingerprint delivery mechanism this Spike's trust-bootstrap implications inform, not resolve; ADR-0005's WSS transport decision is explicitly not reopened here.
- `docs/specifications/m0-endpoint-identity-lifecycle.md` — endpoint trust model this evidence may eventually inform.
- `docs/specifications/m0-stack-and-boundaries-baseline.md` "Boot-orchestration architectural boundary" — records the Boot Port/Adapter boundary this evidence remains subordinate to; no amendment is made by this document (consistent with the pattern used for Issue #8 and Issue #11 — any amendment requires separate, explicit owner authorization).
