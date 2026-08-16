# ADR-0011: V1 site trust-anchor establishment and operator-verified first-key pairing

Status: Accepted

## Context

`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` (Issue
#13) accepted sub-problems (A) Secure Boot executable-chain trust (ADR-0010), (C) the
nonce-bound signed bootstrap assertion, and (D) authenticated Agent `BootstrapEvidence`
reporting, but left sub-problem **(B) site trust-anchor provisioning** as the sole
remaining blocker to owner approval: how does a previously unprepared, arbitrary
Endpoint legitimately learn the public key belonging to a specific Bamep installation,
before it can verify a nonce-bound signed bootstrap assertion (C) from that site?

Issue #14 (`[Spike] Validate site trust-anchor provisioning`) closed that evidence gap
empirically, in a local virtualized UEFI environment (VirtualBox, reusing the Issue
#10 Secure Boot baseline), for the two pre-established-trust candidates the
Specification had recorded but not selected between:

- **Candidate A — shim/MOK enrollment.** Validated end-to-end: enrollment,
  persistence across reboot/power-cycle, functional trust verification (including a
  previously-unrecorded requirement that a chain-loaded binary also carry a valid
  SBAT section, independent of MOK trust), and clean revocation. Measured cost: a
  mandatory interactive MokManager keyboard ceremony per Endpoint, catchable only via
  physical presence or an equivalent out-of-band remote-console capability, at a
  fixed 2-reboot cost per enrollment or revocation operation, repeating identically
  regardless of fleet size and again on every future key rotation/recovery event.
- **Candidate B — direct UEFI `db`/PK enrollment.** Also validated end-to-end via
  `KeyTool.efi`: enrollment, functional trust verification, preservation of the
  already-proven Microsoft-trusting shim/GRUB boot path where combined `db` entries
  were used, and authenticated revocation. Once UEFI Setup Mode is reached, the
  enrollment ceremony itself requires zero reboots, and post-enrollment key
  rotation/revocation no longer requires Setup Mode at all — an authenticated update
  signed by the already-owned `KEK` suffices. However, Issue #14 could not establish
  a generic, unattended way for an arbitrary previously-unprepared OEM Endpoint to
  legitimately reach the required UEFI Setup Mode precondition in the first place;
  the only mechanism exercised (`VBoxManage modifynvram inituefivarstore`) is a
  VirtualBox host-side lab shortcut with no physical-Endpoint equivalent.

Neither candidate was shown to support unattended first-trust establishment from an
arbitrary previously-unprepared OEM Endpoint. Both remain gated by a real,
non-amortizing per-Endpoint interactive requirement at initial-enrollment time. Full
evidence, exact commands, versions, and measured interaction/reboot counts are
recorded in `docs/reference/site-trust-anchor-provisioning-spike.md`.

## Decision

For Bamep V1, the default trust-anchor provisioning mechanism for a previously
unprepared, arbitrary Endpoint is **operator-verified first-site-key pairing** — not
automatic trust-on-first-use, and not a machine-specific firmware modification
prerequisite (MOK or direct `db`/PK enrollment).

A candidate site public key **must not** become trusted merely because it was the
first key observed on the provisioning network.

### Required security semantics

1. The Endpoint reaches the trusted maintenance/bootstrap environment whose
   executable integrity is covered by ADR-0010's Secure Boot baseline.
2. Before it has a site trust anchor, the Endpoint obtains a *candidate* Bamep site
   public key through a transport that is not itself assumed trusted.
3. The Endpoint derives a human-verifiable representation from that exact candidate
   public key.
4. The legitimate Bamep installation independently derives/displays the
   corresponding representation from its own site public key, through an
   operator-trusted management context (e.g. Bamep Web/Admin).
5. The operator explicitly compares/verifies the two representations.
6. Only an explicit, successful verification/approval allows the Endpoint to persist
   that site public key as its trust anchor.
7. Mismatch, cancellation, ambiguity, or absent approval fails closed — the Endpoint
   does not persist any candidate key, and `trusted bootstrap established` remains
   `NotEstablished` for that boot context.
8. After successful pairing, subsequent boots do **not** repeat this ceremony unless
   trust has been explicitly reset, revoked, or requires recovery.

The exact human-verifiable representation (a full fingerprint, a shorter
collision-resistant code, a word-based encoding, QR-assisted comparison, or an
equivalent mechanism) is **not selected by this ADR**. Whatever is selected must
provide enough collision resistance for the comparison to be meaningful against an
active-network-attacker threat model. A short unauthenticated "Yes/No accept key?"
prompt does not satisfy this decision.

### Composition with operator-approval-gated Endpoint enrollment (ADR-0004)

Where practical, the site-key verification ceremony composes with the already-accepted
operator-approval-gated first Endpoint enrollment workflow (ADR-0004,
`docs/specifications/m0-endpoint-identity-lifecycle.md`):

```text
Pending site-key verification
        +
PendingEnrollment (Endpoint identity)
        ↓
operator verifies site identity and Endpoint
        ↓
site trust anchor established
        +
Endpoint may become Enrolled
```

These remain **two distinct security checks** even when they share one operator
workflow/UI action:

- "I approve this Endpoint identity" (ADR-0004, `PendingEnrollment` → `Enrolled`).
- "this public key really represents my Bamep site" (this ADR).

Neither check may be inferred from the other. An operator approving an Endpoint does
not, by itself, approve a site key; verifying a site key does not, by itself, approve
an Endpoint identity.

### No-TOFU clarification

ADR-0010's no-TOFU invariant is not reopened and remains intact. Explicitly:

- **Rejected (TOFU)**: first key observed → automatically persisted/trusted.
- **Accepted (Bamep V1)**: first key observed → **not** trusted → operator performs
  an independent, out-of-band comparison → explicit verified approval → trust
  established.

The network alone never establishes trust. If the comparison cannot be completed
(mismatch, cancellation, ambiguity, timeout, or absent approval), trusted bootstrap
remains `NotEstablished` and the Endpoint does not proceed past the fail-closed
behavior already defined in `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
"Failure semantics."

### Persistence and reset semantics (contract-level only)

- A successfully paired site public key becomes durable Endpoint-local bootstrap
  trust state.
- Normal reboot/reconnect does not remove it (consistent with the already-accepted
  boot-session-scoped, not TTL-scoped, semantics of `trusted bootstrap established`).
- Explicit trust reset/revocation removes it.
- A changed candidate site key does not silently replace an already-paired key —
  replacing a paired key requires an explicit rotation or recovery path, never a
  silent overwrite.
- Site-key rotation requires an authenticated rotation path under the existing
  paired key where possible.
- If the previously paired key is unavailable or compromised, recovery returns to an
  explicit operator verification ceremony (this ADR's ceremony again), not to an
  automatic fallback.

The concrete local storage format, exact rotation protocol, and exact recovery UX are
implementation-time details, not decided here.

### Relationship with accepted (C) and (D)

Unchanged. After a site trust anchor is established via this ceremony:

```text
new boot → boot_nonce → nonce-bound signed bootstrap assertion
        → verify assertion under the paired site trust key
        → obtain authenticated Server TLS fingerprint → WSS pinning
```

`BootstrapEvidence` (D) continues to represent the boot-context trust result under the
already-accepted M0 assurance boundary. This ADR does not introduce remote
attestation and does not change Agent Protocol.

### Optional future pre-provisioned trust modes

MOK enrollment and direct UEFI `db`/PK enrollment are recorded as **validated,
technically viable possible future mechanisms** for environments that can
pre-provision Endpoint firmware trust (e.g., a managed fleet with imaging/BMC
infrastructure already capable of driving the interactive ceremonies Issue #14
characterized). They may eventually permit unattended first-site-trust establishment
in such environments. They are **not required for the V1 baseline** and are **not the
default onboarding path**. This ADR does not implement or fully specify either mode;
Issue #14's evidence remains the authoritative record of their validated mechanics and
cost.

## Alternatives considered

- **shim/MOK enrollment as the V1 default.** Rejected as the default: Issue #14
  showed a real, per-Endpoint, non-amortizing interactive ceremony (MokManager +
  physical/console-equivalent presence + 2 reboots) is required for every Endpoint,
  regardless of fleet size, and again on every future rotation/recovery — not
  technically inferior, simply a firmware-modification prerequisite this decision
  avoids requiring as the baseline.
- **Direct UEFI `db`/PK enrollment as the V1 default.** Rejected as the default for
  the same class of reason: it requires the Endpoint to first legitimately reach UEFI
  Setup Mode, and Issue #14 did not establish a generic, unattended way to do that on
  arbitrary previously-unprepared OEM firmware — only a lab-only hypervisor shortcut
  was exercised. Not rejected as technically inferior; its post-enrollment
  authenticated-update story is materially better than MOK's.
- **Automatic trust-on-first-use (TOFU).** Rejected outright — reopens exactly the
  invariant ADR-0010 already established (no TOFU, no acceptance of an unverified
  Server certificate or, by direct extension, an unverified site trust-anchor key
  under any circumstance).
- **A short unauthenticated "accept this key?" prompt without a verifiable
  representation.** Rejected — provides no meaningful resistance to an
  active-network-attacker substituting the candidate key in transit; does not satisfy
  the collision-resistance requirement this decision states explicitly.

## Consequences

- Sub-problem (B) of `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`
  is resolved; no remaining architectural fork blocks that Specification's approval.
- A new, explicit operator-facing verification ceremony is required before V1 ships:
  Bamep Web/Admin must derive and display a human-verifiable representation of the
  site's own public key, and the Endpoint's maintenance/bootstrap environment must
  derive and display the same representation for the candidate key it received. The
  concrete transport, encoding, and UX are future implementation-time work, not
  designed here.
- The composition with ADR-0004's operator-approval-gated Endpoint enrollment must be
  implemented as two distinct, independently auditable approvals, even where a single
  operator workflow surfaces both.
- Bamep V1 does **not** claim cryptographically strong zero-touch first-site trust
  establishment on an arbitrary previously-unprepared OEM Endpoint. First trust
  establishment requires operator verification unless the Endpoint has been
  pre-provisioned through a future supported trust mechanism (MOK or direct
  `db`/PK). After first trust establishment, subsequent normal Bamep boots may be
  unattended. This is an explicit product/security boundary, not an implementation
  defect, and must be represented as such in product-facing documentation.
- MOK and direct `db`/PK enrollment remain validated, available future options for
  managed-fleet pre-provisioning; adopting either as a supported optional mode is a
  future decision, not made here.

## Related architecture

- `docs/decisions/0010-trusted-bootstrap-and-secure-boot-baseline.md` — Secure Boot
  V1 baseline (A) and the no-TOFU invariant this decision extends to the site
  trust-anchor key.
- `docs/decisions/0004-endpoint-identity-and-enrollment-bootstrap.md` — the
  operator-approval-gated Endpoint enrollment model this decision composes with,
  without collapsing into it.
- `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` — the
  contract this decision completes (sub-problem (B)).
- `docs/specifications/m0-endpoint-identity-lifecycle.md` — the Endpoint identity
  lifecycle and destructive-operation preconditions this decision's paired trust
  anchor feeds precondition 7 of, unchanged.

## Related work

- Issue #14 — `[Spike] Validate site trust-anchor provisioning` (complete; empirical
  basis for this decision).
- `docs/reference/site-trust-anchor-provisioning-spike.md` — full empirical evidence,
  commands, versions, and measured costs for both evaluated candidates.
- Issue #13 — `[WP] Define trusted bootstrap and Server fingerprint delivery
  contract` — amended alongside this ADR to record sub-problem (B) as resolved.
- Issue #10 / ADR-0010 — `[Spike] Validate Secure Boot and hardened boot chain` —
  Secure Boot baseline and no-TOFU invariant this decision builds on.
- Issue #2 / ADR-0004 — Endpoint identity and enrollment/trust bootstrap model — the
  companion operator-approval workflow this decision composes with.

## Open questions

1. The concrete human-verifiable representation/encoding (fingerprint, short code,
   word-based, QR-assisted, or equivalent) — explicitly not selected here; a future
   implementation-time design question bound by this ADR's collision-resistance
   requirement.
2. The concrete transport used in step 2 (how the candidate public key reaches the
   Endpoint's maintenance/bootstrap environment before a trust anchor exists) — not
   selected here; remains subject to the same "transport need not itself be trusted"
   framing already established for (C)'s signed bootstrap assertion.
3. Whether any new Agent Protocol message is genuinely required to support the
   pairing ceremony, versus the ceremony completing entirely before Agent Protocol
   authentication begins (as (C)'s sequence already does) — not decided here; `
   BootstrapEvidence` (D) is unchanged, and no protocol change is introduced by this
   ADR. If implementation reveals a genuine need, that follow-up must go through its
   own Specification/ADR treatment, not be introduced silently.
4. Concrete local storage format, rotation protocol, and recovery UX for the paired
   site key — implementation-time detail, intentionally not decided here.
5. Whether MOK or direct `db`/PK enrollment is ever adopted as a supported optional
   pre-provisioned mode for managed fleets — a future decision, not made here.

Status: Accepted.
