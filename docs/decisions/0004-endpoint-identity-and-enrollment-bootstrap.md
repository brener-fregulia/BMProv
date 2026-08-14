# ADR-0004: Endpoint identity and enrollment/trust bootstrap model

Status: Accepted

## Context

M0 requires resolving Endpoint identity and the enrollment/trust bootstrap (`docs/discovery/adr-triage.md` candidate 2; `docs/specifications/m0-architecture-baseline.md` scope item "Endpoint identity"; `docs/discovery/architecture-redesign.md` "Endpoint identity", "Security invariants"; `AGENTS.md` Safety section). Issue #2 executes this Work Package.

Endpoint identity must survive NIC or MAC replacement (architecture-redesign.md "Endpoint identity"). Architecture-redesign.md proposes a direction "to evaluate through ADR": a Boot Orchestrator issues a short-lived enrollment context/credential, the Agent authenticates the Server, the Agent redeems the short-lived credential, a runtime Agent identity/session credential is established, and MAC addresses/hardware fingerprints remain inventory signals rather than trust anchors.

The provisioning LAN is controlled but not inherently trustworthy (architecture-redesign.md "Security invariants"). MAC addresses are explicitly not authentication or permanent identity — this is a mandatory repository-wide rule (`AGENTS.md`), not something this ADR can weaken.

## Decision

Durable Endpoint identity is a Server-assigned identifier, independent of any hardware attribute. Inventory signals (MAC, disk fingerprint, hardware serials/DMI data when available) are evidence attached to an Endpoint identity record; they are never the identity itself.

Evaluating the proposed enrollment/trust bootstrap direction:

1. A booting endpoint reaches the Boot Orchestrator over the provisioning network — a position of some trust, since it received a lease and a boot artifact from Bamep's own controlled boot chain, but not proof of identity.
2. The Boot Orchestrator issues a short-lived enrollment credential scoped to that specific boot attempt, not to a MAC address.
3. The Agent authenticates the Server before presenting any credential. This ADR states the requirement only; the concrete mutual-authentication mechanism belongs to the Agent control-protocol Work Package (Issue #3) and is not decided here.
4. The Agent redeems the short-lived enrollment credential with the Server.
5. Redemption resolves to one of two outcomes depending on whether the endpoint's inventory signals match a known, previously enrolled Endpoint record:
   - **First-seen endpoint**: no existing match. The Endpoint enters `PendingEnrollment` and requires explicit operator approval before it is trusted (see "Decision: operator-approval-gated first enrollment" below).
   - **Reconnecting known (`Enrolled`) endpoint**: inventory signals match a known Endpoint, but the match alone must never authorize reuse of a previous runtime credential; a fresh runtime credential is issued through the same redemption flow (see "Reconnect handling"), without re-running operator approval, since the Endpoint's trusted identity already persists.
6. On successful redemption, the Server issues a runtime Agent identity/session credential scoped to the Endpoint's durable identity, with an expiry/renewal policy (exact TTL is an implementation-time detail, not an M0 architectural question).

No concrete architectural blocker was identified in this direction for Bamep's V1 threat model and installation profiles (3–24 endpoints, single Server, controlled LAN).

## Decision: operator-approval-gated first enrollment

First-time Endpoint enrollment requires explicit operator approval by default. A newly observed device on the provisioning network must not automatically become a trusted Endpoint merely because it can reach the Bamep Server — auto-trust is rejected as the M0 default. The provisioning LAN is explicitly "controlled but not inherently trustworthy," and destructive operations later depend on Endpoint identity as a trust anchor; auto-trusting any device that can reach the network for PXE boot would extend trust further than the stated threat model supports.

MAC addresses and hardware fingerprints remain inventory/confidence signals, not trust anchors, at every stage of this decision (unchanged from the original evaluation).

Once an Endpoint has been explicitly enrolled, normal reconnects, reboots, and credential renewal must not require repeated operator approval when continuity of the trusted identity can be established (see "Reconnect handling" below and the continuity rule in `docs/specifications/m0-endpoint-identity-lifecycle.md`). Operator approval is a first-enrollment gate, not a recurring tax on legitimate, already-trusted Endpoints.

Significant hardware changes on an already-enrolled Endpoint may lower identity confidence and require operator review, without necessarily un-enrolling the Endpoint outright. The exact confidence model (a graduated `LoweredConfidence`/`Conflict` distinction, not a binary stale flag) is defined in `docs/specifications/m0-endpoint-identity-lifecycle.md`, since it is a state-model concern rather than an enrollment-mechanism decision.

**Future, not required in M0**: pre-authorized enrollment, where an operator explicitly authorizes an enrollment context/token before an endpoint's first connection, may be supported later. This must not be treated as, or implemented as, unrestricted automatic enrollment — the explicit operator action is simply performed before rather than after first contact. Its mechanism is not designed by this ADR.

## Destructive-operation authorization preconditions

Once an Endpoint identity exists, any destructive operation targeting it must validate, immediately before execution, all of the following **independent** preconditions — trusted persistent Endpoint identity, an authenticated current Agent session, an authorized Job/action, sufficiently fresh inventory, target disk identity/fingerprint revalidation, and the absence of an unresolved identity or hardware-confidence conflict. None of these may be inferred from another, and in particular an `Enrolled` identity alone is never sufficient authorization on its own.

The full precondition list, the state dimensions it depends on, and which Work Package owns each dimension are defined in `docs/specifications/m0-endpoint-identity-lifecycle.md` (this ADR does not duplicate that detail). It is available now for the Job lifecycle (Issue #4) and data-plane (Issue #6) Work Packages to reference directly rather than re-derive.

## Hardware-change handling

When an Endpoint's observed inventory signals (MAC, disk fingerprint, etc.) no longer match its previously recorded signals, the Server must not silently update the Endpoint's identity record to match the new hardware, and must not silently resolve the discrepancy in either direction. Divergence is surfaced as a hardware-confidence condition requiring explicit operator review; the graduated confidence model and its resolution rules are defined in `docs/specifications/m0-endpoint-identity-lifecycle.md`.

## Reconnect handling

Reconnect must not blindly re-establish trust merely because a MAC address or other hardware signal matches previous inventory (Issue #2 safety constraint). A reconnecting Agent re-authenticates and redeems a fresh runtime credential through the same flow as an initial connection; a previous runtime credential does not carry implicit trust across a disconnect.

## Alternatives considered

- **Pre-shared per-device secret, provisioned out-of-band** (e.g., injected via USB before first boot): rejected as the V1 default — reintroduces the manual per-device step the credential-bootstrap flow is meant to avoid. Remains an option for higher-security deployments beyond V1's stated scope.
- **PKI/mTLS with a deployment-specific CA issuing per-Agent certificates**: technically stronger, but requires certificate lifecycle management (issuance, rotation, revocation) beyond what M0's small/medium/high-density install profiles require. The short-lived-credential bootstrap achieves an equivalent boot-time/runtime trust separation with less operational machinery. Not rejected outright — may be revisited if a later requirement (e.g., HA, multi-site) demands stronger cryptographic identity.
- **TPM-based attestation**: rejected for V1 — depends on hardware capability not guaranteed across Bamep's target endpoint population; no current requirement demands hardware-rooted attestation.
- **Automatic re-trust purely from MAC/hardware-fingerprint match on reconnect**: rejected — directly conflicts with the invariant that MAC/hardware fingerprints are inventory signals, not trust anchors, and with the explicit safety constraint against blindly re-establishing trust on reconnect.

## Consequences

- Web Administration must support an operator approval action for `PendingEnrollment` Endpoints before implementation of the enrollment flow is complete; this is a durable requirement on the Presentation/Application layers, not an optional convenience.
- The Boot Orchestrator becomes a component with real security responsibility (issuing enrollment credentials), constraining its design within the boot-orchestration boundary (`docs/specifications/m0-stack-and-boundaries-baseline.md`).
- The destructive-operation authorization preconditions are available now and should be referenced, not re-derived or narrowed to a single check, by Issues #4 and #6.
- Agent/Server mutual authentication is a requirement here but its concrete design belongs to Issue #3; this ADR must not be read as having decided that mechanism.
- Pre-authorized enrollment remains a possible future extension and must not be implemented as a bypass of the operator-approval requirement established here.

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Endpoint identity", "Security invariants".
- `docs/discovery/adr-triage.md` — candidate 2; candidate 12 (identity-dependent portion only).
- `AGENTS.md` — Safety section.
- `docs/specifications/m0-endpoint-identity-lifecycle.md` — the identity lifecycle state model this decision governs.

## Related work

- Issue #2 — `[WP] Define endpoint identity and trust model`.
- Issue #3 — `[WP] Define Agent control and action contracts` (owns the mutual-authentication mechanism this ADR requires but does not design).
- Issue #4 — `[WP] Define Job lifecycle and scheduling model` (consumes the destructive-operation authorization preconditions).
- Issue #6 — `[WP] Define data-plane and storage contracts` (consumes the destructive-operation authorization preconditions for artifact/transfer safety).
