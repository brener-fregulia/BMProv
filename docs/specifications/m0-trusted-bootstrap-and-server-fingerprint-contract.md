# M0 — Trusted Bootstrap and Server Fingerprint Delivery Contract

Status: **Proposed - awaiting owner approval**

## Context

This Specification defines the explicit M0 contract that turns ADR-0010's `trusted bootstrap established` security property into an independently implementable Server / boot-boundary / Agent contract, executing Issue #13 (`[WP] Define trusted bootstrap and Server fingerprint delivery contract`). It closes the gap ADR-0010 deliberately left open: Secure Boot authenticates *executable* boot-chain integrity, but does not by itself authenticate the *site-specific bootstrap data* (the expected Server TLS fingerprint, and enrollment context where applicable) that Agent Protocol v1 requires before authentication (`docs/specifications/m0-agent-protocol-contract.md` "Transport and handshake").

This Specification defines the **contract** only — the semantic model, the material format, the cryptographic-binding mechanism, trust-anchor ownership, rotation/revocation/failure behavior, the Agent bootstrap sequence, and Simulator fixture semantics. No production implementation is part of this Work Package.

It consumes, without redefining:

- `docs/decisions/0010-trusted-bootstrap-and-secure-boot-baseline.md` — the `trusted bootstrap established` property and the Secure Boot V1 baseline this contract builds on.
- `docs/specifications/m0-agent-protocol-contract.md` — the WSS/pinned-TLS handshake this contract feeds the expected fingerprint into; unchanged.
- `docs/specifications/m0-endpoint-identity-lifecycle.md` — destructive-operation precondition 7, whose authoritative fact this contract defines the origin of.
- `docs/specifications/m0-job-lifecycle-and-scheduling.md` — the revalidation ordering precondition 7 already participates in; unchanged.
- `docs/specifications/m0-simulator-contract-and-validation-strategy.md` — the Simulator fidelity boundary and fixture-ownership split this contract fills in.
- `docs/specifications/m0-stack-and-boundaries-baseline.md` — the Boot Port/Adapter boundary this contract's mechanics remain behind.
- `docs/reference/secure-boot-hardened-chain-spike.md` — the empirical evidence (Scenarios 1–3) this contract's mechanism design is grounded in, including the shim/MOK (Machine Owner Key) pattern Scenario 3 demonstrated works in the authorized virtualized environment.

## Goal

Define enough of the trusted-bootstrap and Server-fingerprint-delivery contract that it is independently implementable, without inventing a general-purpose PKI or secrets platform, and without hiding a required architectural decision inside a future implementation Work Package.

## Scope

- the semantic meaning, ownership, and scoping of `trusted bootstrap established`;
- the minimum authenticated bootstrap-material contract (Server fingerprint, enrollment context where applicable);
- the cryptographic-binding mechanism authenticating that material through the trusted executable chain;
- trust-anchor and key-ownership model sufficient for independent implementation;
- rotation, revocation, recovery, and fail-closed behavior;
- the Agent bootstrap sequence up to and including Agent Protocol authentication;
- failure semantics for every unsafe case;
- Simulator fixture semantics (ownership only — not the concrete fixture format);
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
- any change to Agent Protocol v1 transport, message semantics, or ADR-0005;
- any change to Job/JobStep/Attempt lifecycle or states;
- production provisioning.

## 1. Trusted-bootstrap semantic model

`trusted bootstrap established` is a fact about **the current Agent boot/session context**, produced by the Boot Adapter boundary and exposed upward through Application-level Boot Orchestration as a simple, firmware-independent assertion — never as `SecureBootEnabled`, and never inspected directly by Domain code (`docs/specifications/m0-stack-and-boundaries-baseline.md`).

**The fact requires two things to both hold, not Secure Boot alone** (ADR-0010 point 7):

1. **Executable boot-chain integrity** — the chain of executables that led to the current Agent instance running was signature-verified end-to-end (Secure Boot, `docs/reference/secure-boot-hardened-chain-spike.md` Scenarios 1 and 3).
2. **Authenticated site-specific bootstrap material** — the expected Server TLS fingerprint (and enrollment context, where applicable) has been cryptographically authenticated through that trusted chain (see "Cryptographic binding" below) — this is the part Secure Boot alone does not provide.

**Ownership:** the Boot Adapter observes/produces the raw evidence for both facts (per the concrete mechanism in this Specification); the Application-level Boot Orchestration responsibility is the boundary that composes them into the single exposed fact `trusted_bootstrap: Established | NotEstablished`, consumed by:

- Endpoint identity precondition 7 (`docs/specifications/m0-endpoint-identity-lifecycle.md`);
- the Agent's own pre-connection gate (see "Agent bootstrap sequence" below).

No third state is introduced. `Established` and `NotEstablished` are the only two values — consistent with not inventing states beyond what evidence requires.

**Scope: boot-session-scoped, not connection-scoped or time-scoped.** The fact is established once per boot cycle (once WinPE, or eventually a running-Windows Agent context, has completed the sequence in "Agent bootstrap sequence" below) and remains valid for the entire duration of that boot session:

- **Agent Protocol reconnect within the same boot session** (a dropped/re-established WSS connection without a reboot) does **not** require re-establishing trusted bootstrap — the fact is a property of the boot session, not of any individual WebSocket connection, and reconnect already re-authenticates the credential independently (`docs/specifications/m0-agent-protocol-contract.md`).
- **A genuine reboot/power-cycle** starts a new boot session; the fact does not carry over and must be freshly established by a new run of the "Agent bootstrap sequence."
- No in-session expiry timer is defined — this is deliberately boot-scoped, not TTL-scoped, distinguishing it from the independently-cycling credential dimension (`docs/specifications/m0-endpoint-identity-lifecycle.md` "Credential/session lifecycle").

**Independence from credential validity is preserved exactly as ADR-0010/precondition 7 already require**: `CredentialActive` proves the Agent authenticated successfully over the current session; it does not prove the boot path leading to that session was itself trusted. This Specification does not change that relationship — it only defines what produces the trusted-bootstrap fact those documents already consume.

## 2. Bootstrap material

The minimum site-specific bootstrap material required by M0:

- **Expected Server TLS certificate fingerprint** — the value the Agent compares the Server's presented certificate against (`docs/specifications/m0-agent-protocol-contract.md` "Transport and handshake"). Always required.
- **Enrollment/bootstrap context** — required only if the future pre-authorized enrollment capability (`docs/specifications/m0-endpoint-identity-lifecycle.md` "Future capability: pre-authorized enrollment") is in use. **Not required for M0's default operator-approval-gated enrollment path**, which needs no pre-issued context — the endpoint simply connects and requests enrollment, and an operator approves afterward. This contract's material model accommodates the future capability without mandating it now.
- **Format/version identifier** — so the Agent (and any verifying trusted stage) can recognize the material's schema, consistent with the versioning convention already established for Agent Protocol v1 and Administrative API v1.
- **Issuance metadata** (`issued_at` or equivalent) — required to support staleness detection (see "Rotation, revocation, and recovery" below). Exact field name/format is implementation-time.

No other configuration is added merely because a bootstrap object exists — no Server hostname/network configuration (a PXE/DHCP concern, not a trust concern), no general Agent configuration.

The digest/hash algorithm used to represent the fingerprint itself is **not selected here** — consistent with `docs/decisions/0008-data-plane-transport-chunking-and-resumability.md` point 3's already-deferred `digest_algorithm` selection, this Specification does not choose a new one independently.

## 3. Cryptographic binding

**Requirement:** the mechanism must prevent an attacker who can alter unauthenticated network bootstrap content from substituting *both* the Server destination/material *and* the expected fingerprint together — the two must be bound as one atomically-authenticated unit, never delivered/trusted as independent facts an attacker could mix and match. Controlled-LAN network position is explicitly not treated as a trust anchor (ADR-0010 "Alternatives considered").

**Alternatives evaluated:**

- **Embed the fingerprint directly in a signed Agent/loader executable at build time.** Rejected: the fingerprint is installation-specific (every Bamep Server has a different certificate), so this would require building and signing a unique Agent/loader binary per site — contradicting the already-accepted single-release-artifact, no-lockstep-per-site-build packaging model (`docs/specifications/m0-stack-and-boundaries-baseline.md` "Packaging and versioning baseline").
- **TPM-sealed bootstrap material.** Rejected for M0: no TPM requirement has been evidenced or established anywhere in the accepted M0 architecture; introducing one would require its own Technical Spike, not assumed here.
- **Unsigned bootstrap data trusted by network position (PXE/DHCP-delivered, unauthenticated).** Rejected — this is the exact alternative ADR-0010 already rejected ("Hard-code or deliver the Server fingerprint as unsigned PXE/configuration data").
- **A small, per-site signed bootstrap manifest, verified by a trusted executable stage before the Agent uses it — RECOMMENDED (Proposed, not yet Accepted; see "Open questions").** A signed data artifact (the bootstrap manifest, containing the material in Section 2) is authenticated by a trusted executable stage that is itself part of the Secure-Boot-verified chain, using a **per-site** trust anchor (see "Trust-anchor and key ownership model" below) distinct from Microsoft's UEFI Secure Boot keys. This satisfies the atomic-binding requirement (the manifest's signature covers the fingerprint and any enrollment context together as one unit) without requiring a per-site Agent binary build.

**Recommended concrete design:** reuse the shim/MOK (Machine Owner Key) pattern already empirically demonstrated as viable in the authorized virtualized environment (`docs/reference/secure-boot-hardened-chain-spike.md` Scenario 3 — a Microsoft-trusted shim chaining to a distribution-signed second stage). Each site operator enrolls their own MOK via shim's standard MOK Manager mechanism (`mmx64.efi`, present but not exercised in Scenario 3), and signs a small per-site bootstrap stage with that key. That stage — now itself part of the executable trust chain via shim's MOK verification — reads and verifies the signed bootstrap manifest before the Agent uses its contents. This reuses standard, already-existing Secure Boot infrastructure rather than inventing a Bamep-specific PKI, satisfying the constraint against building a general-purpose enterprise PKI.

**This mechanism selection is flagged `Proposed`, not `Accepted`, in this round** — it is a genuine architectural decision with meaningful alternatives and real security consequences, and per this session's established practice such forks are surfaced for explicit owner confirmation rather than silently decided. The empirical basis (Scenario 3) validates that shim+MOK executes successfully under Secure Boot; it does **not** validate the additional elements this design adds (operator MOK-enrollment workflow, a per-site bootstrap stage, manifest verification logic) — those remain unvalidated by any current evidence and would need further validation (plausibly a follow-up Technical Spike or Integration Environment work) before being considered production-proven.

## 4. Trust-anchor and key ownership model

- **The trust anchor is a per-site key, not a shared or Bamep-operated one.** Each Bamep Server installation generates its own self-signed bootstrap-signing keypair (a MOK, under the recommended design) during Server/site setup — a one-time, local, operator-controlled action. No external CA and no Bamep-operated signing service is required or introduced.
- **Where trust in that key originates:** the operator's own physical control over their site's boot-media/PXE preparation is the actual root of trust — the same control the operator already exercises over their controlled LAN's PXE/DHCP infrastructure (`docs/discovery/architecture-redesign.md` "Product boundary"). Enrolling the site's MOK via shim's operator-authenticated MOK Manager workflow is what extends that physical control into the Secure-Boot-verified executable chain.
- **Server/site ownership responsibility:** the site operator owns generation, enrollment, safekeeping, and rotation of their own bootstrap-signing key. A compromise of one site's key must not affect any other site's trust — this is a direct consequence of the per-site model, not a shared root.
- **Static vs. installation-specific:** the *mechanism* (shim/MOK verification, manifest format) is static across all Bamep installations, shipped as part of the standard Bamep release; the *key material itself* is installation-specific, generated per site, never shared or distributed by Bamep.
- **Bamep does not need its own signing key** under this design — no Bamep-operated Certificate Authority or code-signing service is introduced. This is separate from, and must not be conflated with, Microsoft's UEFI Secure Boot keys (KEK/db), which continue to authenticate the underlying WinPE/Windows/shim executables exactly as established in ADR-0010; the per-site MOK authenticates only the additional site-specific bootstrap stage layered on top.

## 5. Rotation, revocation, and recovery

- **Legitimate Server TLS certificate/fingerprint rotation.** The Server issues a new signed bootstrap manifest (same still-valid per-site key, new fingerprint value and `issued_at`); this is a lightweight, frequent-as-needed operation that does **not** require touching the per-site signing key. Deployed boot media must be refreshed with the new manifest before the old certificate is retired — an overlap/transition period is operationally required, but its exact duration is implementation-time, not decided here.
- **Bootstrap-signing (MOK) key rotation.** A rarer, heavier operation: the operator enrolls a new MOK (an operator-authenticated, physically-present action per shim's own design) and re-signs/redistributes the per-site bootstrap stage. Shim supports multiple simultaneously-enrolled MOKs, so old and new keys may coexist during a transition window — this Specification requires that such a transition be *possible* by design; it does not mandate an exact duration.
- **Compromised/revoked bootstrap material or key.** The operator revokes the compromised MOK via shim's revocation mechanism and issues a new key and manifest. Any endpoint still presenting material signed by a revoked/untrusted key must fail closed (see "Failure semantics").
- **Stale material.** Staleness is detectable via the manifest's `issued_at`/version metadata (Section 2); the Server may compare presented material's version against its own current expectation at Agent Protocol handshake time and reject a stale session. The exact staleness threshold is implementation-time.
- **Material signed by an unknown/untrusted key, missing material, or corrupted material.** All fail closed uniformly — see "Failure semantics" below. None fall back to an unsigned or default value.
- **No permanent operational trap.** Because routine fingerprint rotation only requires manifest reissuance (not key rotation), and because key rotation itself supports an overlap window, legitimate Server certificate rotation does not, by design, render already-enrolled endpoints unrecoverable — operators retain the ability to refresh boot media before old material's validity is needed. Exact operational tooling/workflow for performing these refreshes is implementation-time, not designed here.

## 6. Agent bootstrap sequence

The ordering before Agent Protocol authentication, elaborating the already-accepted handshake (`docs/specifications/m0-agent-protocol-contract.md` "Transport and handshake") with the steps that precede it:

```text
1. Firmware Secure Boot verifies the executable boot chain (ADR-0010): firmware
   → (Microsoft-trusted shim →) per-site trusted bootstrap stage → WinPE/Windows
   → Agent process launch.
2. The trusted bootstrap stage (or the Agent itself, at startup) locates the
   signed bootstrap manifest from local boot media — never fetched over an
   unauthenticated network channel.
3. The manifest's signature is verified against the site's enrolled MOK /
   trust-anchor public key.
     - Verification failure (missing, corrupted, untrusted-key-signed, or
       unparseable material) → trusted bootstrap is NOT established → go to
       "Failure semantics"; the sequence does not proceed to step 4.
4. On successful verification: the trusted-bootstrap fact becomes `Established`
   for this boot session; the authenticated expected Server fingerprint (and any
   enrollment/bootstrap context) becomes available to the Agent.
5. The Agent opens a WSS connection to the Server.
6. The Agent verifies the Server's presented TLS certificate fingerprint against
   the authenticated expected fingerprint from step 4 — unchanged from
   `m0-agent-protocol-contract.md`: mismatch aborts the connection immediately,
   no Agent Protocol message exchanged, no trust-on-first-use fallback.
7. On fingerprint match: Agent Protocol authentication begins
   (`AuthRequest`/`SessionEstablished`/`AuthError`), entirely unchanged from the
   already-accepted Agent Protocol v1 contract.
```

Steps 1–4 are newly defined by this Specification. Steps 5–7 restate the already-accepted handshake unchanged — no Agent Protocol v1 message semantics are altered.

## 7. Failure semantics

- **Trusted bootstrap cannot be established** (for any reason enumerated in Section 5's failure list): the Agent must not proceed to step 5 expecting to trust any fingerprint. If a connection is attempted at all (e.g., for diagnostic purposes only), the Agent must not treat any received Server certificate as verified and must not proceed to Agent Protocol authentication. This is a fail-closed terminal state for the current boot session — no automatic retry under a different trust assumption, no fallback to unverified acceptance, no TOFU (ADR-0010 point 9, unchanged).
- **Destructive-operation gating.** This failure state directly and solely determines destructive-operation precondition 7 (`docs/specifications/m0-endpoint-identity-lifecycle.md`): an Endpoint whose current session never achieved `trusted bootstrap established` can never satisfy precondition 7, blocking destructive dispatch. This Specification supplies the fact those already-approved documents consume; it does not redefine their gating logic.
- **TLS fingerprint mismatch at the Agent Protocol layer** (step 6) remains exactly as already defined in `docs/specifications/m0-agent-protocol-contract.md` — a connection-level abort, never an `AuthError`. This Specification changes only what the "expected fingerprint" being compared against is authenticated by; it does not change that comparison's own already-accepted behavior.

## 8. Simulator contract

Consistent with the already-accepted Simulator fidelity boundary (`docs/specifications/m0-simulator-contract-and-validation-strategy.md`): the Simulated Agent uses the real Agent Protocol v1 WSS transport end-to-end; only the production boot chain (including this contract's boot-stage mechanics) is faked.

**Fixture semantics owned by this Specification** (the concrete fixture file/schema/token format remains implementation-time, owned by future Simulator implementation work):

- A fixture representing `trusted bootstrap established = Established` must carry a genuinely valid, authenticated expected Server fingerprint that matches the Simulator's own test Server instance's real TLS certificate — since the Simulated Agent uses the real WSS transport, step 6's real fingerprint comparison must be able to genuinely succeed end-to-end against real material, not a dummy value.
- A fixture representing `trusted bootstrap established = NotEstablished` exercises the required negative scenario already specified in `docs/specifications/m0-simulator-contract-and-validation-strategy.md` ("Required trusted-bootstrap independence scenario"): all other six preconditions hold, this one does not, and destructive dispatch must never occur.
- Additional fixture variants for stale and untrusted-key-signed material (exercising Section 5's failure modes at the Simulator layer) are required; whether the test harness exercises real cryptographic verification code against a deliberately invalid fixture, or short-circuits to the equivalent negative outcome, is an implementation choice (`docs/development/testing.md` general preference favors exercising real code paths where practical, but this is not mandated here).
- The Simulator is **not** required to emulate firmware, Secure Boot, shim, MOK enrollment, GRUB, or iPXE mechanics — these remain Integration Environment concerns, unchanged from the already-accepted Simulator fidelity boundary.

## 9. Validation expectations

Per `docs/development/testing.md` "Unit and domain tests": bootstrap-manifest parsing/schema validation as pure domain/contract logic, decoupled from real network transfer (analogous to chunk-manifest verification, `docs/specifications/m0-data-plane-and-storage-contracts.md`); precondition-7 consumption tests already specified in `m0-job-lifecycle-and-scheduling.md` and `m0-endpoint-identity-lifecycle.md` are confirmed aligned, not redefined here.

Per `docs/development/testing.md` "Contract tests": bootstrap-manifest signature verification logic — valid signature accepted; invalid/corrupted signature rejected; unknown/untrusted-key-signed material rejected; missing material handled explicitly; stale material (by `issued_at`/version) detected.

Per `docs/development/testing.md` "Data transfer and artifact tests" (by analogy) and general negative-case practice: Agent-side fail-closed verification — an Agent that fails to establish trusted bootstrap must be tested to confirm it never opens a trusting WSS connection and never proceeds to Agent Protocol authentication.

Per `docs/development/testing.md` "Simulator": the required trusted-bootstrap independence scenario (already specified) plus the stale/untrusted-material scenarios this Specification adds (Section 8).

Per `docs/development/testing.md` "Integration Environment": real Secure-Boot-backed production chain validation — real shim/MOK enrollment, real per-site key generation and signing, real manifest delivery via real boot media, real rotation/revocation workflow — is explicitly deferred, not covered by any automated layer, consistent with Issue #10's own established Integration Environment boundary.

Per "Local development environments," domain/contract tests are expected to run in the Linux reference environment (WSL2 or containers from Windows).

Manual: owner approval of this Specification, including explicit confirmation of the Section 3 cryptographic-binding mechanism (currently `Proposed`).

## Architectural constraints (restated, unchanged)

- ADR-0010 remains authoritative; not reopened.
- ADR-0005 remains authoritative; WSS + pinned Server TLS authentication remains the Agent control-plane architecture; not reopened.
- No TOFU; no acceptance of an unverified Server certificate, under any circumstance.
- `CredentialActive` does not imply trusted bootstrap, and trusted bootstrap does not imply `CredentialActive` — the two remain independent facts.
- Secure Boot mechanics (variables, db/dbx, shim, GRUB, iPXE) stay behind the Boot Adapter boundary; Domain code does not inspect firmware state.
- The network-delivered WinPE mechanism (Issue #8) remains separately unresolved and is not selected by this Work Package.
- This contract does not become a general secrets, identity, or PKI platform — the per-site MOK/manifest design reuses standard Secure Boot infrastructure rather than introducing Bamep-operated key/certificate services.

## Acceptance criteria

An owner-approved Specification defines:

1. the exact semantic meaning and scope of `trusted bootstrap established` — satisfied by "Trusted-bootstrap semantic model."
2. the minimum authenticated bootstrap-material contract — satisfied by "Bootstrap material."
3. the mechanism by which the expected Server TLS fingerprint is cryptographically bound to trusted bootstrap — a mechanism is defined and recommended in "Cryptographic binding," currently `Proposed` pending explicit owner confirmation (see "Open questions").
4. trust-anchor/key ownership sufficient for independent implementation — satisfied by "Trust-anchor and key ownership model," contingent on confirmation of item 3.
5. rotation/revocation/recovery and fail-closed behavior — satisfied by "Rotation, revocation, and recovery" and "Failure semantics."
6. Agent bootstrap ordering before WSS/Agent Protocol authentication — satisfied by "Agent bootstrap sequence."
7. how destructive-operation precondition 7 obtains its authoritative fact — satisfied by "Trusted-bootstrap semantic model" and "Failure semantics."
8. Simulator fixture semantics and negative cases — satisfied by "Simulator contract."
9. contract-test and Integration Environment validation expectations — satisfied by "Validation expectations."
10. no remaining architectural decision required to implement this boundary is hidden inside a future implementation Work Package — the one genuine fork identified (Section 3's mechanism selection) is flagged explicitly for owner decision rather than assumed; no other decision in this Specification is left implicit.

## Related ADRs

No new ADR is created by this Work Package. This Specification consolidates and applies ADR-0010 without reopening it. Per Issue #13's explicit instruction, an ADR would only be warranted if a genuine durable decision not already covered by ADR-0010 were discovered and accepted — the Section 3 cryptographic-binding mechanism is exactly such a decision, but it is recorded here as `Proposed`, not `Accepted`; should the owner confirm it in a future review round, promoting it to a dedicated ADR (or accepting it directly within this Specification) remains a decision for that round, not made here.

## Related work

- Issue #13 — `[WP] Define trusted bootstrap and Server fingerprint delivery contract`.
- Issue #10 / ADR-0010 — `[Spike] Validate Secure Boot and hardened boot chain` (complete; origin of `trusted bootstrap established` and the empirical shim/MOK evidence this contract's recommended mechanism builds on).
- Issue #3 / ADR-0005 / `m0-agent-protocol-contract.md` — WSS/pinned-TLS handshake this contract feeds the authenticated expected fingerprint into; unchanged.
- Issue #2 / ADR-0004 / `m0-endpoint-identity-lifecycle.md` — destructive-operation precondition 7, whose authoritative fact this contract defines.
- Issue #4 / ADR-0006 / `m0-job-lifecycle-and-scheduling.md` — precondition-7 revalidation ordering; unchanged.
- Issue #7 / `m0-simulator-contract-and-validation-strategy.md` — Simulator fidelity boundary and fixture-ownership split this contract fills in.
- Issue #1 / `m0-stack-and-boundaries-baseline.md` — Boot Port/Adapter boundary this contract's mechanics remain behind.

## Open questions

1. **Section 3's cryptographic-binding mechanism (shim/MOK-based per-site signed bootstrap manifest) is `Proposed`, not `Accepted`.** This is the one genuine architectural fork this Work Package identified — it requires explicit owner confirmation before being treated as decided. If confirmed, whether it warrants promotion to a dedicated ADR or remains recorded directly in this Specification is a decision for that review round.
2. Exact overlap/transition duration for manifest and MOK rotation — implementation-time, not decided here.
3. Exact staleness threshold for bootstrap material `issued_at` — implementation-time, not decided here.
4. Concrete bootstrap-manifest file format/schema — implementation-time, not decided here.
5. Concrete Simulator fixture file/configuration technique — owned by future Simulator implementation work, not this Specification (the semantic fixture contract is defined in "Simulator contract" above).
6. Whether the trusted bootstrap stage's manifest verification is performed by a dedicated small pre-Agent stage or by the Agent binary itself at startup — both are consistent with this Specification's contract; the exact split is implementation-time, not decided here.

Status: Proposed - awaiting owner approval.
