# M0 — Trusted Bootstrap and Server Fingerprint Delivery Contract

Status: **Proposed - awaiting owner approval**

## Context

This Specification defines the explicit M0 contract that turns ADR-0010's `trusted bootstrap established` security property into an independently implementable Server / boot-boundary / Agent contract, executing Issue #13 (`[WP] Define trusted bootstrap and Server fingerprint delivery contract`). It closes the gap ADR-0010 deliberately left open: Secure Boot authenticates *executable* boot-chain integrity, but does not by itself authenticate the *site-specific bootstrap data* (the expected Server TLS fingerprint, and enrollment context where applicable) that Agent Protocol v1 requires before authentication (`docs/specifications/m0-agent-protocol-contract.md` "Transport and handshake").

This Specification defines the **contract** only. No production implementation is part of this Work Package.

It consumes, without redefining:

- `docs/decisions/0010-trusted-bootstrap-and-secure-boot-baseline.md` — the `trusted bootstrap established` property and the Secure Boot V1 baseline this contract builds on.
- `docs/specifications/m0-agent-protocol-contract.md` — the WSS/pinned-TLS handshake this contract feeds the expected fingerprint into; unchanged, not modified by this Work Package.
- `docs/specifications/m0-endpoint-identity-lifecycle.md` — destructive-operation precondition 7, whose authoritative fact this contract defines the origin of.
- `docs/specifications/m0-job-lifecycle-and-scheduling.md` — the revalidation ordering precondition 7 already participates in; unchanged.
- `docs/specifications/m0-simulator-contract-and-validation-strategy.md` — the Simulator fidelity boundary and fixture-ownership split this contract fills in.
- `docs/specifications/m0-stack-and-boundaries-baseline.md` — the Boot Port/Adapter boundary this contract's mechanics remain behind.
- `docs/reference/secure-boot-hardened-chain-spike.md` — the empirical evidence this contract's design is grounded in (see "Four distinct sub-problems" below for exactly what that evidence does and does not establish).

**This round of owner review found that an earlier draft collapsed several distinct architectural decisions into one proposed shim/MOK design, and overstated what Issue #10's evidence actually demonstrated.** This revision corrects the evidence characterization and separates the previously-collapsed decisions explicitly (see "Four distinct sub-problems").

## Goal

Define enough of the trusted-bootstrap and Server-fingerprint-delivery contract that it is independently implementable, without inventing a general-purpose PKI or secrets platform, and without hiding a required architectural decision inside a future implementation Work Package.

## Scope

- the semantic meaning, ownership, and scoping of `trusted bootstrap established`, distinguishing local (Agent-side) establishment from Server-side authoritative knowledge of it;
- the minimum authenticated bootstrap-material contract (Server fingerprint, enrollment context where applicable);
- **(A)** restating, not redeciding, boot executable trust (ADR-0010);
- **(B)** site trust-anchor provisioning — how an arbitrary Endpoint learns a public key that legitimately belongs to this Bamep installation;
- **(C)** authenticated/fresh bootstrap material — how that trusted key authenticates the current Server fingerprint/enrollment context;
- **(D)** Server-side bootstrap evidence — how destructive-operation precondition 7 becomes authoritatively satisfied for the Server, not merely locally enforced by the Agent;
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
- any change to Agent Protocol v1 transport, message semantics, or ADR-0005 — **this Work Package does not modify `docs/specifications/m0-agent-protocol-contract.md`**, even where it identifies a real carrying-capacity implication for that contract (see "(D) Server-side bootstrap evidence" below);
- any change to Job/JobStep/Attempt lifecycle or states;
- production provisioning;
- selecting a concrete network transport (HTTP/TFTP/PXE/etc.) for bootstrap-material delivery.

## Four distinct sub-problems (do not collapse)

Owner review requires these kept explicit and separate, rather than folded into a single "shim/MOK" answer:

- **(A) Boot executable trust** — ADR-0010 / the Secure Boot baseline. Already accepted, not reopened here.
- **(B) Site trust-anchor provisioning** — how an arbitrary Endpoint learns a public key that legitimately belongs to *this* Bamep installation, before it can trust anything signed by that key. Unresolved (see "(B) Site trust-anchor provisioning").
- **(C) Authenticated/fresh bootstrap material** — how that trusted key authenticates the current Server fingerprint/enrollment context, including freshness/replay handling. Two candidates evaluated, one recommended, both `Proposed` (see "(C) Authenticated and fresh bootstrap material").
- **(D) Server-side bootstrap evidence** — how destructive-operation precondition 7 becomes authoritatively satisfied *for the Server*, not merely locally enforced by the Agent refusing to proceed. Unresolved, with a real Agent Protocol carrying-capacity implication surfaced, not designed (see "(D) Server-side bootstrap evidence").

### Corrected characterization of Issue #10's evidence

`docs/reference/secure-boot-hardened-chain-spike.md` Scenario 3 empirically demonstrated exactly: **firmware Secure Boot → Microsoft-trusted shim → Canonical-signed GRUB**, reaching a genuine interactive `grub>` prompt. That is the full extent of what was exercised.

Scenario 3 did **not** exercise, and this Specification must not claim it validated:

- MOK (Machine Owner Key) enrollment — `mmx64.efi` (shim's MOK Manager) was present on the test disc but explicitly **not** exercised;
- a custom EFI executable signed by an operator-enrolled MOK;
- MOK rotation or removal;
- any Bamep-specific per-site bootstrap stage.

**Corrected framing, used consistently throughout this Specification**: "shim + a signed second stage" is empirically demonstrated viable in the authorized virtualized environment. A **MOK-based extension** of that chain to authenticate site-specific Bamep material remains a **technically documented candidate** (shim's MOK mechanism is standard, well-documented upstream functionality) — it is **not** Bamep-validated evidence, and the earlier draft's framing of it as a "recommended concrete design" overstated what was actually tested. This Specification demotes it to one candidate among several for sub-problem (B), evaluated on its documented properties rather than on empirical validation that does not exist yet.

## 1. Trusted-bootstrap semantic model

`trusted bootstrap established` is a fact about **the current Agent boot/session context**, produced by the Boot Adapter boundary and exposed upward through Application-level Boot Orchestration as a simple, firmware-independent assertion — never as `SecureBootEnabled`, and never inspected directly by Domain code (`docs/specifications/m0-stack-and-boundaries-baseline.md`).

**The fact requires two things to both hold, not Secure Boot (A) alone** (ADR-0010 point 7):

1. **(A) Executable boot-chain integrity** — Secure Boot, already accepted, not reopened here.
2. **(B)+(C) Authenticated site-specific bootstrap material** — the expected Server TLS fingerprint (and enrollment context, where applicable) has been cryptographically authenticated using a legitimately-provisioned trust anchor. This is the part Secure Boot alone does not provide, and the part sub-problems (B) and (C) below remain open on.

**Local establishment vs. Server-side authority are distinct and must not be conflated.** The Agent can locally determine, at boot time, whether (1) and (2) above hold for itself — this is **local establishment**, and it is sufficient to gate the Agent's own willingness to proceed (see "Failure semantics"). It is **not**, by itself, sufficient to make destructive-operation precondition 7 authoritative *for the Server* — the Server cannot observe the Agent's local boot state directly, and inferring it from connection success or `CredentialActive` is exactly what ADR-0010 forbids. Making the fact Server-observable is sub-problem (D), which remains unresolved (see "(D) Server-side bootstrap evidence").

**Ownership:** the Boot Adapter observes/produces the raw evidence; Application-level Boot Orchestration composes it into the exposed fact `trusted_bootstrap: Established | NotEstablished`, consumed by:

- Endpoint identity precondition 7 (`docs/specifications/m0-endpoint-identity-lifecycle.md`) — **at the Server**, which requires sub-problem (D) to be resolved before this consumption is actually possible in production;
- the Agent's own pre-connection gate (see "Agent bootstrap sequence" below) — **at the Agent**, which requires only local establishment.

No third state is introduced. `Established` and `NotEstablished` are the only two values.

**Scope: boot-session-scoped, not connection-scoped or time-scoped.** The fact is established once per boot cycle and remains valid for the entire duration of that boot session:

- **Agent Protocol reconnect within the same boot session** does **not** require re-establishing trusted bootstrap — it is a property of the boot session, not of any individual WebSocket connection.
- **A genuine reboot/power-cycle** starts a new boot session; the fact must be freshly established.
- No in-session expiry timer is defined — deliberately boot-scoped, not TTL-scoped, distinguishing it from the independently-cycling credential dimension.

**Independence from credential validity is preserved exactly as ADR-0010/precondition 7 already require**: `CredentialActive` proves the Agent authenticated successfully over the current session; it does not prove the boot path leading to that session was itself trusted, and (per sub-problem (D)) does not by itself prove trusted bootstrap to the Server either.

## 2. Bootstrap material

The minimum site-specific bootstrap material required by M0:

- **Expected Server TLS certificate fingerprint** — always required.
- **Enrollment/bootstrap context** — required only if the future pre-authorized enrollment capability is in use; **not required for M0's default operator-approval-gated enrollment path**.
- **Format/version identifier (schema version)** — so the verifying party can recognize the material's schema.
- **Signing-key identifier / verification metadata** — required so the verifying party knows which trust-anchor key the material claims to be signed by, distinct from *whether* that key is actually trusted (sub-problem (B)).

**Freshness is not solved by a static issuance timestamp alone** (see "(C) Authenticated and fresh bootstrap material" below for why, and for the recommended mechanism). Any `issued_at`-style field, if retained, is auxiliary metadata only — it is not, by itself, this Specification's freshness mechanism.

No other configuration is added merely because a bootstrap object exists. The digest/hash algorithm used to represent the fingerprint itself is **not selected here**, consistent with ADR-0008 point 3's already-deferred `digest_algorithm` selection.

## (B) Site trust-anchor provisioning

**How does an arbitrary Endpoint learn a public key that legitimately belongs to this specific Bamep installation, before it can trust anything signed by that key?** This is left **unresolved pending explicit owner review** — no candidate is selected in this round.

**Candidates evaluated, none selected:**

- **Per-site MOK enrolled on every Endpoint.** A site operator generates a bootstrap-signing keypair and enrolls it as a Machine Owner Key via shim's standard MOK Manager mechanism. **Important operational consequence, previously understated**: MOK enrollment is **machine-local**, not a one-time Server/site action. Using a site MOK as the trust anchor for a custom Bamep EFI/bootstrap stage means establishing that trust **on each Endpoint** that must execute the stage — this may involve local/console-assisted enrollment and a reboot, depending on the exact shim/MokManager workflow used (e.g., `mokutil --import` still requires one manual confirmation at next boot in standard shim deployments). The owner has **not** accepted this per-Endpoint enrollment cost; it is recorded here as the real cost of this candidate, not glossed over.
- **Direct firmware db/PK enrollment of the site's key on every Endpoint**, bypassing shim/MOK. Same fundamental per-Endpoint problem as above, and typically heavier (firmware-level key enrollment tooling/UX, not a userspace-mediated flow like MOK) — not clearly better than the MOK candidate, and not evidenced as available/scriptable in this environment.
- **Trust the key delivered via the same Microsoft-signed executable chain that Secure Boot already validates.** Rejected as infeasible for M0: this would require Bamep binaries to be Microsoft-signed (via Microsoft's own signing program), and no such relationship is evidenced or assumed anywhere in the accepted M0 architecture.
- **Operator-approval-gated first-key trust, analogous to the already-accepted Endpoint-enrollment model** (`docs/specifications/m0-endpoint-identity-lifecycle.md`: `PendingEnrollment` → explicit operator approval → `Enrolled`). A candidate site key could be recorded on first observation and require explicit operator approval before being trusted, mirroring how Endpoint identity itself is already handled, potentially avoiding per-Endpoint console/reboot cost. **This is materially weaker than pre-established trust** for the specific purpose this contract exists to serve: the entire point of authenticating the fingerprint *before* Agent Protocol contact is to protect against a rogue/malicious Server at first contact, and an approval step that happens *after* first observation reintroduces a window structurally similar to trust-on-first-use for the trust anchor itself, even though it would not weaken the already-decided no-TOFU rule for the Server TLS fingerprint comparison narrowly. This trade-off is recorded, not resolved.

None of these candidates is recommended over the others in this round — this is left for explicit owner decision (see "Open questions" and "Technical Spike recommendation").

## (C) Authenticated and fresh bootstrap material

**Requirement (unchanged from the prior round):** the mechanism must prevent an attacker who can alter unauthenticated content from substituting *both* the Server destination/material *and* the expected fingerprint together — the two must be bound as one atomically-authenticated unit.

**Transport independence (corrected from the prior round).** The prior draft required bootstrap material to come from "local boot media — never fetched over an unauthenticated network channel." That transport-level requirement is removed. The security property this contract needs is **transport-independent**: material MAY eventually be delivered through a transport that is not itself trusted (including, in the future, a provisioning network), **provided**:

- authenticity/integrity is independently verified against an already-trusted anchor (sub-problem (B)), regardless of the channel it arrived on;
- substitution (of either the material or the transport) fails closed;
- replay/freshness is addressed explicitly (this is exactly where the two candidates below differ).

This Work Package does **not** select HTTP, TFTP, PXE, or any other concrete network transport. Issue #8's network-delivery uncertainty remains independently unresolved and is not affected by this decision.

**Two candidates evaluated:**

### Candidate A: static signed manifest

A signed artifact containing the expected fingerprint (and other material from Section 2), signed once by the trust-anchor key, staged wherever the deployment chooses (local media or, per the transport-independence correction above, potentially network-delivered).

**Genuine, unresolved problem with this candidate: replay/freshness.** A validly-signed static manifest, once created, remains validly-signed indefinitely as far as signature verification alone is concerned. Nothing in signature verification distinguishes "the current, intended manifest" from "an old, superseded-but-still-validly-signed manifest" unless an additional trusted mechanism is layered on top — and every such mechanism has a real cost:

- a trusted wall-clock at the boot stage is not guaranteed available or trustworthy pre-OS;
- a persisted "last-seen version" state is itself tamperable/resettable and adds its own trust requirement;
- checking the current expected version against the Server cannot happen before pinned TLS Server authentication succeeds — Agent Protocol authentication occurs only *after* pinned TLS Server authentication (`docs/specifications/m0-agent-protocol-contract.md`), so using the Server as the freshness oracle at this stage would introduce circular ordering, which this Specification does not do.

**This candidate is recorded as viable only if this freshness gap is either accepted as a residual risk (with an explicit, separately-designed revocation/staleness mitigation) or closed by an additional mechanism not designed here.** It is not rejected outright, but it is not recommended (see below).

### Candidate B: nonce-bound signed bootstrap assertion (recommended, still `Proposed`)

A challenge/response model:

1. the trusted bootstrap stage generates a cryptographically random `boot_nonce` locally — no wall-clock trust and no network dependency required for this step alone;
2. it obtains site-specific bootstrap material from whichever party holds the trust-anchor private key (in practice, most plausibly the Server itself, or a local provisioning service acting on the Server's behalf) — this step **requires a live, per-boot exchange**, not a purely offline pre-staged artifact, since nobody could have pre-signed a response containing a nonce that does not exist until this specific boot generates it;
3. the signed response covers, as one signed unit: schema/contract version; the exact `boot_nonce`; the expected Server TLS fingerprint; enrollment/bootstrap context when applicable; and the signing-key identifier;
4. the trusted stage verifies the signature **and** the exact nonce match — a response bound to a different nonce (e.g., a captured/replayed old response) is rejected;
5. only then is the fingerprint accepted for WSS pinning.

**Why this resolves the ordering concern without circularity:** verification of the signed response is performed **locally**, using the already-provisioned trust-anchor public key (sub-problem (B)) — it does not depend on TLS-authenticating the remote party first. The challenge/response exchange itself can therefore occur over an untrusted channel (its own security comes from the signature and nonce binding, not from transport security), entirely *before* any WSS/TLS pinning is attempted. This is consistent with, and does not reopen, the already-accepted ordering in `docs/specifications/m0-agent-protocol-contract.md`.

**Recommendation:** Candidate B is recommended over Candidate A because it closes the replay/freshness gap structurally (via the nonce) rather than relying on an additional, separately-trusted staleness mechanism. This recommendation is **not** an acceptance — per owner instruction, the final architectural choice between A and B remains `Proposed`, pending explicit owner review (see "Open questions").

No concrete signature algorithm or encoding is selected by either candidate; that remains implementation-time unless a specific choice proves necessary to make the contract unambiguous, which has not been found to be the case here.

## (D) Server-side bootstrap evidence

**How does the Server obtain the authoritative current-session fact needed to evaluate `trusted bootstrap established` for destructive-operation precondition 7?** Local establishment (Section 1) is not sufficient by itself — this sub-problem is genuinely unresolved and is **not** designed by this Work Package. It is surfaced explicitly, including a real protocol-carrying-capacity implication, rather than assumed away.

**Options evaluated, distinguishing local enforcement from Server-observable evidence:**

- **Local enforcement only (Agent refuses to connect/authenticate unless its own bootstrap gate passed).** This is already required regardless of any other choice (see "Failure semantics") — but **local enforcement alone does not make the fact Server-observable**. If the Server has no independent evidence, the only thing it could infer is "an Agent Protocol session was established with a valid credential," which is exactly the `CredentialActive`-implies-trusted-bootstrap conflation ADR-0010 explicitly forbids. Local enforcement is necessary but **not sufficient** on its own.
- **Trusted Agent self-reports the boot-context result as part of session establishment.** The Agent asserts "my local bootstrap gate passed" as part of the handshake. Weaker form of Server-observable evidence: the Server trusts the Agent's own assertion rather than independently verifying cryptographic proof. Given the Agent binary itself runs inside the already-Secure-Boot-verified chain (A), a compromised/substituted Agent binary should not be able to run at all — which gives this option more credibility than a bare unverified claim, but it still stops short of independent proof.
- **A bootstrap proof/token/assertion is presented to the Server and independently verified there.** The Agent forwards the *signed* bootstrap assertion itself (from sub-problem (C), whichever candidate is eventually accepted) to the Server, which independently verifies its signature — using the same trust-anchor key the Server itself holds or knows, since the Server is plausibly the party that issued it in Candidate B. This gives the Server genuine independent cryptographic proof, not the Agent's word alone — the stronger form of Server-observable evidence.
- **Hardware-backed remote attestation.** Not introduced as a requirement — no TPM requirement or evidence exists anywhere in the accepted M0 architecture, and none is assumed here. Mentioned only as a theoretical possibility explicitly not pursued.

**Real, honestly-surfaced blocker: Agent Protocol v1 currently has no carrying capacity for any of this.** `AuthRequest{credential}` (`docs/specifications/m0-agent-protocol-contract.md`) carries only the credential — it has no field for a bootstrap self-report or a forwarded signed assertion, regardless of which evidence-strength option above is eventually chosen. Adding such a field (or a new message) would be a genuine Agent Protocol v1 contract change. **This Work Package does not make that change** — `m0-agent-protocol-contract.md` is not modified here, consistent with scope — but this gap is recorded as a real architectural fork requiring resolution before production implementation, not papered over.

## 5. Rotation, revocation, and recovery

- **Legitimate Server TLS certificate/fingerprint rotation.** Under Candidate B (nonce-bound assertions, recommended): routine rotation is achieved by having the signer (the party holding the trust-anchor private key — plausibly the Server itself) issue newly signed assertions containing the new fingerprint in response to future boot challenges, **without rotating the site trust-anchor key at all**. No boot-media refresh is required for routine fingerprint rotation under this model — this directly satisfies the requirement that legitimate rotation remain operationally viable and not require assuming boot media must always be manually refreshed. Under Candidate A (static manifest), routine rotation would still require reissuing and redistributing the manifest, as previously noted, without resolving its underlying freshness gap.
- **Signing-key (trust-anchor) rotation stays fully distinct from Server-certificate rotation**, under either candidate. It is a rarer, heavier operation, whose exact mechanics depend on the sub-problem (B) decision (e.g., MOK re-enrollment, if that candidate is chosen) — not designed further here since (B) itself is unresolved.
- **Compromised/revoked bootstrap material or key.** Fails closed (see "Failure semantics"); revocation mechanics depend on the (B) decision.
- **Stale material.** Under Candidate B, freshness is structural (the nonce), not a separate staleness policy. Under Candidate A, staleness detection would need a separately-designed mechanism not resolved by this Specification.
- **No silent TOFU or multiple simultaneously-accepted unverified fingerprints are introduced by rotation in either candidate** — exactly one authenticated fingerprint is accepted per successful bootstrap sequence.

## 6. Agent bootstrap sequence

```text
1. Firmware Secure Boot verifies the executable boot chain (A, ADR-0010) up
   through the trusted bootstrap stage and the Agent process launch.
2. The trusted bootstrap stage obtains site-specific bootstrap material,
   authenticated per whichever (C) candidate is eventually accepted — under
   Candidate B, this includes generating a `boot_nonce` and completing a
   live challenge/response exchange over a transport not itself required to
   be trusted (see "(C) Authenticated and fresh bootstrap material").
3. The material's signature (and, under Candidate B, exact nonce binding) is
   verified against the site trust-anchor public key provisioned per
   whichever (B) candidate is eventually accepted.
     - Verification failure (missing, corrupted, untrusted-key-signed,
       unparseable, or nonce-mismatched material) → trusted bootstrap is NOT
       established locally → go to "Failure semantics"; the sequence does
       not proceed to step 4.
4. On successful local verification: the trusted-bootstrap fact becomes
   `Established` for this boot session, locally, at the Agent. Whether and
   how this becomes authoritative *to the Server* is sub-problem (D), not
   resolved by this step.
5. The Agent opens a WSS connection to the Server.
6. The Agent verifies the Server's presented TLS certificate fingerprint
   against the authenticated expected fingerprint from step 4 — unchanged
   from `m0-agent-protocol-contract.md`: mismatch aborts the connection
   immediately, no Agent Protocol message exchanged, no trust-on-first-use.
7. On fingerprint match: Agent Protocol authentication begins, entirely
   unchanged from the already-accepted Agent Protocol v1 contract — subject
   to the (D) carrying-capacity gap noted above if Server-side evidence is
   eventually required at this point.
```

Steps 1–4 are newly defined by this Specification, with sub-problems (B), (C), and (D) left open within them as noted. Steps 5–7 restate the already-accepted handshake unchanged — no Agent Protocol v1 message semantics are altered by this Work Package.

## 7. Failure semantics

- **Trusted bootstrap cannot be established locally** (any reason in Section 6 step 3): the Agent must not proceed to step 5 expecting to trust any fingerprint, must not treat any received Server certificate as verified if it connects anyway, and must not proceed to Agent Protocol authentication. Fail-closed, no automatic retry under a different trust assumption, no fallback, no TOFU (ADR-0010 point 9, unchanged).
- **Destructive-operation gating remains conditional on resolving (D).** Local Agent-side failure already blocks the Agent's own willingness to proceed. Server-side gating of destructive-operation precondition 7 additionally requires sub-problem (D) to be resolved — until it is, the Server has no independent way to enforce this failure mode itself, only the Agent's own local refusal to connect meaningfully.
- **TLS fingerprint mismatch at the Agent Protocol layer** (step 6) remains exactly as already defined in `docs/specifications/m0-agent-protocol-contract.md` — unchanged by this Specification.

## 8. Simulator contract

Consistent with the already-accepted Simulator fidelity boundary (`docs/specifications/m0-simulator-contract-and-validation-strategy.md`): the Simulated Agent uses the real Agent Protocol v1 WSS transport end-to-end; only the production boot chain (including this contract's boot-stage mechanics) is faked.

**Fixture semantics owned by this Specification** (the concrete fixture file/schema/token format remains implementation-time):

- A fixture representing `trusted bootstrap established = Established` must carry a genuinely valid, authenticated expected Server fingerprint matching the Simulator's own test Server instance's real TLS certificate, since the Simulated Agent uses the real WSS transport and step 6's real fingerprint comparison must genuinely succeed end-to-end.
- A fixture representing `trusted bootstrap established = NotEstablished` exercises the required negative scenario already specified in `docs/specifications/m0-simulator-contract-and-validation-strategy.md`.
- Additional fixture variants for stale/replayed and untrusted-key-signed material are required, exercising whichever (C) candidate is eventually accepted.
- The Simulator is **not** required to emulate firmware, Secure Boot, shim, MOK enrollment, GRUB, or iPXE mechanics.
- **The Simulator fixture necessarily represents only local establishment (Section 1) unless and until sub-problem (D) is resolved** — a Simulated Agent's local fixture state does not, by itself, demonstrate how the Server would independently know that fact in production; this limitation is inherited from (D) being open, not introduced by the Simulator contract itself.

## 9. Validation expectations

Per `docs/development/testing.md` "Unit and domain tests": bootstrap-material parsing/schema validation as pure domain/contract logic; precondition-7 consumption tests already specified in `m0-job-lifecycle-and-scheduling.md` and `m0-endpoint-identity-lifecycle.md` are confirmed aligned, not redefined here.

Per `docs/development/testing.md` "Contract tests": bootstrap-material signature verification logic — valid signature accepted; invalid/corrupted signature rejected; unknown/untrusted-key-signed material rejected; missing material handled explicitly; under Candidate B, nonce-mismatch (replay) rejected explicitly.

Per general negative-case practice: Agent-side fail-closed verification — an Agent that fails to establish trusted bootstrap locally must never open a trusting WSS connection or proceed to Agent Protocol authentication.

Per `docs/development/testing.md` "Simulator": the required trusted-bootstrap independence scenario (already specified) plus stale/replayed and untrusted-material scenarios per Section 8.

Per `docs/development/testing.md` "Integration Environment": real Secure-Boot-backed production chain validation — real trust-anchor provisioning (whichever (B) candidate is accepted), real material authentication (whichever (C) candidate is accepted), real Server-side evidence handling (whichever (D) option is accepted) — is explicitly deferred, not covered by any automated layer.

Per "Local development environments," domain/contract tests are expected to run in the Linux reference environment (WSL2 or containers from Windows).

Manual: owner approval of this Specification, including explicit resolution of sub-problems (B), (C), and (D) — none of which is approved in this round.

## Architectural constraints (restated, unchanged)

- ADR-0010 remains authoritative; not reopened.
- ADR-0005 remains authoritative; WSS + pinned Server TLS authentication remains the Agent control-plane architecture; not reopened.
- No TOFU; no acceptance of an unverified Server certificate, under any circumstance.
- `CredentialActive` does not imply trusted bootstrap, locally or to the Server; trusted bootstrap does not imply `CredentialActive`.
- Secure Boot mechanics stay behind the Boot Adapter boundary; Domain code does not inspect firmware state.
- The network-delivered WinPE mechanism (Issue #8) remains separately unresolved and is not selected by this Work Package.
- The seven destructive-operation preconditions are unchanged.
- This contract does not become a general secrets, identity, or PKI platform.

## Acceptance criteria

An owner-approved Specification defines:

1. the exact semantic meaning and scope of `trusted bootstrap established`, including the local-vs-Server-authoritative distinction — satisfied by "Trusted-bootstrap semantic model."
2. the minimum authenticated bootstrap-material contract — satisfied by "Bootstrap material."
3. the mechanism by which the expected Server TLS fingerprint is cryptographically bound to trusted bootstrap — two candidates evaluated and one recommended in "(C) Authenticated and fresh bootstrap material," both `Proposed` pending explicit owner confirmation.
4. trust-anchor/key ownership sufficient for independent implementation — **not yet satisfied**; sub-problem (B) is left explicitly unresolved pending owner review (see "Open questions").
5. rotation/revocation/recovery and fail-closed behavior — satisfied by "Rotation, revocation, and recovery" and "Failure semantics," contingent on (B)/(C) resolution.
6. Agent bootstrap ordering before WSS/Agent Protocol authentication — satisfied by "Agent bootstrap sequence."
7. how destructive-operation precondition 7 obtains its authoritative fact — **not yet satisfied**; sub-problem (D) is left explicitly unresolved, with a real Agent Protocol carrying-capacity implication surfaced but not designed (see "(D) Server-side bootstrap evidence").
8. Simulator fixture semantics and negative cases — satisfied by "Simulator contract," with the (D)-dependent limitation noted.
9. contract-test and Integration Environment validation expectations — satisfied by "Validation expectations."
10. no remaining architectural decision required to implement this boundary is hidden inside a future implementation Work Package — three genuine forks are identified (B, C, D) and explicitly flagged for owner decision rather than assumed; none is hidden.

## Related ADRs

No new ADR is created by this Work Package. Per Issue #13's explicit instruction, an ADR would only be warranted for a genuine durable decision not already covered by ADR-0010, accepted with confidence — sub-problems (B), (C), and (D) are exactly such candidate decisions, but all three remain `Proposed`/unresolved in this round; whether any is promoted to a dedicated ADR (or resolved directly within this Specification) is a decision for a future review round, not made here.

## Related work

- Issue #13 — `[WP] Define trusted bootstrap and Server fingerprint delivery contract`.
- Issue #10 / ADR-0010 — `[Spike] Validate Secure Boot and hardened boot chain` (complete; origin of `trusted bootstrap established` and the corrected Scenario 3 evidence this contract's sub-problem (B) candidates are evaluated against).
- Issue #3 / ADR-0005 / `m0-agent-protocol-contract.md` — WSS/pinned-TLS handshake this contract feeds the authenticated expected fingerprint into; not modified by this Work Package, despite the (D) carrying-capacity implication surfaced above.
- Issue #2 / ADR-0004 / `m0-endpoint-identity-lifecycle.md` — destructive-operation precondition 7, whose authoritative fact this contract defines the origin of, pending (D).
- Issue #4 / ADR-0006 / `m0-job-lifecycle-and-scheduling.md` — precondition-7 revalidation ordering; unchanged.
- Issue #7 / `m0-simulator-contract-and-validation-strategy.md` — Simulator fidelity boundary and fixture-ownership split this contract fills in.
- Issue #1 / `m0-stack-and-boundaries-baseline.md` — Boot Port/Adapter boundary this contract's mechanics remain behind.

## Open questions

Three genuine architectural forks remain, none decided by this round:

1. **(B) Site trust-anchor provisioning mechanism.** Per-Endpoint MOK enrollment (real, previously-understated per-Endpoint console/reboot cost), direct firmware db/PK enrollment (similar or heavier cost), and operator-approval-gated first-key trust (structurally weaker, avoids per-Endpoint cost) are recorded as candidates, none selected. Requires explicit owner decision.
2. **(C) Static signed manifest vs. nonce-bound signed bootstrap assertion.** Candidate B (nonce-bound) is recommended for structurally resolving the replay/freshness gap that Candidate A (static manifest) leaves open, and for enabling routine Server-fingerprint rotation without touching the trust anchor — but the final choice remains `Proposed`, pending explicit owner review.
3. **(D) Server-observable evidence for precondition 7, and its Agent Protocol implication.** Local Agent-side enforcement alone is confirmed insufficient to make the fact Server-authoritative. Self-report vs. forwarded-signed-assertion are the two genuine options identified; both require Agent Protocol v1 carrying capacity that does not currently exist. This Work Package does not design or select an option, and does not modify `m0-agent-protocol-contract.md` — the implication is surfaced for a future decision.

Remaining implementation-time details (not architectural forks): exact overlap/transition duration for material or key rotation; concrete bootstrap-material file/wire format; concrete Simulator fixture file/configuration technique; whether manifest/assertion verification is performed by a dedicated pre-Agent stage or by the Agent binary itself at startup.

## Technical Spike recommendation

Sub-problem **(B)** plausibly warrants a dedicated Technical Spike: whether MOK enrollment (or an alternative) can be made sufficiently unattended/scriptable, and what the genuine per-Endpoint operational cost is, was not empirically exercised by Issue #10 (MokManager was present but not tested) and is not resolvable by architectural reasoning alone — it depends on observed shim/MokManager behavior this session did not produce evidence for.

Sub-problems **(C)** and **(D)** are primarily protocol/architecture design decisions, not empirical-hardware questions — no firmware or physical uncertainty remains to resolve for either; they are better suited to owner design review (and, for (D), a following Agent Protocol amendment round) than to a Technical Spike.

Whether to actually commission a Technical Spike for (B), and whether/when to open a follow-up Work Package for (C)/(D) and the resulting Agent Protocol amendment, remain owner decisions — not made or materialized by this task.

Status: Proposed - awaiting owner approval.
