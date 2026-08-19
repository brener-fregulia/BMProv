# ADR-0012: Runtime Agent credential issuance, rotation, and reconnect recovery

Status: Accepted

## Context

ADR-0004 (Endpoint identity and enrollment/trust bootstrap model, `Accepted`) establishes that
the Boot Orchestrator issues a short-lived enrollment credential scoped to a boot attempt, that
the Agent redeems it, and that "the Server issues a runtime Agent identity/session credential
scoped to the Endpoint's durable identity, with an expiry/renewal policy (exact TTL is an
implementation-time detail, not an M0 architectural question)" (ADR-0004 point 6), and that
reconnect "redeems a fresh runtime credential through the same flow as an initial connection"
(ADR-0004, "Reconnect handling"). ADR-0005 (Agent control-plane protocol and typed-action model,
`Accepted`) establishes that "successful completion of this handshake is the mechanism that
establishes `CredentialActive`" and that the handshake ends in
`SessionEstablished{protocol_version, session_id}` (`docs/specifications/m0-agent-protocol-contract.md`).

Neither ADR-0004 nor ADR-0005 defines the concrete mechanism by which the runtime credential the
Server "issues" on each successful redemption is delivered to the Agent, nor how a fresh runtime
credential safely replaces its predecessor across reconnects without leaving the Agent unable to
authenticate if a message is lost between commit and delivery. This gap was found while defining
Work Package WP1 of the M1 Milestone (Issue #17, `[WP] Establish simulated Endpoint trust,
enrollment, and Agent session`): `m0-agent-protocol-contract.md`'s
`SessionEstablished{protocol_version, session_id}` carries no credential field, and no other
approved Agent Protocol v1 message conveys the issued runtime credential back to the Agent.
Issue #17 had provisionally recorded an unapproved working interpretation ("Reading A") that
avoided the gap by treating the Server's internal `CredentialActive` state as itself constituting
"the issued/renewed runtime credential," so the Agent would reuse its original credential value
indefinitely. That interpretation conflicts with ADR-0004's own language ("issues a runtime
credential," a reconnect "redeems a **fresh** runtime credential") and is rejected by this ADR.

This ADR resolves how the runtime credential is issued, delivered, safely replaced across
reconnects without stranding the Agent, and how it composes with revocation — without reopening
ADR-0004's enrollment/bootstrap model or ADR-0005's transport/handshake/typed-message decisions.

## Decision

1. **Boot-scoped chain.** A single, uniform mechanism governs every credential exchange:
   presenting a still-valid predecessor credential in `AuthRequest` and receiving a freshly
   issued successor. The predecessor may be the boot-scoped enrollment credential issued by the
   Boot Orchestrator (ADR-0004 point 2) on the first `AuthRequest` of a boot, or a previously
   issued runtime credential on any later reconnect:

   ```text
   same boot:      E1 → R1 → R2 → R3 → ...
   genuine reboot:  E2 (new enrollment credential) → fresh runtime credential → ...
   ```

   A runtime credential does not need to survive a genuine Agent reboot. A new boot always
   re-engages the Boot Orchestrator for a new enrollment credential and restarts the redemption
   flow (ADR-0004 point 1); Endpoint identity continuity across reboot is carried by the identity
   dimension (`Enrolled`, matched by inventory signals — ADR-0004, "Reconnect handling"), never
   by credential persistence across boots. Operator approval is not repeated when Endpoint
   continuity holds, unchanged from ADR-0004.
2. **Persist-before-send ordering.** On a successful `AuthRequest`, durable Endpoint
   identity/credential changes (predecessor grace bookkeeping, the newly minted successor, and —
   on first contact only — `PendingEnrollment` creation and the `NoActiveCredential →
   CredentialActive` transition) commit in one atomic persistence transaction, consistent with the
   atomic state+event+audit model already required by ADR-0013 (carrying forward ADR-0007) /
   `m0-persistence-observability-and-domain-events.md`. Only after that commit does the Server
   attempt to deliver `SessionEstablished` (carrying the new credential) over WSS. A database
   transaction and a WebSocket send cannot be atomic with each other — the same constraint
   ADR-0013 carries forward (point 16), originally established in ADR-0007's "Crash-safe dispatch
   persistence ordering" for `ActionDispatch`, applied here to credential issuance. A crash or
   dropped connection between commit and delivery
   is therefore an expected, not exceptional, case, recovered by point 3.
3. **Replacement, not redelivery, of an unconfirmed successor.** If a predecessor `P` is
   presented again in a fresh `AuthRequest` while its previously issued successor `S` has never
   itself successfully authenticated, and `P` is still within its grace/expiry bound, the Server
   does not attempt to reconstruct or redeliver `S`. It atomically supersedes `S`, mints a fresh
   successor `S'`, and `SessionEstablished` carries `S'`. The Server is never required to
   reconstruct a previously emitted secret.
4. **Bounded valid set.** For one credential chain, the durable valid set never exceeds: one
   predecessor in grace, plus at most one current unconfirmed successor. Replacing an unconfirmed
   successor invalidates it atomically within the same transaction that mints its replacement; no
   unbounded accumulation of valid credentials is permitted.
5. **Confirmation semantics.** Receiving `SessionEstablished` does not by itself confirm delivery
   of the successor. A successor becomes confirmed only when it is later presented in an
   `AuthRequest` and successfully authenticates. On that confirmation: its predecessor is retired,
   the confirmed successor becomes the predecessor for the next rotation, and a fresh successor is
   issued.
6. **Agent-side retention.** The Agent retains its predecessor `P` until the successor `S`
   delivered to it has itself successfully authenticated on a later connection. If `S` is
   rejected while `P` is still within its valid grace/expiry bound, the Agent may retry using `P`.
   `AuthError` is not required to reveal whether the presented credential was a superseded
   successor as opposed to any other rejection cause — a generic authentication rejection is
   sufficient, consistent with the already-accepted minimal-disclosure precedent in
   `TransferAuthorizationDenied.reason` (`m0-agent-protocol-contract.md`).
7. **Concurrent redemption.** Multiple connections may authenticate concurrently while presenting
   the same still-valid predecessor. Credential issuance is conceptually serialized by the
   durable transaction each successful redemption commits in: the last committed successor is the
   only current one. An already-established WSS session is not retroactively invalidated merely
   because the credential issued to it for a future reconnect was later superseded by a
   concurrent redemption. The exact locking/isolation mechanism is implementation-time.
8. **Revocation.** Explicit `CredentialRevoked` invalidates every credential still valid in the
   Endpoint's chain at that instant — the current predecessor in grace and any unconfirmed
   successor alike — never only the most recently issued value.
9. **Rotation is not a new lifecycle transition.** Routine rotation while the credential
   dimension remains `CredentialActive` (points 3, 5, 7) is durable bookkeeping required to
   validate a future `AuthRequest`; it does not change the credential dimension's value and does
   not, by itself, introduce a new domain event. The illustrative domain-event catalog
   (`m0-persistence-observability-and-domain-events.md`) remains transition-oriented, not
   rotation-oriented.
10. **No recoverable-secret requirement.** This model never requires the Server to store or
    reconstruct a previously issued plaintext runtime credential. The concrete durable
    representation (e.g., a salted hash verified against a presented value, or a self-verifying
    signed capability in the style of `TransferAuthorizationGrant`'s `token`) remains
    implementation-time, provided it satisfies points 1–9.
11. **Wire mechanism.** `SessionEstablished` is extended —
    `SessionEstablished{protocol_version, session_id, runtime_credential, credential_expires_at}`
    — rather than introducing a separate message type. Issuance of the runtime credential is 1:1
    with, and decided in the same instant as, successful credential validation and session
    establishment (ADR-0004 point 6); a dedicated message would introduce a new partial-delivery
    failure state ("session established but credential never arrived") that bundling into one
    message structurally cannot produce. `runtime_credential` is opaque from Agent Protocol's own
    perspective, matching how `bootstrap_assertion` and the `TransferAuthorizationGrant` `token`
    are already treated; `credential_expires_at` follows the timestamp convention
    `m0-agent-protocol-contract.md` "Wire encoding" already defines (RFC 3339 / ISO 8601 UTC).

## Alternatives considered

- **Immediate predecessor invalidation**: rejected — directly causes the stranding failure this
  ADR must prevent: a crash or dropped connection between commit (point 2) and delivery would
  leave the Agent holding no usable credential.
- **Same-successor redelivery requiring recoverable secret material**: reapresenting the
  predecessor while the successor is unconfirmed always returns the identical successor value.
  Rejected as the M0 default — it requires the Server to reconstruct a previously issued secret,
  via either a new encrypted-secret-recovery capability or a deterministic-derivation scheme
  keyed by a long-lived Server secret. Both introduce a durable-secrets-management responsibility
  no M0 ADR or Specification currently owns, a strictly larger at-rest exposure surface than
  hash-only verification, and their own protection/rotation/recovery treatment.
- **Replacement of an unconfirmed successor** (adopted): satisfies the same anti-stranding
  requirement without ever reconstructing a secret; keeps durable representation hash-only (or
  self-verifying-opaque), homogeneous with every other credential in this model.
- **Explicit delivery acknowledgement**: rejected for M0 — the marginal reduction in the
  predecessor's replay window is small relative to Bamep's V1 threat model (controlled LAN,
  pinned-TLS-authenticated channel, 3–24 endpoints, infrequent reconnects), and adds a protocol
  round trip and a new message type for a property the "confirmed by next successful use" rule
  already provides implicitly.
- **Dedicated `RuntimeCredentialIssued` message**: rejected — `BootstrapEvidence`/
  `TransferAuthorizationGrant` carry facts that are N:1 or asynchronous relative to
  `SessionEstablished`. Runtime credential issuance is 1:1 and decided in the same instant as
  session establishment; splitting it into a second message introduces a new ambiguous
  partial-delivery failure state that bundling cannot produce, with no corresponding benefit. Not
  preferred merely for syntactic consistency with the additive-message precedent.
- **Collapsing the runtime credential into the Server-side `CredentialActive` state** (the
  rejected Issue #17 "Reading A"): the Agent would reuse its original fixture/enrollment-issued
  value indefinitely and no message ever conveys a distinct credential back to it. Rejected —
  contradicts ADR-0004's own language ("issues a runtime credential," redeem a "**fresh** runtime
  credential" on every reconnect) and `m0-endpoint-identity-lifecycle.md`'s existing treatment of
  the runtime credential as an artifact distinct from the `CredentialActive` state fact.

## Consequences

- `m0-endpoint-identity-lifecycle.md` is amended to define the credential-chain/grace/
  replacement/confirmation/revocation model in the "Credential/session lifecycle" dimension it
  already owns — this ADR records the decision and its reasoning; that Specification remains the
  normative lifecycle definition.
- `m0-agent-protocol-contract.md` is amended to extend `SessionEstablished` with
  `runtime_credential` and `credential_expires_at`, and to define Agent-side retention/fallback
  behavior and the generic-rejection requirement for a superseded successor — this ADR does not
  restate that wire-level detail as the normative source.
- Issue #17's now-invalid "Reading A" working interpretation is removed and replaced by a
  reference to this ADR and the amended Specifications.
- Neither ADR-0004 nor ADR-0005 is reopened or rewritten; both remain historically accurate as
  written. This ADR consumes them and resolves the specific sub-problem both explicitly left to a
  later Work Package.
- No new durable-secrets-management component is introduced; concrete credential representation
  remains implementation-time, bounded by point 10.
- The exact numeric grace/expiry duration remains implementation-time, unchanged from ADR-0004
  point 6's existing delegation.

## Related architecture

- `docs/specifications/m0-endpoint-identity-lifecycle.md` — the Credential/session lifecycle
  dimension this ADR's model extends.
- `docs/specifications/m0-agent-protocol-contract.md` — the `SessionEstablished` wire contract
  this ADR's model extends.
- `docs/specifications/m0-persistence-observability-and-domain-events.md` — the atomic
  state+event+audit transaction model (originally ADR-0007, carried forward by ADR-0013) this
  ADR's persist-before-send ordering and rotation/event distinction rely on.

## Related work

- ADR-0004 — Endpoint identity and enrollment/trust bootstrap model (`Accepted`) — the
  enrollment/runtime-credential model and reconnect redemption flow this ADR consumes without
  reopening.
- ADR-0005 — Agent control-plane protocol and typed-action model (`Accepted`) — the
  WSS/typed-envelope/handshake decisions this ADR consumes without reopening; `SessionEstablished`'s
  existence and its role in establishing `CredentialActive`.
- ADR-0013 — PostgreSQL persistence backend baseline (`Accepted`) — the current persistence
  backend; carries forward the crash-safe persist-before-send ordering pattern this ADR applies to
  credential issuance.
- ADR-0007 — Persistence backend and durable/transient boundary (`Superseded by ADR-0013`) —
  originally established the crash-safe persist-before-send ordering pattern this ADR applies to
  credential issuance; the backend-independent pattern itself is unchanged and is carried forward
  by ADR-0013.
- Issue #17 — `[WP] Establish simulated Endpoint trust, enrollment, and Agent session` — the Work
  Package whose Discovery surfaced this gap, and which implements this ADR's model; its
  previously recorded "Reading A" is superseded by this ADR.
