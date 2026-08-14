# ADR-0004: Endpoint identity and enrollment/trust bootstrap model

Status: Proposed

## Context

M0 requires resolving Endpoint identity and the enrollment/trust bootstrap (`docs/discovery/adr-triage.md` candidate 2; `docs/specifications/m0-architecture-baseline.md` scope item "Endpoint identity"; `docs/discovery/architecture-redesign.md` "Endpoint identity", "Security invariants"; `AGENTS.md` Safety section). Issue #2 executes this Work Package.

Endpoint identity must survive NIC or MAC replacement (architecture-redesign.md "Endpoint identity"). Architecture-redesign.md proposes a direction "to evaluate through ADR": a Boot Orchestrator issues a short-lived enrollment context/credential, the Agent authenticates the Server, the Agent redeems the short-lived credential, a runtime Agent identity/session credential is established, and MAC addresses/hardware fingerprints remain inventory signals rather than trust anchors.

The provisioning LAN is controlled but not inherently trustworthy (architecture-redesign.md "Security invariants"). MAC addresses are explicitly not authentication or permanent identity — this is a mandatory repository-wide rule (`AGENTS.md`), not something this ADR can weaken.

## Decision (evaluated, not yet accepted)

Durable Endpoint identity is a Server-assigned identifier, independent of any hardware attribute. Inventory signals (MAC, disk fingerprint, hardware serials/DMI data when available) are evidence attached to an Endpoint identity record; they are never the identity itself.

Evaluating the proposed enrollment/trust bootstrap direction:

1. A booting endpoint reaches the Boot Orchestrator over the provisioning network — a position of some trust, since it received a lease and a boot artifact from Bamep's own controlled boot chain, but not proof of identity.
2. The Boot Orchestrator issues a short-lived enrollment credential scoped to that specific boot attempt, not to a MAC address.
3. The Agent authenticates the Server before presenting any credential. This ADR states the requirement only; the concrete mutual-authentication mechanism belongs to the Agent control-protocol Work Package (Issue #3) and is not decided here.
4. The Agent redeems the short-lived enrollment credential with the Server.
5. Redemption resolves to one of two outcomes depending on whether the endpoint's inventory signals match a known, previously enrolled Endpoint record:
   - **First-seen endpoint**: no existing match. Whether the Server auto-trusts a new Endpoint identity at this point, or requires explicit operator approval first, is the open decision below — not decided by this ADR.
   - **Reconnecting known endpoint**: inventory signals match a known Endpoint, but the match alone must never authorize reuse of a previous runtime credential; a fresh runtime credential is issued through the same redemption flow (see "Reconnect handling").
6. On successful redemption, the Server issues a runtime Agent identity/session credential scoped to the Endpoint's durable identity, with an expiry/renewal policy (exact TTL is an implementation-time detail, not an M0 architectural question).

No concrete architectural blocker was identified in this direction for Bamep's V1 threat model and installation profiles (3–24 endpoints, single Server, controlled LAN).

## Open decision (requires owner approval)

Whether first-seen-endpoint enrollment is:

- **(a) auto-trusted** — any endpoint that reaches the Boot Orchestrator through the controlled provisioning network and completes the credential exchange is automatically enrolled as a new Endpoint; or
- **(b) operator-approval-gated** — a first-seen endpoint is recorded as pending and requires explicit operator approval (e.g., via Web Administration) before it is enrolled and eligible for provisioning/recovery Jobs.

**Recommendation: (b), operator-approval-gated enrollment.** The provisioning LAN is explicitly "controlled but not inherently trustworthy," and destructive operations later depend on Endpoint identity as a trust anchor — auto-trusting any device that can reach the network for PXE boot extends trust further than the stated threat model supports. This is a recommendation, not a decision: unattended zero-touch enrollment has genuine operational value the owner may prefer for a specific deployment profile, and the trade-off is the owner's to make.

## Destructive-operation identity precondition (not open — stated invariant)

Regardless of which enrollment mechanism is accepted, once an Endpoint identity exists, any destructive operation targeting it must validate, immediately before execution:

1. the Endpoint identity itself (matches the durable identity record, not merely a MAC/hardware signal);
2. the current inventory revision (the operation was authorized against inventory that is still current);
3. disk identity/fingerprint (the target disk/volume matches what the operation was authorized against).

This precondition is required by `docs/discovery/architecture-redesign.md` "Security invariants" independent of the enrollment mechanism. It is available now for the Job lifecycle (Issue #4) and data-plane (Issue #6) Work Packages to reference directly rather than re-derive. The full state model behind it is defined in `docs/specifications/m0-endpoint-identity-lifecycle.md`.

## Hardware-change handling

When an Endpoint's observed inventory signals (MAC, disk fingerprint, etc.) no longer match its previously recorded signals, the Server must not silently update the Endpoint's identity record to match the new hardware. Divergence must be surfaced as a stale-signal condition requiring explicit operator confirmation before the Endpoint's record is updated and before further destructive operations are authorized against it.

## Reconnect handling

Reconnect must not blindly re-establish trust merely because a MAC address or other hardware signal matches previous inventory (Issue #2 safety constraint). A reconnecting Agent re-authenticates and redeems a fresh runtime credential through the same flow as an initial connection; a previous runtime credential does not carry implicit trust across a disconnect.

## Alternatives considered

- **Pre-shared per-device secret, provisioned out-of-band** (e.g., injected via USB before first boot): rejected as the V1 default — reintroduces the manual per-device step the credential-bootstrap flow is meant to avoid. Remains an option for higher-security deployments beyond V1's stated scope.
- **PKI/mTLS with a deployment-specific CA issuing per-Agent certificates**: technically stronger, but requires certificate lifecycle management (issuance, rotation, revocation) beyond what M0's small/medium/high-density install profiles require. The short-lived-credential bootstrap achieves an equivalent boot-time/runtime trust separation with less operational machinery. Not rejected outright — may be revisited if a later requirement (e.g., HA, multi-site) demands stronger cryptographic identity.
- **TPM-based attestation**: rejected for V1 — depends on hardware capability not guaranteed across Bamep's target endpoint population; no current requirement demands hardware-rooted attestation.
- **Automatic re-trust purely from MAC/hardware-fingerprint match on reconnect**: rejected — directly conflicts with the invariant that MAC/hardware fingerprints are inventory signals, not trust anchors, and with the explicit safety constraint against blindly re-establishing trust on reconnect.

## Consequences

- Enrollment/trust bootstrap implementation cannot begin until the open decision above (auto-trust vs. operator-approval-gated) is resolved by the owner.
- The Boot Orchestrator becomes a component with real security responsibility (issuing enrollment credentials), constraining its design within the boot-orchestration boundary (`docs/specifications/m0-stack-and-boundaries-baseline.md`).
- The destructive-operation identity precondition is available now and should be referenced, not re-derived, by Issues #4 and #6.
- Agent/Server mutual authentication is a requirement here but its concrete design belongs to Issue #3; this ADR must not be read as having decided that mechanism.

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Endpoint identity", "Security invariants".
- `docs/discovery/adr-triage.md` — candidate 2; candidate 12 (identity-dependent portion only).
- `AGENTS.md` — Safety section.
- `docs/specifications/m0-endpoint-identity-lifecycle.md` — the identity lifecycle state model this decision governs.

## Related work

- Issue #2 — `[WP] Define endpoint identity and trust model`.
- Issue #3 — `[WP] Define Agent control and action contracts` (owns the mutual-authentication mechanism this ADR requires but does not design).
- Issue #4 — `[WP] Define Job lifecycle and scheduling model` (consumes the destructive-operation identity precondition).
- Issue #6 — `[WP] Define data-plane and storage contracts` (consumes the destructive-operation identity precondition for artifact/transfer safety).
