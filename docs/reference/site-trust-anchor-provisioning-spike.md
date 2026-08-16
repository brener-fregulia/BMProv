# Site Trust-Anchor Provisioning — Local Virtualized Evidence

## Question

Determine whether Bamep can establish a legitimate per-site trust anchor on a
previously unprepared UEFI x86-64 Endpoint with an operational cost compatible with
automated bare-metal provisioning (Issue #14, `[Spike] Validate site trust-anchor
provisioning`) — the practical question left open by Issue #13 / sub-problem (B) of
`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`: how does
a new Endpoint learn a public key that legitimately belongs to a specific Bamep
installation, before trusting nonce-bound signed bootstrap assertions signed by that
site?

Two pre-established-trust candidates were in scope:

- **Candidate A — shim/MOK enrollment.**
- **Candidate B — direct UEFI `db`/PK enrollment.**

Operator-approval-gated first-key trust (the already-recorded architectural fallback)
was explicitly out of scope for empirical testing in this round.

## Why existing evidence was insufficient

`docs/reference/secure-boot-hardened-chain-spike.md` (Issue #10) proved Secure Boot
enforcement and a Microsoft-trusted-shim → Canonical-signed-GRUB chain work, but
explicitly did **not** exercise MOK enrollment — `mmx64.efi` (shim's MOK Manager) was
present on the test disc but never triggered. No prior evidence characterized the real
MOK enrollment lifecycle, its interaction/reboot cost, or any direct `db`/PK
enrollment path.

## Constraints and assumptions

- UEFI x86-64 is the V1 target; Legacy BIOS is out of scope.
- Reuses the VirtualBox VM, artifacts, and method already established by Issue #8 and
  Issue #10 (`docs/reference/winpe-boot-mechanism-spike.md`,
  `docs/reference/secure-boot-hardened-chain-spike.md`), per the same owner
  authorization pattern for a local virtualized approximation.
- Does not reopen ADR-0010, the accepted nonce-bound signed bootstrap assertion (C),
  or the accepted `BootstrapEvidence` mechanism (D).
- Does not select or accept a trust-anchor architecture. That remains an owner
  decision informed by, not made by, this Spike.

## Environment scoping decision

Reused the existing VM `BamepSpike-WinPE-UEFI` (VirtualBox **7.2.14r174565**, firmware
`EFI64`), with VirtualBox's own representative Microsoft-trusting Secure Boot default
configuration (`enrollmssignatures` + `enrollorclpk`, `SecureBoot=on`), exactly as
characterized in Issue #10 — **virtualized-firmware evidence, not physical
Integration-Environment evidence.** A snapshot (`pre-trust-anchor-spike`) was taken
before any state-changing experiment and used to keep Candidate A and Candidate B
independent; the VM was restored to that snapshot at the end of this round.

**Linux tool environment:** WSL2 Ubuntu 24.04.1 LTS on the Windows 11 host, used only
for key generation, signing, and boot-media construction — WSL2 cannot itself reach
the VirtualBox VM's UEFI NVRAM (`mokutil --sb-state` inside WSL2 returns "EFI
variables are not supported on this system"), consistent with WSL2 being a separate
Hyper-V-based virtualization layer, not connected to the VM's firmware. All operations
against the VM's own NVRAM were performed by a disposable Linux environment booted
**inside** the VM itself (see Method).

**Reused, unmodified, from Issue #10:** `shim-signed 1.58+15.8-0ubuntu1`
(`shimx64.efi.signed`, Microsoft-signed) and `grub-efi-amd64-signed
1.202.5+2.12-1ubuntu7.3` (`grubx64.efi.signed`, Canonical-signed), including shim's own
`mmx64.efi` MOK Manager — the same Scenario-3 chain that already proved it boots
cleanly under Secure Boot.

## Method — building a disposable per-boot Linux environment inside the VM

Neither `mokutil` nor `efitools` can run against the VM's real firmware NVRAM from
outside the VM. A minimal, disposable Linux environment was built to run **inside**
the VM's own boot chain, using tooling already available after Issue #8/#10
(`xorriso`, `mtools`, `grub-mkstandalone`):

1. Installed `linux-image-generic 6.8.0-137.137` (Canonical-signed kernel,
   `vmlinuz-6.8.0-137-generic`) in WSL2, matching the same signed-package family
   already used for shim/GRUB.
2. Built a custom initramfs via `update-initramfs -c`, using an
   `/etc/initramfs-tools/hooks/` hook (`copy_exec`) to embed `mokutil 0.6.0-2build3`
   (and, for the Candidate B round, `efitools 1.9.2-3ubuntu3`'s `cert-to-efi-sig-list`
   / `efi-updatevar`) plus their shared-library dependencies
   (`libcrypto.so.3`, `libefivar.so.1`, `libkeyutils.so.1`) and the test certificate.
3. Assembled a bootable ISO reusing the exact proven `BOOTX64.EFI`/`grubx64.efi`/
   `mmx64.efi` binaries from Issue #10's Scenario 3, adding a `grub.cfg`
   (`linux /boot/vmlinuz break=premount` + `initrd /boot/initrd.img`) and the GRUB
   `x86_64-efi` module tree (`grub-efi-amd64-bin 2.12-1ubuntu7.3` — the exact unsigned
   module-set counterpart of the signed GRUB build) at a path GRUB could load once its
   embedded default prefix (`(cd0)/EFI/ubuntu`, absent on this disc) was manually
   overridden at the rescue prompt (`set root=…`, `set prefix=…/boot/grub`,
   `insmod normal`, `normal`) — a boot-media-construction detail, not a
   trust-relevant finding.
4. `break=premount` reliably dropped to a BusyBox `v1.36.1` `(initramfs)` shell with no
   real root filesystem — sufficient to mount `efivarfs` and run `mokutil`/`efitools`
   directly against the VM's live NVRAM.

Test keypair: RSA 2048, self-signed, `CN=Bamep Site Test Trust Anchor`, generated with
`openssl req -x509 -new -newkey rsa:2048 … -nodes -days 3650` in WSL2.
SHA-256 of `MOK.der`: `c70613324734b47cf47b5d32625a57f30c3c53feecf20aabd8a1d85f6e766f62`.

## Candidate A — shim/MOK enrollment

### Enrollment procedure (observed exactly)

1. From the booted shell, with `efivarfs` mounted at `/sys/firmware/efi/efivars`:
   `mokutil --import /root/MOK.der` → interactively prompts for and confirms a
   password (typed twice) → stages a pending request, confirmed present via
   `mokutil --list-new` (showed the full certificate).
2. **Reboot required.** On the *next* boot, shim detects the pending request and
   interrupts the normal chain with a distinct screen: `Shim UEFI key management` /
   `Press any key to perform MOK management`, with a short (~6 second) countdown.
   **Missing this window is silent and consumes the pending request** — two of our
   own attempts in this round were too slow (screenshot/keystroke round-trip latency)
   and the pending enrollment request was dropped without any error, requiring
   `mokutil --import` to be redone from scratch. This is a real, narrow,
   keyboard-interaction requirement, not a "confirm at your leisure" prompt.
3. Interactive text-mode `MokManager` menu, navigated with arrow keys + Enter:
   `Perform MOK management` → **Enroll MOK** → `[Enroll MOK]` (`View key 0` /
   `Continue`) → **Continue** → `Enroll the key(s)? No / Yes` → **Yes** →
   `Password:` (the same password set in step 1) → returns to `Perform MOK
   management`, now offering only `Reboot` / `Enroll key from disk` / `Enroll hash
   from disk` (`Continue boot` no longer offered, confirming the action was
   consumed) → **Reboot**.
4. After this second reboot, `mokutil --list-enrolled` (run in a fresh, independent
   boot of the same disposable environment) showed `Bamep Site Test Trust Anchor` as
   enrolled — persistence confirmed across a full reboot cycle.

**Total reboot cost per enrollment: exactly 2** (one to trigger the MokManager
ceremony, one to finalize after confirmation), plus one mandatory interactive
keyboard ceremony (≈5 keypresses/typed password) that must be caught within the short
firmware-driven window.

### Functional verification: does shim now trust the key?

A minimal test EFI binary was built with `grub-mkstandalone` and signed with the
enrolled test key (`sbsign`), then placed as shim's expected second stage
(`\EFI\Boot\grubx64.efi`, replacing the Canonical-signed GRUB, `\EFI\Boot\BOOTX64.EFI`
= the same unmodified shim).

- **First attempt failed**: `ERROR — Verification failed: (0x1A) Security Violation`.
  This is **not** a MOK-trust rejection — comparing our self-built binary against the
  official `grubx64.efi.signed` showed the official binary carries a populated
  `.sbat` section (`sbat,1,SBAT Version,…` / `grub,5,…` / `grub.ubuntu,2,…`) that our
  self-built binary lacked entirely. Shim 15.8 enforces SBAT-generation/revocation
  policy on every loaded image **independently of MOK trust**. Rebuilding the test
  binary with `grub-mkstandalone --sbat=<minimal-sbat-file>` (properly placed by
  GRUB's own build tooling, not manual PE-section splicing — a first manual
  `objcopy --add-section` attempt produced a structurally invalid section and a
  *different* error, `(0x3) Unsupported`) and re-signing resolved this.
- **With a valid `.sbat` section**: the MOK-signed binary was accepted and executed
  cleanly (reached a `grub>` prompt, no `Access Denied`, no `Security Violation`) —
  confirming shim now trusts an executable signed by the site-enrolled test key.

**This is a concrete, previously-unrecorded operational requirement**: a real Bamep
boot component chain-loaded via this mechanism needs a valid SBAT section in addition
to a MOK-enrolled signing key — MOK enrollment alone is not sufficient for shim
15.x to accept an otherwise-legitimately-signed binary.

### Revocation

`mokutil --delete /root/MOK.der` (same password ceremony) → reboot → MokManager
offered **Delete MOK** (in place of Enroll MOK) → the identical
Continue → Yes → password → Reboot flow → after the second reboot, the *same*
previously-accepted SBAT-compliant, MOK-signed binary was rejected again with the
identical `(0x1A) Security Violation`, and the key no longer appeared in
`mokutil --list-enrolled`. **Full enroll/verify-accept/revoke/verify-reject lifecycle
confirmed symmetric**, at the same 2-reboots-plus-one-ceremony cost as enrollment.

### What this establishes about firmware/NVRAM vs. OS tie

The disposable environment used for every step here has **no persistent OS or disk at
all** (an `initramfs`-only boot with no root filesystem). Enrollment and revocation
state nonetheless persisted correctly across full VM `poweroff`/`startvm` power
cycles, not just warm reboots — confirming the MOK state lives in the machine's own
UEFI NVRAM (`MokListRT`/`MokNew`/`MokDel` variables), independent of any installed
OS or disk image, directly answering Issue #14's question on this point.

### Automatability

Every step **except the MokManager confirmation UI** was fully scripted in this round
(key generation, signing, boot-media construction, `mokutil --import`/`--delete`,
reboot) using VirtualBox's remote console API (`VBoxManage controlvm … screenshotpng
/ keyboardputstring / keyboardputscancode`). The MokManager confirmation itself
**cannot** be driven by any in-OS/remote-API call — it strictly requires synthetic
keyboard input delivered to the console framebuffer at boot time, within a narrow
window, before any OS is running. On real hardware, the equivalent requires either
genuine physical presence at each Endpoint, or an out-of-band remote-console
capability (KVM-over-IP / BMC-class keyboard injection) scripted analogously to what
this Spike used against VirtualBox's own console API — not a given on arbitrary
customer bare-metal hardware.

## Candidate B — direct UEFI `db`/PK enrollment

This round completed Candidate B to the same evidentiary rigor as Candidate A, via
`KeyTool.efi` (Preferred path A: an efitools EFI application, no Linux userspace
involved) — superseding the previous round's blocked `efi-updatevar`/BusyBox attempt,
preserved below only as resolved context. That earlier finding stands as recorded:
`efi-updatevar` failed on `mount: invalid option -- 'l'` because BusyBox's minimal
`mount` lacks GNU `mount -l`, which `efi-updatevar` shells out to internally — an
environment-tooling dependency, not a demonstrated mechanism failure. No further time
was spent patching BusyBox/initramfs internals to rescue that path, per instruction.

### Method: preserving Microsoft trust while adding a site key

1. Before resetting NVRAM, dumped the VM's existing Microsoft/Oracle `db`, `KEK`,
   `PK` content directly from the live variable store
   (`VBoxManage modifynvram <vm> queryvar --name db|KEK|PK --filename …`):
   `ms-db.bin` (7636 bytes, 5 Microsoft signature entries), `ms-kek.bin` (3066 bytes),
   `ms-pk.bin` (1035 bytes).
2. In WSL2: built an EFI Signature List for the same "Bamep Site Test Trust Anchor"
   test key reused from Candidate A (`cert-to-efi-sig-list -g <GUID> MOK.pem
   bamep-own.esl`, 863 bytes).
3. Concatenated (`EFI_SIGNATURE_LIST` is self-delimiting, so lists concatenate
   safely): `db-combined.esl` = `ms-db.bin` + `bamep-own.esl` (8499 bytes);
   `kek-combined.esl` = `ms-kek.bin` + `bamep-own.esl` (3929 bytes). `PK` cannot be
   combined — UEFI defines `PK` as single-valued — so `pk-own.esl` = `bamep-own.esl`
   alone, a **necessary replacement, recorded here explicitly and not a claim about
   any desired production configuration.**
4. Self-signed all three as authenticated update files:
   `sign-efi-sig-list -c MOK.pem -k MOK.priv <VarName> <in.esl> <out.auth>` →
   `db.auth` (9765 B), `KEK.auth` (5195 B), `PK.auth` (2129 B).
5. Reset the VM's NVRAM to Setup Mode (`VBoxManage modifynvram <vm>
   inituefivarstore`; `SecureBoot` reported `off` immediately).
6. Built a boot disc with the **stock, unsigned** `KeyTool.efi` (from `efitools
   1.9.2-3ubuntu3`) as `\EFI\Boot\BOOTX64.EFI` plus the three `.auth` files, and
   booted it directly — no shim, no MOK, no Linux userspace anywhere in this path.

### Enrollment ceremony (observed exactly)

`KeyTool.efi` booted immediately — unsigned execution is permitted while in Setup
Mode — and its own main menu self-reported `Platform is in Setup Mode` / `Secure Boot
is off`. From `Edit Keys` → `Select Key to Manipulate`:

1. **KEK** → `Add New Key` → browse to the CD-ROM device → `KEK.auth` → applied
   silently, no error, no reboot.
2. **db** → `Add New Key` → same file browser → `db.auth` → applied silently, no
   reboot.
3. **PK** → only `Replace Key(s)` is offered (`PK` is single-valued; KeyTool itself
   shows `WARNING: Setting PK will take the platform out of Setup Mode`) → browse →
   `PK.auth` → applied silently.
4. Backing out to the KeyTool main menu — **no reboot anywhere in this sequence** —
   immediately reported `Platform is in User Mode` / `Secure Boot is on`.

**Zero reboots were required for the entire `db`+`KEK`+`PK` enrollment.** This is a
material difference from Candidate A's fixed 2-reboot cost. The whole ceremony was:
boot once, then ~3 repeated menu actions (select variable → Add/Replace → browse
device → browse directory → select file), each applied live.

### Functional verification: does firmware now trust the key directly?

- A minimal EFI binary signed with the site test key (Candidate A's SBAT-compliant
  `test-signed-v2.efi`, reused unmodified) was placed directly as
  `\EFI\Boot\BOOTX64.EFI` — **no shim involved at all**. It booted cleanly (reached
  `grub>`, no rejection), confirming firmware itself — not merely shim — now trusts
  the site key via `db`. This path is structurally simpler than Candidate A's: SBAT
  is a shim-specific policy layer, and firmware's own Secure Boot check does not
  enforce it.
- **Preserved Microsoft-trusting capability, confirmed functionally**: with the
  combined `db`/`KEK`/`PK` active, the original, completely unmodified Issue #10
  Scenario-3 disc (Microsoft-signed shim → Canonical-signed GRUB, no MOK involved)
  was booted again and **still succeeded cleanly**. Adding a site key via this path
  did not break the already-proven Microsoft-trusting boot path — direct evidence
  for Issue #14 §2's "preserve where practical" instruction.

### Revocation

Once `PK` is set (User Mode), further `db`/`KEK` changes require an **authenticated**
update — unauthenticated writes stop being accepted. A second, unprompted finding
surfaced here: **the stock, unsigned `KeyTool.efi` was itself rejected by firmware**
once `PK` was set (`Access Denied`, the same firmware-level rejection observed in
Issue #10's Scenario 2) — once enforcement is active, even the management tool must
itself be trusted (signed by a `db`-trusted key) to run again. Signing `KeyTool.efi`
with the same site test key (`sbsign` — valid because that key is already in `db`)
restored the ability to run it.

Using the signed `KeyTool.efi`:

1. Built an authenticated `db` update reverting `db` to the original 5 Microsoft
   entries only (`sign-efi-sig-list -c MOK.pem -k MOK.priv db ms-db.bin
   db-revoke.auth`) — signed by the site key, valid because that key is a member of
   `KEK`.
2. `Edit Keys` → `db` (the live list correctly displayed all 5 Microsoft entries plus
   the 1 site-key entry) → `Replace Key(s)` → browsed to `db-revoke.auth` → applied
   silently, no reboot. Re-entering `db` confirmed only the 5 Microsoft entries
   remained.
3. Rebooted into the site-key-signed test binary directly as `BOOTX64.EFI` (no
   shim) — **rejected again** (`Access Denied`, firmware-level — identical signature
   to the pre-enrollment baseline).
4. Rebooted into the unmodified Issue #10 Scenario-3 shim+GRUB disc — **still
   succeeded**, confirming the Microsoft/Canonical entries were untouched by the
   site-key revocation.

**Full enroll → verify-accept → preserve-Microsoft-trust → revoke → verify-reject
lifecycle confirmed, at a cost of zero reboots for enrollment and zero reboots for
revocation** — each a single boot session with several interactive menu selections,
no boot cycling required between `db`/`KEK`/`PK` writes.

### Firmware-state prerequisites (recorded exactly, per Issue #14 §3)

- **Setup Mode is required** to write `db`/`KEK`/`PK` unauthenticated for the first
  time. In this lab, Setup Mode was reached via `VBoxManage modifynvram
  inituefivarstore` — a **host-side, out-of-band action with no physical-Endpoint
  equivalent**, and per Issue #14 §3 this does **not** count as evidence of an
  automatable physical workflow. What Setup Mode corresponds to on real OEM
  firmware — factory-default state, or an interactive firmware "Clear Secure Boot
  Keys"/Custom Mode menu action — was **not tested**. This is the single largest
  open question for physical portability of this candidate.
- **No reboot is required before the variable writes** — `KeyTool.efi` operates live
  once already running with Setup Mode active.
- **No reboot is required between or after the `db`/`KEK` writes** while still in
  Setup Mode.
- **Setting `PK` transitions Setup Mode → User Mode immediately and live, with no
  reboot** — Secure Boot enforcement activates instantly, observed directly on
  KeyTool's own status line.
- **After User Mode is active, further `db`/`KEK` changes require an authenticated
  (signed) update file** — unlike Candidate A, where `mokutil`'s request-staging step
  never requires any signing at all.
- **After User Mode is active, the management tool itself must be signed by a
  trusted key to keep running** — the stock unsigned `KeyTool.efi` build stopped
  working the moment enforcement activated.

**What the tool automates vs. what must happen first:** `KeyTool.efi` automates
applying an already-prepared, correctly-formatted `.esl`/`.auth` file to a variable,
live, with no reboot. It does **not** automate, and cannot itself provide, reaching
Setup Mode in the first place — that precondition sits entirely outside the tool,
and on physical hardware is understood (not evidenced here) to require either a
factory-fresh machine or an interactive firmware action.

### Not attempted this round

`UpdateVars.efi` — the non-interactive companion EFI application (applies
`PK.auth`/`KEK.auth`/`db.auth` automatically, by filename convention, with no menu
navigation) was not exercised. Its documented invocation takes command-line
arguments (`UpdateVars <VarName> <file>`), which needs a UEFI Shell environment (not
built in this round) rather than a plain chainloaded `BOOTX64.EFI`. `KeyTool.efi`'s
interactive path was used instead as the simplest, least-artificial option available,
per Issue #14 §1, and required no UEFI Shell. `UpdateVars.efi` remains a concrete,
well-scoped next step if a fully non-interactive Candidate B ceremony needs to be
evidenced — Issue #14 §6 stops experimentation here regardless.

## Operational evaluation

Both candidates were completed to comparable evidentiary rigor this round.

### Candidate A (shim/MOK)

- **Per-Endpoint cost is real and does not amortize across a fleet.** Staging the
  request (`mokutil --import`) must run *on* the target Endpoint itself against its
  own firmware — there is no centralized/remote way to pre-load a pending MOK request
  from the Bamep Server. Each Endpoint separately requires 2 reboots plus one
  interactive confirmation ceremony.
- **At 3–5 Endpoints**: survivable as a manual or KVM-scripted per-machine bring-up
  step.
- **At the 20–24 Endpoint target**: the identical per-Endpoint interactive ceremony
  repeats 20–24 times for initial bring-up, and again for every future rotation or
  recovery event — it does not become cheaper at scale.
- **Recovery/rotation**: MOK state is tied to Endpoint firmware NVRAM, not the OS —
  it survives OS/boot-media reprovisioning, but is lost by any firmware-level
  "reset Secure Boot keys" action (available on essentially all real UEFI
  firmware) and by full NVRAM-store resets. Losing or rotating the site key requires
  re-running the full per-Endpoint interactive ceremony again on every affected
  Endpoint.

### Candidate B (direct `db`/PK)

- **Per-Endpoint cost is also real and does not amortize**, for a different reason:
  every Endpoint still needs its own boot-time interactive ceremony (there is no
  remote/centralized way to write another machine's firmware NVRAM), but the
  ceremony itself is cheaper once underway — zero reboots for the full `db`+`KEK`+`PK`
  enrollment, versus Candidate A's fixed 2 reboots.
- **The Setup Mode precondition dominates the real cost and was not resolved by this
  Spike.** Reaching Setup Mode was done here via a host-side, VirtualBox-only
  command with no physical-Endpoint equivalent. Whether an arbitrary previously
  unprepared OEM Endpoint can reach Setup Mode without interactive firmware-menu
  access remains unknown — this is the decisive open question, not the enrollment
  mechanics themselves.
- **At 3–5 or 20–24 Endpoints**: the same per-Endpoint, non-amortizing shape as
  Candidate A applies to *reaching Setup Mode plus running the ceremony*; only the
  ceremony portion is cheaper (no reboot cycling).
- **Recovery/rotation**: `db`/`KEK` state is Endpoint-firmware-NVRAM-resident, same
  tie as MOK. Once `PK` is owned by the site key, *routine* `db`/`KEK` rotation no
  longer needs Setup Mode at all — an authenticated update signed by the existing
  `KEK` suffices (demonstrated directly: the revocation step was a live, no-reboot,
  authenticated `db` replace). This is a genuine operational advantage over Candidate
  A once initial enrollment is complete: post-enrollment key lifecycle management
  does not require re-entering Setup Mode or physical/console access again, only a
  correctly-signed update file transported to the machine.
- **A previously-unrecorded cost specific to this candidate**: once Secure Boot
  enforcement is active, the management tool itself must be signed by a trusted key
  to keep running — an unsigned `KeyTool.efi`/`UpdateVars.efi` stops working the
  moment `PK` is set, so ongoing management requires maintaining a signed copy of
  whatever tool performs future updates.

### Distinguishing the four framings Issue #14 requires (both candidates)

|                                                              | Candidate A (MOK)        | Candidate B (direct `db`/PK) |
| ------------------------------------------------------------ | ------------------------- | ------------------------------ |
| Technically possible                                          | **Yes** — full enroll/accept/revoke/reject cycle demonstrated | **Yes** — full enroll/accept/preserve-MS-trust/revoke/reject cycle demonstrated |
| Automatable *after* required firmware state is reached        | Partially — everything but the MokManager confirmation screen | Everything — `KeyTool.efi` ceremony itself needs no reboot, and `UpdateVars.efi` (untested) is plausibly non-interactive once Setup Mode is reached |
| Unattended from a previously unprepared Endpoint               | **No** — not demonstrated; MOK enrollment interaction is unavoidable in shim 15.8 | **Not established** — reaching the required Setup Mode precondition was not tested on anything but a VirtualBox host-side shortcut; whether an arbitrary unprepared OEM Endpoint can reach Setup Mode unattended is unknown |
| Operationally acceptable for 3–5 Endpoints                    | Owner judgment; survivable manual/KVM-scripted cost | Owner judgment; same per-Endpoint shape, cheaper ceremony |
| Operationally acceptable for 20–24 Endpoints                  | Owner judgment; identical repeated interactive cost, does not amortize | Owner judgment; identical repeated Setup-Mode-reach cost, does not amortize; post-enrollment rotation is cheaper than Candidate A |

**Directly answering Issue #14's central question — can Bamep take an arbitrary
previously-unprepared OEM UEFI Endpoint and establish this site key without
per-machine console/firmware intervention?** — **the evidence cannot establish that**,
for either candidate. Both require some form of per-Endpoint interactive access (a
console/keyboard ceremony for MOK; reaching Setup Mode, by an unverified means, for
direct `db`/PK). Candidate B's *ceremony* is measurably cheaper and its
*post-enrollment* rotation/revocation story is materially better than Candidate A's,
but this Spike did not — and, per Issue #14 §6, will not — determine whether an
arbitrary unprepared OEM Endpoint can reach the Setup Mode precondition without
equivalent console/firmware intervention. That gap is isolated below as required
future Integration Environment validation, consistent with the already-recorded
operational-cost concern in
`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
("(B) Site trust-anchor provisioning").

## Conclusion

Both Candidate A (shim/MOK) and Candidate B (direct UEFI `db`/PK) are **technically
viable and were fully validated end-to-end** in this virtualized environment:
enrollment, functional trust verification, persistence, and clean revocation all
behaved correctly and reproducibly for both. Neither is **unattended-automatable**
from a previously unprepared Endpoint in the sense Bamep's target bare-metal
provisioning model would want:

- Candidate A requires a genuine interactive keyboard ceremony per Endpoint,
  catchable only via physical presence or an equivalent out-of-band remote-console
  capability, at a fixed 2-reboot cost that repeats identically at 3–5 or 20–24
  Endpoints and again on every future key rotation/recovery event.
- Candidate B requires reaching a Setup Mode firmware precondition whose
  physical-Endpoint accessibility this Spike could not evidence (only a
  VirtualBox-only host-side shortcut was available), but once that precondition is
  met, the enrollment/revocation ceremony itself is cheaper (zero reboots) and,
  materially, **post-enrollment key rotation/revocation no longer needs Setup Mode
  at all** — an authenticated update signed by the already-owned `KEK` suffices,
  live, with no reboot. This is a real operational advantage over Candidate A for
  the ongoing-management portion of the lifecycle, independent of the still-open
  initial-access question.

Per Issue #14's own evaluation criteria, this Spike does not need to find an
acceptable candidate, and per §6 this is the final experimental round. The evidence
supports — without this Spike making the decision itself — the following framing:
**both tested pre-established-trust mechanisms are technically sound, and both
carry a real, non-amortizing per-Endpoint operational cost at initial-enrollment
time; Candidate B is materially less interactive during enrollment and
meaningfully better for ongoing key-lifecycle management after initial
enrollment, but this Spike could not establish whether either candidate's
initial per-Endpoint interactive requirement can be eliminated on arbitrary,
previously-unprepared OEM hardware.** Whether either cost is acceptable for V1
remains an owner judgment, informed by but not resolved by this Spike.

## Remaining uncertainty

- **This is virtualized-firmware evidence** (VirtualBox's representative
  Microsoft-trusting default configuration), **not physical Integration-Environment
  evidence** — real OEM firmware's MokManager and Setup Mode implementations, exact
  keyboard/timing behavior, and SBAT/revocation policy version may differ. Physical
  validation remains required before any production conclusion, consistent with
  `docs/development/testing.md`. This is now isolated as the single concrete item
  for future Integration Environment validation: **whether an arbitrary,
  previously-unprepared OEM Endpoint can reach UEFI Setup Mode (for Candidate B) or
  complete the MokManager ceremony (for Candidate A) without physical presence or an
  equivalent out-of-band remote-console/BMC capability.**
- **`UpdateVars.efi` (Candidate B's non-interactive companion tool) was not
  exercised** — it requires a UEFI Shell environment not built in this round;
  `KeyTool.efi`'s interactive path was used instead and fully answered the
  mechanism-level questions this Spike required.
- **No labor-time estimate is offered** — only measured interaction/reboot counts,
  per Issue #14's explicit instruction not to invent time estimates without
  measured evidence.
- **MokManager's exact countdown/window duration** was not measured (only observed
  to be short enough that two of our own scripted attempts missed it) — a precise
  timing measurement was not pursued as it does not change the qualitative
  automation-ceiling finding.
- **Revocation/recovery mechanics for a *lost* (not merely rotated) site key** were
  not evaluated for either candidate — only the case where the original key
  material remains available to authorize its own replacement/deletion.
- Per Issue #14 §6, this Spike does not extend to other hypervisors, other firmware
  implementations, TPM/measured boot, Microsoft signing arrangements, vendor
  enterprise tooling, or physical hardware. The physical-firmware portability
  boundary noted above is the one item explicitly carried forward, not resolved
  here.

## Related work

- Issue #14 — `[Spike] Validate site trust-anchor provisioning` (this Spike).
- Issue #13 — `[WP] Define trusted bootstrap and Server fingerprint delivery
  contract`; `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
  "(B) Site trust-anchor provisioning" and "Technical Spike recommendation" — the
  open question this Spike gathers evidence for, not resolves.
- Issue #10 / ADR-0010 — `docs/reference/secure-boot-hardened-chain-spike.md` — the
  Secure Boot baseline and shim/GRUB artifacts this Spike reused unmodified.
- Issue #8 — `docs/reference/winpe-boot-mechanism-spike.md` — VM/tooling this Spike's
  method continues to build on.
