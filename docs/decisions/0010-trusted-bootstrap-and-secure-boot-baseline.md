# ADR-0010: Trusted bootstrap and Secure Boot baseline

Status: Accepted

## Context

`docs/specifications/m0-agent-protocol-contract.md` ("Transport and handshake") requires the Server's certificate fingerprint to be delivered to the Agent through an authenticated, integrity-protected boot mechanism, and explicitly does not assume the current boot chain already provides that assurance — this was left dependent on the Secure Boot / hardened boot-chain Technical Spike (Issue #10).

Issue #10 (`[Spike] Validate Secure Boot and hardened boot chain`) established, in the authorized virtualized environment (`docs/reference/secure-boot-hardened-chain-spike.md`), that Secure Boot enforcement is practically viable:

- a Microsoft-signed WinPE boot path is accepted cleanly;
- unsigned/untrusted EFI applications (iPXE, self-built GRUB) are rejected fail-closed, with a distinct, unambiguous error signature (`Access Denied`);
- a Microsoft-trusted shim chaining to a distribution-signed GRUB (Ubuntu's official `shim-signed`/`grub-efi-amd64-signed` packages) is accepted end to end.

The Spike also demonstrated an important limitation: Secure Boot authenticates *executable stages* as they load — it does not by itself authenticate arbitrary site-specific bootstrap *data*, such as a Server TLS fingerprint or enrollment context, carried alongside or read by that code. Establishing trust in the executable chain and establishing trust in Bamep-specific bootstrap material are therefore related but distinct problems; an additional contract is still required to bind Server-specific bootstrap material to the trusted chain.

## Decision

1. Bamep requires authenticated trusted bootstrap for production Agent startup.
2. Secure Boot is the V1 baseline mechanism for establishing executable boot-chain integrity on UEFI x86-64.
3. The architectural invariant consumed above the Adapter boundary is **`trusted bootstrap established`** — not `Secure Boot enabled`. Higher layers (Domain, Application) depend on the invariant that the current Agent boot/session context has been anchored in an authenticity/integrity-established bootstrap, never on a firmware-specific fact.
4. Firmware/Secure Boot mechanics — Secure Boot variables, db/dbx, shim, GRUB, iPXE, or any other concrete boot-chain component — remain Boot Adapter concerns, consistent with the already-accepted Boot Port boundary (`docs/specifications/m0-stack-and-boundaries-baseline.md`). Domain code must not depend on these firmware-specific concepts directly.
5. A future alternative hardened boot mechanism may satisfy the same `trusted bootstrap established` invariant, but only after it is explicitly specified, threat-modeled, and validated as providing equivalent required security properties.
6. Until such an alternative is accepted, the production trusted-bootstrap path is based on Secure Boot.
7. Secure Boot alone does **not** authenticate arbitrary bootstrap data such as the Server TLS fingerprint, enrollment context, or other site-specific configuration. Those values must eventually be cryptographically bound to, or verified by, a trusted executable/bootstrap stage through a separate, explicit contract (see "Related work" — the dedicated trusted-bootstrap and Server-fingerprint-delivery contract remains unresolved and is not designed by this ADR).
8. Agent Protocol v1 remains WSS with pinned Server authentication (ADR-0005). ADR-0005 is not reopened by this decision.
9. No TOFU (trust-on-first-use) fallback or acceptance of an unverified Server certificate is introduced by this decision, at any layer.
10. Development, simulation, and explicitly non-production validation environments may substitute a deterministic trusted fixture for the trusted-bootstrap property, where already allowed by the Simulator Specification (`docs/specifications/m0-simulator-contract-and-validation-strategy.md`) — this does not weaken the production invariant above.
11. The exact trusted-bootstrap material format, signing hierarchy, key ownership, rotation, revocation, delivery mechanism, and production bootloader choice are **not** selected by this ADR.

## Alternatives considered

- **Secure Boot not required / rely only on controlled LAN.** Rejected: network reachability is already explicitly not a trust anchor (`AGENTS.md`; `docs/discovery/architecture-redesign.md` "Security invariants" — MAC addresses and network position are inventory signals, not authentication) and does not by itself satisfy authenticated fingerprint delivery.
- **Hard-code or deliver the Server fingerprint as unsigned PXE/configuration data.** Rejected: an attacker able to alter unsigned bootstrap delivery could substitute both the destination and the expected fingerprint together, defeating the purpose of pinning.
- **Make `SecureBootEnabled` itself a Domain invariant.** Rejected: this would couple Domain safety semantics to one concrete firmware mechanism and would prevent a future, equivalently hardened bootstrap implementation from ever satisfying the same invariant without a Domain-level change.
- **Secure Boot-backed trusted bootstrap as the current V1 baseline, exposed to higher layers only as a mechanism-independent `trusted bootstrap established` property.** Accepted, based on Issue #10's evidence, for the reasons above.

## Consequences

- Production destructive-operation safety gains an additional, independent precondition — trusted bootstrap — layered on top of the already-accepted preconditions in `docs/specifications/m0-endpoint-identity-lifecycle.md`.
- A dedicated future M0 contract is required to define what establishes and represents `trusted bootstrap established`, and how the Server TLS fingerprint / enrollment context is authenticated through it — not designed here (see "Related work"). **Resolved**: that contract was materialized as Issue #13 and is `Approved` (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md`); its site trust-anchor provisioning sub-problem was accepted separately as ADR-0011, informed by Issue #14's empirical evidence.
- No change to the Agent Protocol v1 wire contract, transport, or ADR-0005.
- Simulator-level and other non-production validation may use a deterministic trusted-bootstrap fixture instead of exercising real Secure Boot, consistent with the Simulator Specification's existing fidelity boundary.

## Related architecture

- `docs/specifications/m0-stack-and-boundaries-baseline.md` — Boot Port/Adapter boundary this decision is consumed through; amended alongside this ADR to record the architectural consequence above.
- `docs/specifications/m0-agent-protocol-contract.md` — Server-fingerprint delivery requirement this decision informs; amended alongside this ADR to remove the stale "depends on Issue #10" wording.
- `docs/specifications/m0-endpoint-identity-lifecycle.md` — destructive-operation authorization preconditions; amended alongside this ADR to add the independent trusted-bootstrap precondition.

## Related work

- Issue #10 — `[Spike] Validate Secure Boot and hardened boot chain` (this ADR's evidentiary basis).
- `docs/reference/secure-boot-hardened-chain-spike.md` — empirical findings this decision applies.
- At the time this ADR was accepted, a dedicated future M0 Work Package was required for the trusted-bootstrap and Server-fingerprint-delivery contract, not yet materialized as a GitHub Issue. Its required scope was: what constitutes `trusted bootstrap established`; the trusted-bootstrap evidence/state exposed to Application/Domain; how the Server TLS fingerprint is authenticated and integrity-protected; how enrollment/bootstrap context is authenticated where applicable; how site-specific bootstrap material is bound to a trusted executable stage; artifact/manifest signing or an equivalent mechanism, if selected; trust-anchor/key ownership; key/fingerprint rotation; revocation/recovery behavior; failure semantics; how the Agent receives and verifies the material before WSS authentication; Simulator fixture semantics versus production bootstrap; and contract-test/future Integration Environment validation expectations. **Resolved**: that Work Package was materialized as Issue #13 and answered all of the above — `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` (`Approved`). Its site trust-anchor provisioning sub-problem (how an Endpoint comes to trust the signing key) was accepted separately as ADR-0011 (operator-verified first-site-key pairing), informed by Issue #14's empirical evidence (`docs/reference/site-trust-anchor-provisioning-spike.md`).
- Issue #8 — `[Spike] Validate WinPE boot mechanism` (network-delivered boot mechanism remains separately unresolved; not affected by this decision — see `docs/specifications/m0-stack-and-boundaries-baseline.md`).

## Open questions

1. The concrete trusted-bootstrap and Server-fingerprint-delivery contract (listed above) — explicitly out of scope for this ADR, required before production implementation. **Resolved** by Issue #13 (`Approved`) and ADR-0011; see "Related work".
2. Whether/how a future alternative hardened boot mechanism would be specified and validated to satisfy the `trusted bootstrap established` invariant — not designed here.
3. Exact Simulator fixture representation of `trusted bootstrap established` — `docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` (Issue #13, now `Approved`) owns the semantic fixture contract, i.e. what production fact the fixture substitutes for (see its Section 8, "Simulator contract"); Simulator/vertical-slice implementation work later chooses only its concrete implementation/configuration technique within that contract, not the semantics themselves. The concrete implementation/configuration technique remains undecided here either way.

Status: Accepted.
