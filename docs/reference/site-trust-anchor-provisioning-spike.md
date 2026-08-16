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

**Evidence for this candidate is incomplete** — the mechanism's ESL-construction and
host-level NVRAM-reset steps were demonstrated; the actual variable-write step via
guest-side standard tooling was not completed in this round. Reported honestly as a
partial result per Issue #14's explicit allowance for this.

### What was established

- `VBoxManage modifynvram <vm> inituefivarstore` resets the VM's UEFI variable store
  to a blank state (`SecureBoot` reports `off` immediately afterward) — the
  virtualized analogue of a real firmware's "Clear Secure Boot keys" / enter Custom
  Setup Mode action, which on physical hardware requires interactive firmware
  Setup-menu access.
- `VBoxManage modifynvram <vm> enrollpk [--platform-key=FILE]` and `enrollmok
  [--mok=FILE]` exist as **host-side, out-of-band, unauthenticated** NVRAM-injection
  commands. These are a genuine convenience for lab/CI automation of *this specific
  hypervisor* but have **no physical-hardware equivalent** — no OEM firmware exposes
  an external host tool that can write Secure Boot variables from outside the
  machine. Flagged explicitly as a virtualization-only artifact, not evidence
  transferable to physical Endpoints.
- `efitools 1.9.2-3ubuntu3` (`cert-to-efi-sig-list`, `sign-efi-sig-list`,
  `efi-updatevar`, and the interactive `KeyTool.efi` family) installs cleanly and
  provides the standard guest-side tooling path. `cert-to-efi-sig-list` requires a
  **PEM**-encoded certificate — an initial attempt using the DER-encoded `MOK.der`
  silently produced a near-empty (44-byte) signature list with no cert data; using
  the already-available `MOK.pem` produced a correctly-sized (863-byte) EFI
  Signature List.

### What blocked completion

`efi-updatevar -f bamep.esl db` (and `KEK`) consistently failed with `mount: invalid
option -- 'l'` inside the disposable `initramfs`-only environment. This traces to
`efi-updatevar` internally invoking `mount -l` (a GNU/util-linux-specific option) to
introspect mount state — an option BusyBox's minimal `mount` applet (the only `mount`
available in this environment) does not implement. An attempted compatibility shim
(a wrapper script placed ahead of BusyBox's `mount` on `PATH`, rebuilt into the
initramfs) did not resolve the failure within this round's time budget, most likely
because `initramfs-tools`' own core scripts re-establish BusyBox's `mount` applet
after custom hooks run. This is an **environment-tooling dependency finding, not a
demonstrated mechanism failure**: `efi-updatevar` (unlike `mokutil`, which has no such
dependency) assumes a more complete userspace than a minimal disposable boot
environment provides.

### Not attempted this round

`KeyTool.efi` — the interactive, pre-boot EFI application shipped by `efitools` that
performs the same `db`/KEK/PK enrollment without depending on any Linux userspace
`mount` at all. This is arguably the more direct real-firmware equivalent of
"standard EFI tooling" per Issue #14's framing (closer to what an integrator would
actually use, and structurally similar to shim's own `mmx64.efi` MOK Manager
ceremony already characterized under Candidate A). It remains a concrete, well-scoped
next step if Candidate B needs to be revisited with further evidence.

## Operational evaluation

Based on Candidate A, the only candidate completed to the same rigor as Issue #10:

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
- **Distinguishing the four framings Issue #14 requires:**
  - *Technically possible*: **yes** — demonstrated end-to-end, including functional
    accept/reject verification and clean revocation.
  - *Automatable*: **partially** — every step except the MokManager confirmation
    screen is scriptable; the confirmation screen itself requires equivalent
    out-of-band keyboard/console-injection capability per Endpoint.
  - *Unattended (zero interactive console touch per Endpoint)*: **no** — not
    demonstrated; the evidence shows an unavoidable interactive step in the
    mechanism as it exists in shim 15.8.
  - *Operationally acceptable at Bamep's V1 target scale*: **not established by this
    Spike** — that remains an owner judgment. The evidence gathered here is
    consistent with the operational-cost concern already recorded (but not
    evidenced) in `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
    ("(B) Site trust-anchor provisioning").

## Conclusion

Candidate A (shim/MOK enrollment) is **technically viable and was fully validated
end-to-end** in this virtualized environment: enrollment, functional trust
verification (including the previously-unrecorded SBAT-compliance requirement),
persistence across reboot and power cycles, and clean revocation all behaved
correctly and reproducibly. It is **not unattended-automatable** in the sense
Bamep's target bare-metal provisioning model would want — it requires a genuine
interactive keyboard ceremony per Endpoint, catchable only via physical presence or
an equivalent out-of-band remote-console capability, at a fixed 2-reboot cost that
repeats identically at 3–5 or 20–24 Endpoints and again on every future key
rotation/recovery event.

Candidate B (direct `db`/PK enrollment) evidence is **incomplete**: the host-side
NVRAM-reset and guest-side ESL-construction steps were demonstrated, but the actual
`db`/KEK variable write via standard guest-side tooling (`efi-updatevar`) was blocked
by an environment-specific tooling dependency (BusyBox `mount` lacking `-l` support)
in the disposable Linux environment used, not by a demonstrated mechanism failure.
`KeyTool.efi` remains an untested, more directly comparable alternative path.

Per Issue #14's own evaluation criteria, this Spike does not need to find an
acceptable candidate. The evidence gathered for Candidate A supports — without this
Spike making the decision itself — the framing that **a pre-established per-Endpoint
trust-anchor mechanism requiring interactive console access is technically sound but
carries a real, non-amortizing per-Endpoint operational cost** at Bamep's target
scale. Whether that cost is acceptable for V1, and whether Candidate B (or
`KeyTool.efi` specifically) changes this picture, remain open questions for owner
review, informed by but not resolved by this Spike.

## Remaining uncertainty

- **This is virtualized-firmware evidence** (VirtualBox's representative
  Microsoft-trusting default configuration), **not physical Integration-Environment
  evidence** — real OEM firmware's MokManager implementation, exact keyboard/timing
  behavior, and SBAT/revocation policy version may differ. Physical validation
  remains required before any production conclusion, consistent with
  `docs/development/testing.md`.
- **Candidate B's core mechanism (does firmware actually accept and enforce a
  directly db-enrolled key at boot) was not empirically confirmed or refuted** in
  this round — only its ESL-construction and NVRAM-reset preconditions were.
  `KeyTool.efi` was not attempted.
- **No labor-time estimate is offered** — only measured interaction/reboot counts,
  per Issue #14's explicit instruction not to invent time estimates without
  measured evidence.
- **MokManager's exact countdown/window duration** was not measured (only observed
  to be short enough that two of our own scripted attempts missed it) — a precise
  timing measurement was not pursued as it does not change the qualitative
  automation-ceiling finding.
- **Revocation/recovery mechanics for a *lost* (not merely rotated) site key** were
  not evaluated — only the interactive `mokutil --delete` path with the original key
  still available.

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
