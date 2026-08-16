# M0 — Trusted Bootstrap and Server Fingerprint Delivery Contract

Status: **Approved**

## Context

This Specification defines the explicit M0 contract that turns ADR-0010's `trusted bootstrap established` security property into an independently implementable Server / boot-boundary / Agent contract, executing Issue #13 (`[WP] Define trusted bootstrap and Server fingerprint delivery contract`). It closes the gap ADR-0010 deliberately left open: Secure Boot authenticates *executable* boot-chain integrity, but does not by itself authenticate the *site-specific bootstrap data* (the expected Server TLS fingerprint, and enrollment context where applicable) that Agent Protocol v1 requires before authentication.

This Specification defines the **contract** only. No production implementation is part of this Work Package.

It consumes, without redefining:

- `docs/decisions/0010-trusted-bootstrap-and-secure-boot-baseline.md` — the `trusted bootstrap established` property and the Secure Boot V1 baseline this contract builds on.
- `docs/specifications/m0-agent-protocol-contract.md` — the WSS/pinned-TLS handshake, and the `BootstrapEvidence` message this round adds to carry sub-problem (D)'s evidence; **this Work Package's second round amended that Specification directly**, adding `BootstrapEvidence` without changing WSS, pinned TLS, `AuthRequest`, `SessionEstablished`, or ADR-0005.
- `docs/specifications/m0-endpoint-identity-lifecycle.md` — destructive-operation precondition 7, whose authoritative fact this contract defines the origin of.
- `docs/specifications/m0-job-lifecycle-and-scheduling.md` — the revalidation ordering precondition 7 already participates in; unchanged.
- `docs/specifications/m0-simulator-contract-and-validation-strategy.md` — the Simulator fidelity boundary and fixture-ownership split this contract fills in.
- `docs/specifications/m0-stack-and-boundaries-baseline.md` — the Boot Port/Adapter boundary this contract's mechanics remain behind.
- `docs/reference/secure-boot-hardened-chain-spike.md` — the empirical evidence this contract's design is grounded in (see "Four distinct sub-problems").
- `docs/reference/site-trust-anchor-provisioning-spike.md` — the empirical evidence (Issue #14) sub-problem (B)'s accepted mechanism is grounded in.
- `docs/decisions/0011-site-trust-anchor-operator-verified-pairing.md` — the accepted decision for sub-problem (B), consumed and cross-referenced, not redefined, by this round.

## Owner-review status

All four sub-problems are now **accepted**:

- **(C) Authenticated/fresh bootstrap material — ACCEPTED**: the nonce-bound signed bootstrap assertion (previously "Candidate B") is the M0 mechanism. The static signed manifest is **not** the M0 baseline.
- **(D) Server-side bootstrap evidence — ACCEPTED**: an authenticated Agent bootstrap report (`BootstrapEvidence`, now added to `docs/specifications/m0-agent-protocol-contract.md`), explicitly **not** hardware-backed remote attestation.
- **(B) Site trust-anchor provisioning — ACCEPTED (ADR-0011)**: operator-verified first-site-key pairing is the V1 default mechanism, informed by Issue #14's empirical evidence for MOK and direct UEFI `db`/PK enrollment (both validated, neither selected as the V1 default). No remaining architectural fork blocks this Specification.

**Status: Approved.**

## Goal

Define enough of the trusted-bootstrap and Server-fingerprint-delivery contract that it is independently implementable, without inventing a general-purpose PKI or secrets platform, and without hiding a required architectural decision inside a future implementation Work Package.

## Scope

- the semantic meaning, ownership, and scoping of `trusted bootstrap established`, distinguishing local (Agent-side) establishment from Server-side authoritative knowledge of it;
- the minimum authenticated bootstrap-material contract;
- **(A)** restating, not redeciding, boot executable trust (ADR-0010);
- **(B)** site trust-anchor provisioning — **accepted**: operator-verified first-site-key pairing (ADR-0011), restated, not redecided, here;
- **(C)** the accepted nonce-bound signed bootstrap assertion mechanism;
- **(D)** the accepted authenticated Agent bootstrap report mechanism, and its explicit assurance limitations;
- the M0 threat-model boundary this design is, and is not, intended to defend against;
- the Agent-integrity requirement that makes (D) meaningful;
- rotation, revocation, recovery, and fail-closed behavior;
- the Agent bootstrap sequence up to and including Agent Protocol authentication and evidence reporting;
- failure semantics for every unsafe case;
- Simulator fixture semantics;
- validation expectations across all layers, including Integration Environment.

## Out of scope

- production implementation code;
- selecting the final PXE/network bootloader mechanism (Issue #8's network-delivered boot mechanism remains separately unresolved and is not selected here);
- resolving Issue #8's physical network-delivery uncertainty;
- real Secure Boot deployment/configuration, or firmware key-enrollment tooling;
- a broad enterprise PKI;
- Administrative API/Web authentication, multi-user support, or RBAC;
- data-plane transfer authentication;
- driver-provider behavior;
- any change to Agent Protocol v1 transport, WSS, pinned TLS, `AuthRequest`/`SessionEstablished` semantics, or ADR-0005 — the `BootstrapEvidence` addition to `docs/specifications/m0-agent-protocol-contract.md` is strictly additive;
- any change to Job/JobStep/Attempt lifecycle or states;
- production provisioning;
- selecting a concrete network transport (HTTP/TFTP/PXE/etc.) for bootstrap-assertion delivery;
- concrete signature algorithm or serialization format, unless an existing accepted project-wide convention already owns that decision (none does yet);
- the concrete human-verifiable representation/encoding, transport, and UX for the (B) operator-verified pairing ceremony — accepted at the contract level (ADR-0011), not designed to implementation detail here; see "(B) Site trust-anchor provisioning";
- hardware-backed remote attestation (e.g., measured boot / TPM-class functionality) — explicitly not introduced as an M0 requirement (see "M0 threat-model boundary").

## Four distinct sub-problems (do not collapse)

- **(A) Boot executable trust** — ADR-0010 / the Secure Boot baseline. Already accepted, not reopened here.
- **(B) Site trust-anchor provisioning** — how an arbitrary Endpoint learns a public key that legitimately belongs to *this* Bamep installation. **Accepted**: operator-verified first-site-key pairing (see "(B) Site trust-anchor provisioning" and ADR-0011).
- **(C) Authenticated/fresh bootstrap material** — **Accepted**: the nonce-bound signed bootstrap assertion (see "(C) Authenticated and fresh bootstrap material").
- **(D) Server-side bootstrap evidence** — **Accepted**: the authenticated Agent bootstrap report, explicitly not remote attestation (see "(D) Server-side bootstrap evidence").

### Evidence characterization

`docs/reference/secure-boot-hardened-chain-spike.md` Scenario 3 empirically demonstrated exactly: **firmware Secure Boot → Microsoft-trusted shim → Canonical-signed GRUB**, reaching a genuine interactive `grub>` prompt — the evidentiary basis for (A). `docs/reference/site-trust-anchor-provisioning-spike.md` (Issue #14) subsequently validated MOK enrollment and direct UEFI `db`/PK enrollment end-to-end (enrollment, functional trust verification, revocation), the evidentiary basis for (B)'s accepted default and for recording both mechanisms as validated optional future pre-provisioned modes (ADR-0011) — neither Endpoint-firmware-modification mechanism was selected as the V1 default because neither was shown to support unattended first-trust establishment from an arbitrary previously-unprepared OEM Endpoint.

## 1. Trusted-bootstrap semantic model

`trusted bootstrap established` is a fact about **the current Agent boot/session context**, produced by the Boot Adapter boundary and exposed upward through Application-level Boot Orchestration — never as `SecureBootEnabled`, and never inspected directly by Domain code (`docs/specifications/m0-stack-and-boundaries-baseline.md`).

**The fact requires two things to both hold, not Secure Boot (A) alone** (ADR-0010 point 7):

1. **(A) Executable boot-chain integrity** — Secure Boot, already accepted, not reopened here.
2. **(B)+(C) Authenticated site-specific bootstrap material** — the expected Server TLS fingerprint has been cryptographically authenticated using a legitimately-provisioned trust anchor. Both are now accepted: (C) the nonce-bound signed bootstrap assertion mechanism, and (B) how that trust anchor is legitimately provisioned to the Endpoint in the first place — by default, the operator-verified first-site-key pairing ceremony (ADR-0011).

**Local establishment vs. Server-side authority are distinct.** The Agent locally determines, at boot time, whether (1) and (2) hold for itself — **local establishment**, sufficient to gate the Agent's own willingness to proceed (see "Failure semantics"). Making the fact **Server-observable** is (D), now accepted via `BootstrapEvidence` (see "(D) Server-side bootstrap evidence") — but note (D)'s explicit assurance limitations before treating it as equivalent to (A) actually having held.

**Ownership:** the Boot Adapter observes/produces the raw evidence; Application-level Boot Orchestration composes it into the exposed fact `trusted_bootstrap: Established | NotEstablished`, consumed by Endpoint identity precondition 7 (Server-side, now possible via accepted (D)) and the Agent's own pre-connection gate (Agent-side, local establishment only).

**Scope: boot-session-scoped.** Established once per boot cycle, valid for the entire boot session. Agent Protocol reconnect within the same boot session does not require re-establishment; a genuine reboot does. No in-session expiry timer — boot-scoped, not TTL-scoped.

**Independence from credential validity is preserved exactly as ADR-0010/precondition 7 require**: `CredentialActive` proves the Agent authenticated successfully; it does not prove the boot path was trusted, locally or to the Server — this is precisely why (D) is needed as a distinct mechanism rather than an inference from authentication success.

## 2. Bootstrap material

- **Expected Server TLS certificate fingerprint** — always required.
- **Enrollment/bootstrap context** — required only if the future pre-authorized enrollment capability is in use; **not required for M0's default operator-approval-gated enrollment path**. See "Confidentiality boundary" below for why this field's future semantics are explicitly not decided here.
- **Explicit domain/contract discriminator and schema/contract version** — so the signer and verifier agree the signed structure is a Bamep bootstrap assertion of a known shape, not an arbitrary signed byte string (see "(C)" for why this matters).
- **`boot_nonce`** — the freshness primitive; see "(C)."
- **Signing-key identifier / verification metadata** — which trust-anchor key the assertion claims to be signed by, distinct from *whether* that key is actually trusted (sub-problem (B)).

No other configuration is added merely because a bootstrap object exists. The digest/hash algorithm used to represent the fingerprint itself is **not selected here**, consistent with ADR-0008 point 3's already-deferred `digest_algorithm` selection.

## (B) Site trust-anchor provisioning

**Accepted (ADR-0011): operator-verified first-site-key pairing** is the V1 default
mechanism by which a previously unprepared, arbitrary Endpoint legitimately learns the
public key belonging to a specific Bamep installation. This is restated from
ADR-0011, not redecided here; ADR-0011 remains the authoritative decision record.

**This is explicitly not automatic trust-on-first-use.** A candidate site public key
must not become trusted merely because it was the first key observed on the
provisioning network.

**Required security semantics:**

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
7. Mismatch, cancellation, ambiguity, or absent approval fails closed — no candidate
   key is persisted, and `trusted bootstrap established` remains `NotEstablished`
   (see "Failure semantics").
8. After successful pairing, subsequent boots do not repeat this ceremony unless
   trust has been explicitly reset, revoked, or requires recovery.

The exact human-verifiable representation (full fingerprint, shorter
collision-resistant code, word-based encoding, QR-assisted comparison, or an
equivalent mechanism) is **not selected here** — implementation-time, bound by
ADR-0011's requirement that it provide enough collision resistance to be meaningful
against an active-network-attacker threat model. A short unauthenticated "Yes/No
accept key?" prompt does not satisfy this contract.

**Composition with Endpoint enrollment (ADR-0004):** where practical, this ceremony
composes with the already-accepted operator-approval-gated first Endpoint enrollment
workflow (`docs/specifications/m0-endpoint-identity-lifecycle.md`) in a single
operator workflow, but remains a **distinct security check** — "I approve this
Endpoint identity" and "this public key really represents my Bamep site" are never
inferred from one another.

**No-TOFU clarification:** ADR-0010's no-TOFU invariant is not reopened. First key
observed → **not** trusted → operator performs an independent, out-of-band
comparison → explicit verified approval → trust established. The network alone never
establishes trust.

**Persistence and reset (contract-level):** a successfully paired site public key
becomes durable Endpoint-local bootstrap trust state; normal reboot/reconnect does
not remove it; explicit reset/revocation does; a changed candidate key never silently
replaces an already-paired key; rotation requires an authenticated path under the
existing paired key where possible; recovery from an unavailable/compromised paired
key returns to an explicit operator verification ceremony. Concrete local storage
format, rotation protocol, and recovery UX remain implementation-time (see "Open
questions").

**MOK and direct UEFI `db`/PK enrollment** were both empirically validated end-to-end
by Issue #14 (`docs/reference/site-trust-anchor-provisioning-spike.md`) and are
recorded as **validated, technically viable optional future pre-provisioned trust
modes** for environments that can pre-provision Endpoint firmware trust (e.g. a
managed fleet with imaging/BMC infrastructure). Neither is required for the V1
baseline, neither is the default onboarding path, and neither is implemented or
further specified by this Specification. See ADR-0011 "Alternatives considered" for
the full evidence-driven rationale for not selecting either as the V1 default.

**Product limitation, stated explicitly:** Bamep V1 does not claim cryptographically
strong zero-touch first-site trust establishment on an arbitrary previously-unprepared
OEM Endpoint. First trust establishment requires operator verification unless the
Endpoint has been pre-provisioned through a future supported trust mechanism. After
first trust establishment, subsequent normal Bamep boots may be unattended. This is an
explicit product/security boundary, not an implementation defect.

## (C) Authenticated and fresh bootstrap material — ACCEPTED: nonce-bound signed bootstrap assertion

**The static signed manifest is not the M0 baseline.** Its unresolved replay/freshness gap (a validly-signed static artifact remains validly-signed indefinitely, with no structural way to distinguish "current" from "superseded" without an additional, separately-trusted mechanism) is why it was not accepted.

**Accepted contract:**

1. At each new boot context, the trusted bootstrap stage generates a cryptographically random `boot_nonce`.
2. It obtains a signed bootstrap assertion through a transport that need not itself be trusted — authenticity/integrity comes from the signature and nonce binding, not from transport security. No concrete transport is selected here (see "Out of scope"; Issue #8 remains independently unresolved).
3. The signed assertion covers, at minimum, as one signed unit:
   - an explicit domain/contract discriminator (so the signer cannot be tricked into signing an unrelated structure that happens to parse compatibly);
   - schema/contract version;
   - the exact `boot_nonce`;
   - the expected Server TLS certificate fingerprint;
   - signing-key identifier / verification metadata;
   - enrollment/bootstrap context, **only** where a separately-defined enrollment mechanism requires it (see "Confidentiality boundary" below).
4. **The signer signs this fixed, structured assertion — it must not act as a generic arbitrary-byte signing oracle.** Signing an attacker-chosen arbitrary payload under the site key would defeat the discriminator/schema protections above; the signer's role is scoped to producing exactly this structure.
5. The bootstrap stage verifies: the signature; the signer against the already-provisioned site trust anchor (B); the schema/version; the exact nonce match; and required-field validity.
6. Only successful verification makes the authenticated Server fingerprint locally usable for WSS pinning.
7. A replayed assertion bound to a different `boot_nonce` fails closed.
8. The assertion is **boot-context-scoped**: a WSS reconnect during the same boot does not require a new assertion merely because the socket changed; a genuine reboot generates a new nonce and requires a new assertion.

No concrete signature algorithm or serialization format is selected — implementation-time, unless a future project-wide convention already decides it.

### Confidentiality boundary

**A digital signature supplies authenticity/integrity, not confidentiality.** M0's required Server TLS fingerprint is not treated as secret — it is authenticated data, not protected data, and this contract's guarantees are about *tampering*, not *disclosure*.

The future *optional* enrollment/bootstrap context (Section 2) must **not** be assumed safe to expose over an unauthenticated plaintext transport if that future mechanism gives it bearer-secret or confidential semantics (e.g., a pre-authorization token that grants trust merely by possession). **This Specification does not design that future enrollment mechanism, and does not resolve its confidentiality/binding question** — that mechanism, when and if it is specified, owns deciding whether its own bootstrap context requires confidentiality in addition to the authenticity this contract already provides.

## (D) Server-side bootstrap evidence — ACCEPTED: authenticated Agent bootstrap report (not remote attestation)

**Accepted M0 model:**

```text
local trusted boot evaluation (A)
        +
locally verified nonce-bound signed assertion (B)+(C)
        ↓
Agent establishes local trusted-bootstrap result
        ↓
WSS pinned TLS succeeds using the assertion fingerprint
        ↓
Agent Protocol credential authentication succeeds
        ↓
authenticated Agent sends bootstrap evidence/report
        (`BootstrapEvidence`, `docs/specifications/m0-agent-protocol-contract.md`)
        ↓
Server validates assertion + correlates boot context
        ↓
Server records trusted bootstrap = Established
        for that current boot context
        ↓
destructive precondition 7 may now be satisfied
```

**The Server MUST NOT infer this fact merely from:** a TCP/WSS connection; a fingerprint match alone; `CredentialActive`; or mere possession of a valid assertion without performing independent verification. Only the explicit sequence above — ending in independently-verified `BootstrapEvidence` — establishes the Server-side fact.

### What this proves, and what it does not (corrected from the prior round)

**Forwarding the signed assertion to the Server, and the Server independently verifying it, does not by itself prove the Endpoint booted through Secure Boot / trusted bootstrap.** Server-side verification of the assertion proves only:

- the assertion was produced by the accepted site signer;
- its authenticated material (the fingerprint, nonce, etc.) is intact;
- it is bound to the declared `boot_nonce`/boot context.

It does **not** independently prove:

- that firmware Secure Boot was actually enabled for this boot;
- that the expected executable chain actually executed;
- that the current Agent process itself is genuine, as opposed to a substitute able to replay or fabricate a report.

**(A) local boot-chain establishment and (B)+(C) assertion authentication remain distinct facts.** `BootstrapEvidence` is Server-observable evidence of (B)+(C) having been locally verified by *something* claiming to be the Agent; it is not independent proof of (A). This distinction is exactly why the model is an **authenticated Agent report**, not remote attestation — see "M0 threat-model boundary" for what assurance this does and does not provide, and "Agent-integrity requirement" for what production must additionally guarantee to make this report meaningful.

## M0 threat-model boundary

This M0 mechanism is designed to protect against the already-accepted threat boundary, including:

- an untrusted provisioning network;
- Server/fingerprint substitution;
- tampering with or replay of bootstrap material;
- accidental or stale bootstrap context.

**It does not claim cryptographic remote attestation against a malicious/fully-compromised Endpoint capable of executing a counterfeit authenticated Agent or falsifying local platform state.** Providing that stronger property would require a separately justified hardware-backed attestation design (for example, measured boot or TPM-class functionality), which is **not** introduced as an M0 requirement — no such requirement is evidenced anywhere in the accepted M0 architecture.

**This is a limitation of assurance, not permission to bypass Secure Boot.** Production Bamep still requires the trusted executable-bootstrap baseline from ADR-0010 in full; this mechanism supplements it with site-specific data authentication, it does not substitute for it.

## Agent-integrity requirement

**An authenticated Agent bootstrap report is meaningful only if the Agent code and the logic producing that report are themselves covered by the trusted maintenance-boot integrity boundary.** This is persisted as a production requirement: "Secure Boot enabled" does not automatically verify an arbitrary Bamep user-space Agent binary — the executable chain of trust (A) must extend to the specific code that generates and sends `BootstrapEvidence`, not stop short of it.

The final Boot Adapter / maintenance-environment implementation must provide an authenticated path from the accepted boot chain to the Agent code/configuration that produces the bootstrap report. **The exact packaging mechanism is not selected here** — GRUB, a Unified Kernel Image (UKI), WinPE-specific packaging, an initramfs layout, or another mechanism all remain candidates, dependent on the eventual production boot implementation (Issue #8's still-unresolved network-delivered mechanism) and future Integration Environment validation. This requirement constrains that future work; it does not design it.

## 5. Rotation, revocation, and recovery

- **Legitimate Server TLS certificate/fingerprint rotation does not require site-trust-anchor rotation.** Once the Server begins using a new certificate, newly issued valid assertions contain the corresponding authenticated fingerprint, signed by the same still-valid trust-anchor key — no boot-media refresh or trust-anchor change is required for routine rotation.
- **Operational overlap.** Currently-running boot contexts may finish under their already-established assertion; a new boot receives a new nonce and a new assertion reflecting current material. **No arbitrary time window is defined here without evidence** — exact overlap/transition duration is implementation-time.
- **Signing/trust-anchor key rotation** follows the contract-level semantics accepted in "(B) Site trust-anchor provisioning" (ADR-0011): an authenticated rotation path under the existing paired key where possible; recovery from an unavailable/compromised key returns to an explicit operator verification ceremony. Concrete rotation protocol/UX remains implementation-time.
- **Compromised/revoked bootstrap material or key** fails closed (see "Failure semantics"); revocation mechanics follow the same (B) contract-level semantics.
- **No silent TOFU or multiple simultaneously-accepted unverified fingerprints** are introduced by rotation — exactly one authenticated fingerprint is accepted per successful bootstrap sequence.

## 6. Agent bootstrap sequence

```text
1. Firmware Secure Boot verifies the executable boot chain (A, ADR-0010) up
   through the trusted bootstrap stage and the Agent process launch
   (subject to the Agent-integrity requirement above).
2. The trusted bootstrap stage generates a `boot_nonce` and obtains a
   signed bootstrap assertion through a transport that need not itself be
   trusted (see "(C)").
3. The assertion's signature and exact nonce binding are verified against
   the site trust-anchor public key provisioned via the operator-verified
   first-site-key pairing ceremony (ADR-0011), or, where a future optional
   pre-provisioned mode is adopted, via MOK or direct UEFI db/PK enrollment.
     - Verification failure → trusted bootstrap is NOT established locally
       → go to "Failure semantics"; the sequence does not proceed to step 4.
4. On successful local verification: the trusted-bootstrap fact becomes
   `Established` for this boot session, locally, at the Agent.
5. The Agent opens a WSS connection to the Server.
6. The Agent verifies the Server's presented TLS certificate fingerprint
   against the authenticated expected fingerprint from step 4 — unchanged
   from `m0-agent-protocol-contract.md`.
7. On fingerprint match: Agent Protocol authentication begins
   (`AuthRequest`/`SessionEstablished`/`AuthError`), unchanged.
8. On `SessionEstablished`: the authenticated Agent sends `BootstrapEvidence`
   (`boot_nonce`, the assertion, `local_boot_trust: Established`).
9. The Server independently verifies the assertion and correlates it to the
   current boot context via `boot_nonce`; on success, the Server records
   `trusted bootstrap = Established` for that boot context, making
   destructive-operation precondition 7 satisfiable.
```

Steps 1–4 and 8–9 are defined by this Specification, with (B) accepted (ADR-0011) as the mechanism steps 2–4 and 9 depend on for how the verifying trust anchor was itself legitimately established. Steps 5–7 restate the already-accepted Agent Protocol handshake unchanged.

## 7. Failure semantics

- **Trusted bootstrap cannot be established locally** (Section 6 step 3): the Agent must not proceed to step 5, must not treat any received Server certificate as verified if it connects anyway, and must not proceed to Agent Protocol authentication. Fail-closed, no retry under a different trust assumption, no fallback, no TOFU.
- **`BootstrapEvidence` is missing, malformed, invalid-signature, or nonce-mismatched** (Server-side, step 9): the Server-side fact remains `NotEstablished` for that boot context; destructive-operation precondition 7 cannot be satisfied. This does not affect the Agent's already-established credential session or non-destructive operation.
- **TLS fingerprint mismatch at the Agent Protocol layer** (step 6) remains exactly as already defined in `docs/specifications/m0-agent-protocol-contract.md`.

## 8. Simulator contract

Consistent with the already-accepted Simulator fidelity boundary: the Simulated Agent uses the real Agent Protocol v1 WSS transport end-to-end, including the real `BootstrapEvidence` message; only the production boot chain (including this contract's boot-stage mechanics) is faked.

**Fixture semantics owned by this Specification** (concrete fixture file/schema/token format remains implementation-time):

- **Positive fixture:** a valid nonce-bound signed assertion (matching the Simulator's real test-Server TLS certificate, since the real WSS fingerprint check must genuinely succeed) **plus** an authenticated `BootstrapEvidence` report with `local_boot_trust: Established`, correctly bound to the fixture's `boot_nonce`.
- **Negative variants**, each required: signature failure (assertion signed by an untrusted/wrong key); nonce mismatch/replay (assertion bound to a different `boot_nonce` than the one presented); absent evidence (`SessionEstablished` succeeds but no `BootstrapEvidence` is ever sent); and — consistent with the already-specified "Required trusted-bootstrap independence scenario" (`docs/specifications/m0-simulator-contract-and-validation-strategy.md`) — the case where all other six preconditions hold but this Server-side fact is never established.
- The Simulator does **not** claim to validate real firmware Secure Boot, real trust-anchor provisioning, or real Agent-integrity packaging — those remain Integration Environment concerns.
- **The Simulator fixture necessarily represents only (A)+(B)+(C) local establishment plus the (D) reporting mechanism** — it cannot, and does not claim to, validate that a real production boot chain, or a real operator-verified pairing ceremony, would have genuinely produced that state; that gap is inherent to the assurance limitation already recorded in "M0 threat-model boundary," not introduced by the Simulator contract itself. (B) being accepted at the architecture level (ADR-0011) does not change this — the Simulator still fakes the mechanism, exactly as it already fakes real firmware Secure Boot.

`docs/specifications/m0-simulator-contract-and-validation-strategy.md` itself is not modified by this Work Package — no direct contradiction with it was found requiring amendment; the fixture semantics above are owned here and referenced from there via the already-existing required scenario.

## 9. Validation expectations

Per `docs/development/testing.md` "Unit and domain tests": bootstrap-assertion parsing/schema validation as pure domain/contract logic; precondition-7 consumption tests already specified in `m0-job-lifecycle-and-scheduling.md` and `m0-endpoint-identity-lifecycle.md` are confirmed aligned, not redefined here.

Per `docs/development/testing.md` "Contract tests": assertion signature verification — valid accepted; invalid/corrupted/wrong-signer rejected; nonce mismatch (replay) rejected; missing material handled explicitly. `BootstrapEvidence` contract tests are now specified directly in `docs/specifications/m0-agent-protocol-contract.md` "Validation expectations (contract tests)."

Per general negative-case practice: Agent-side fail-closed verification (local, step 3) and Server-side fail-closed verification (step 9) are tested independently — a failure at either layer must not be masked by success at the other.

Per `docs/development/testing.md` "Simulator": the required trusted-bootstrap independence scenario plus the positive/negative fixture variants in Section 8.

Per `docs/development/testing.md` "Integration Environment": the real operator-verified site-key pairing ceremony (ADR-0011) — including whether an arbitrary previously-unprepared OEM Endpoint can complete it, and the physical-firmware behavior of any future optional MOK/db-PK pre-provisioned mode adopted — real Agent-integrity packaging, and real end-to-end Secure-Boot-backed production chain validation remain explicitly deferred.

Per "Local development environments," domain/contract tests are expected to run in the Linux reference environment (WSL2 or containers from Windows).

Manual: owner approval of this Specification — **given**; all four sub-problems ((A) restated, (B), (C), (D)) are accepted, per Issue #14's empirical evidence and ADR-0011.

## Architectural constraints (restated, unchanged)

- ADR-0010 remains authoritative; not reopened.
- ADR-0011 remains authoritative for site trust-anchor provisioning (B); not reopened.
- ADR-0005 remains authoritative; WSS + pinned Server TLS authentication remains the Agent control-plane architecture; not reopened.
- No TOFU; no acceptance of an unverified Server certificate, or an unverified site trust-anchor key, under any circumstance.
- `CredentialActive` does not imply trusted bootstrap, locally or to the Server; trusted bootstrap does not imply `CredentialActive`.
- Secure Boot mechanics stay behind the Boot Adapter boundary; Domain code does not inspect firmware state.
- The network-delivered WinPE mechanism (Issue #8) remains separately unresolved and is not selected by this Work Package.
- The seven destructive-operation preconditions are unchanged.
- This contract does not become a general secrets, identity, or PKI platform.
- No hardware-backed remote attestation (TPM/measured boot) is introduced as an M0 requirement.

## Acceptance criteria

An owner-approved Specification defines:

1. the exact semantic meaning and scope of `trusted bootstrap established` — satisfied.
2. the minimum authenticated bootstrap-material contract — satisfied.
3. the mechanism by which the expected Server TLS fingerprint is cryptographically bound to trusted bootstrap — **satisfied and accepted**: nonce-bound signed bootstrap assertion.
4. trust-anchor/key ownership sufficient for independent implementation — **satisfied and accepted**: operator-verified first-site-key pairing (ADR-0011), with MOK and direct UEFI `db`/PK enrollment recorded as validated optional future modes.
5. rotation/revocation/recovery and fail-closed behavior — satisfied for (B), (C), and (D); concrete encoding/transport/storage details remain implementation-time (see "Open questions").
6. Agent bootstrap ordering before WSS/Agent Protocol authentication, and evidence reporting after — satisfied.
7. how destructive-operation precondition 7 obtains its authoritative fact — **satisfied and accepted**: the authenticated Agent bootstrap report (`BootstrapEvidence`), with its assurance limitations explicit.
8. Simulator fixture semantics and negative cases — satisfied.
9. contract-test and Integration Environment validation expectations — satisfied.
10. no remaining architectural decision required to implement this boundary is hidden inside a future implementation Work Package — **satisfied; no genuine architectural fork remains open in this Specification.**

## Related ADRs

- ADR-0010 remains authoritative for the Secure Boot/trusted-bootstrap baseline (A); not reopened.
- **ADR-0011 — V1 site trust-anchor establishment and operator-verified first-key pairing (`Accepted`)** — the decision record for sub-problem (B), created alongside this amendment; restated, not redefined, here.
- The accepted (C) nonce-assertion and (D) Server-evidence decisions remain recorded directly in this Specification, as scoped extensions of the already-Accepted ADR-0010 rather than new durable boundaries independent of it, per `docs/development/documentation-policy.md`'s ADR criteria.

## Related work

- Issue #13 — `[WP] Define trusted bootstrap and Server fingerprint delivery contract` — this Specification's approval blocker is now resolved.
- Issue #14 / ADR-0011 — `[Spike] Validate site trust-anchor provisioning` (complete) and `docs/reference/site-trust-anchor-provisioning-spike.md` — empirical evidence for (B)'s accepted mechanism and the recorded optional future modes.
- Issue #10 / ADR-0010 — `[Spike] Validate Secure Boot and hardened boot chain` (complete; origin of `trusted bootstrap established` and the Scenario 3 evidence (A) and (B) candidates were evaluated against).
- Issue #3 / ADR-0005 / `m0-agent-protocol-contract.md` — WSS/pinned-TLS handshake; amended in an earlier round to add `BootstrapEvidence`, without reopening WSS, pinned TLS, `AuthRequest`, `SessionEstablished`, or ADR-0005.
- Issue #2 / ADR-0004 / `m0-endpoint-identity-lifecycle.md` — destructive-operation precondition 7, and the operator-approval-gated Endpoint enrollment workflow the (B) pairing ceremony composes with as a distinct check.
- Issue #4 / ADR-0006 / `m0-job-lifecycle-and-scheduling.md` — precondition-7 revalidation ordering; unchanged.
- Issue #7 / `m0-simulator-contract-and-validation-strategy.md` — Simulator fidelity boundary and fixture-ownership split; not modified by this round.
- Issue #1 / `m0-stack-and-boundaries-baseline.md` — Boot Port/Adapter boundary this contract's mechanics remain behind.

## Open questions

No genuine architectural fork remains open in this Specification. Remaining questions
are implementation-time details, not architectural blockers:

1. The concrete human-verifiable representation/encoding for the (B) pairing ceremony (fingerprint, short code, word-based, QR-assisted, or equivalent) — bound by ADR-0011's collision-resistance requirement, not selected here.
2. The concrete transport used to deliver the candidate site public key to the Endpoint before a trust anchor exists (ADR-0011 step 2) — not selected here; follows the same "transport need not itself be trusted" framing already established for (C).
3. Whether any new Agent Protocol message is genuinely required to support the pairing ceremony, versus the ceremony completing entirely before Agent Protocol authentication begins — not decided here; `BootstrapEvidence` (D) is unchanged. Any genuine need discovered during implementation must go through its own Specification/ADR treatment, not be introduced silently.
4. Concrete local storage format, rotation protocol, and recovery UX for the paired site key — implementation-time, not decided here.
5. Exact overlap/transition duration for material or key rotation; concrete bootstrap-assertion wire format and signature algorithm; concrete Simulator fixture file/configuration technique; the exact Agent-integrity packaging mechanism (GRUB/UKI/WinPE-specific/initramfs), dependent on Issue #8's still-unresolved network-delivery mechanism.
6. Whether MOK or direct UEFI `db`/PK enrollment is ever adopted as a supported optional pre-provisioned mode for managed fleets — a future decision, not made here (ADR-0011).

Status: Approved.
