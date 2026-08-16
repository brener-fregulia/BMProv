# M0 — Data-Plane and Storage Contracts

Status: **Approved**

## Context

This Specification details the chunk-oriented data-plane contract, artifact lifecycle, and storage capability model accepted in ADR-0008, executing Issue #6 (`[WP] Define data-plane and storage contracts`). It applies the empirical evidence in `docs/reference/transfer-resumability-spike.md`.

## Chunk manifest

Every artifact transferred over the data plane has a **chunk manifest**:

- `artifact_id` — identifies the artifact this manifest belongs to.
- `digest_algorithm` — identifies the cryptographic digest algorithm used for every `digest` value in this manifest (chunk and artifact level). **Not selected by this Specification** — see ADR-0008 point 3; must be fixed before the concrete Agent/Server wire contract is implemented.
- `chunk_size` — fixed for this manifest; exact value is implementation-time tuning (ADR-0008).
- `chunk_count` — fixed once the manifest is **sealed** (see "Manifest construction and sealing" below); not assumed known before then.
- per chunk: `chunk_index`, `digest` (per `digest_algorithm`), `size` (the last chunk may be shorter than `chunk_size`).
- `artifact_digest` — the **expected** full-artifact digest, per `digest_algorithm`, independent of the per-chunk digests. For an Agent → Server capture, this is computed by the producer **incrementally while producing the logical Artifact**, not by a second pre-read of the completed source. It is checked (independently recomputed and compared, never redefined) at `PendingVerification` → `Verified` (see "Artifact lifecycle").

Chunk identity metadata is durable domain data (per ADR-0007's durable/transient boundary) — it is not high-frequency data. Each chunk's identity (index, size, digest) is written durably as that chunk is produced, not batched into one complete write only at the end.

## Manifest construction and sealing

A manifest is not necessarily complete when a capture begins — requiring the complete source to be pre-read solely to construct a manifest before any chunk can be transferred is not an M0 requirement, and would be impractical for an Agent → Server capture of a large volume/image.

- `digest_algorithm` is fixed for the Artifact before the first chunk identity using it is committed — it cannot change partway through a capture.
- **Construction** (while the Artifact is `Incomplete`): as each logical chunk is produced, its identity (index, size, digest) becomes durable manifest metadata at that time; the expected `artifact_digest` is updated incrementally alongside it.
- **Sealing**: once every expected chunk for the capture has been identified/produced (end of the source range for Volume/Image; end of file enumeration for Selective), the manifest is **sealed** — `digest_algorithm`, the expected `artifact_digest`, `chunk_count`, and the full set of chunk identities become fixed. A sealed manifest is immutable: none of those values may be added, removed, or changed afterward.
- The Artifact may reach `PendingVerification` only once its manifest is sealed **and** every sealed chunk identity has a durably received, verified chunk matching it (see "Artifact lifecycle").
- **Capture continuation** (producing a chunk whose identity does not yet exist in the manifest) is distinct from **transfer resume/retransmission** (a chunk identity already exists, sealed or not, and the same bytes must be reproduced from the source or retrieved from durable staging to satisfy it) — the latter is what `docs/reference/transfer-resumability-spike.md` Experiments C, D, and E evaluated.
- If a previously identified chunk cannot be reproduced to match its recorded `digest`, that `digest` is **never rewritten** to accept different bytes under the same chunk identity. The affected chunk, and the Artifact, follow the failure policy below instead.
- **Verification never defines or rewrites the value it checks**: the receiving/verifying side independently computes the digest over the stored, correctly ordered Artifact content and compares it against the sealed expected `artifact_digest` — it does not treat its own computed value as ground truth.

## Chunk transfer

- Each chunk is transferred as an independently addressable unit (e.g., one HTTP request per chunk), carrying `artifact_id`, `chunk_index`, and enough information for the receiver to verify the chunk's digest immediately upon receipt.
- A chunk that fails digest verification on receipt is rejected and not written to durable chunk storage as if it were valid — the sender is expected to retry that chunk.
- Resume logic: before (re)transferring any chunk, the receiver checks whether it already holds a chunk at that index matching the manifest digest; if so, that chunk is skipped. Only missing or mismatching chunks are (re)transferred — directly implementing the pattern validated in `docs/reference/transfer-resumability-spike.md` Experiments C and D.
- Chunk transfer direction is symmetric: the same manifest/verification pattern applies whether the Agent is producing (backup/capture) or consuming (provisioning/restore) the artifact.

## Transfer-session authentication

Accepted (ADR-0008 point 9, executing Issue #15): every data-plane transfer is authorized and authenticated by a **short-lived, transfer-scoped, sender-constrained capability** — a Server-signed capability bound to an ephemeral Agent-held asymmetric proof key, never a plain bearer capability — delivered over the already-authenticated Agent Protocol control-plane channel and presented, together with a fresh per-request proof of possession, on the HTTPS data-plane channel. This section is the full operational contract; ADR-0008 records the decision and its rationale, including why a plain bearer capability was evaluated and rejected.

**The capability alone must not authorize a data-plane request.** Possession of a stolen capability, without the corresponding ephemeral private proof key, must be insufficient to use it.

### Transport

The data plane is **HTTPS**, not plain HTTP. Server identity reuses the same pinned Server TLS certificate/fingerprint already authenticated for the Agent Protocol WSS connection via trusted bootstrap (`docs/decisions/0010-trusted-bootstrap-and-secure-boot-baseline.md`, `docs/decisions/0011-site-trust-anchor-operator-verified-pairing.md`) — no second trust relationship is introduced, and no new site trust-anchor question is reopened. The Agent does not present a client certificate; the data plane is not mTLS, consistent with Agent Protocol (`m0-agent-protocol-contract.md` "Transport and handshake") — sender-constraint is achieved at the application level (below), not via mTLS.

### Ephemeral proof key

For a transfer-authorization context:

1. the already-authenticated Agent generates an asymmetric ephemeral keypair;
2. the private key remains Agent-local, in memory only, and is never persisted;
3. it is **never** an Endpoint identity credential and is **never** persisted as durable Endpoint identity/trust state (`docs/specifications/m0-endpoint-identity-lifecycle.md` is not extended or reinterpreted by this key);
4. `TransferAuthorizationRequest` supplies the public key, or a canonical representation sufficient for the Server to derive its cryptographic thumbprint;
5. the Server-issued capability binds to that key's thumbprint.

The concrete asymmetric algorithm and serialization are implementation-time choices, provided the resulting representation is explicit and interoperable. The capability-signing mechanism (Server-held signing secret, point 4 of ADR-0008 point 9) and the proof-key algorithm do **not** need to be the same.

The ephemeral key's own lifetime is intentionally bounded to what the authorization context needs — never longer than the owning transfer's active lifetime, and typically shorter, since it is lost on any Agent process restart (see "Reconnect and restart behavior").

### Authorization bindings

One sender-constrained capability is bound to exactly:

- `endpoint_id`;
- `transfer_id`;
- `artifact_id`;
- direction (Agent → Server, or Server → Agent);
- `attempt_id` of the transfer JobStep's Attempt (`docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md`) that caused it to be issued;
- authorization expiry / bounded lifetime;
- the proof-key thumbprint (see "Ephemeral proof key" above);
- a unique capability identifier or equivalent cryptographic identity sufficient for proof binding (see "Per-request proof of possession" below).

No other identifier is bound merely because it exists — `job_id`/`jobstep_id`/`action_id` are reachable transitively through `attempt_id` and are not separately embedded, since they provide no independently necessary authorization property beyond it. A capability authorizes only the exact `(endpoint_id, transfer_id, artifact_id, direction)` tuple it was issued for, presented together with proof of possession of its bound key; it never authorizes another transfer, another Artifact, the opposite direction, or another Endpoint, even for the same Agent session, and it is not a generic data-plane credential.

### Issuance sequence

```text
1. Agent is authenticated over Agent Protocol (SessionEstablished,
   BootstrapEvidence already sent) — unchanged.
2. The transfer JobStep's Attempt is dispatched normally via
   ActionDispatch{action_id, action_type, parameters: {transfer_id,
   artifact_id, direction, ...}} — unchanged Job/Attempt dispatch flow
   (ADR-0006, ADR-0007 persist-before-send), no special-casing.
3. Agent sends ActionAck{outcome: Accepted}.
4. Agent generates an ephemeral asymmetric proof keypair (or reuses
   one already held for this transfer_id, see "Renewal").
5. Agent sends TransferAuthorizationRequest{transfer_id,
   proof_public_key} over the already-authenticated control-plane
   connection.
6. Server checks: does a non-terminal Attempt exist for this
   transfer_id, bound to the requesting Endpoint's identity/session,
   and is the Endpoint's credential CredentialActive?
     - yes → TransferAuthorizationGrant{transfer_id, token, expires_at}
             where token is bound to proof_public_key's thumbprint
     - no  → TransferAuthorizationDenied{transfer_id, reason}
7. Agent opens the HTTPS data-plane connection; each chunk request
   carries the capability (token) plus a fresh proof, signed by the
   ephemeral private key, of possession of the key the capability is
   bound to (see "Per-request proof of possession").
8. Server verifies the capability, the proof, and current durable
   state on every chunk request (see "Per-request verification"
   below); on success, chunk transfer proceeds exactly as already
   specified in "Chunk transfer" above.
9. If the capability expires, or the Agent no longer holds a usable
   key/capability pair (e.g. after an Agent process restart), while
   the underlying transfer remains legitimately active, the Agent
   repeats steps 4–5 for the same transfer_id — a renewal, never a
   new Attempt and never a new transfer_id.
```

`TransferAuthorizationRequest` / `TransferAuthorizationGrant` / `TransferAuthorizationDenied` are three new, strictly additive Agent Protocol v1 message types — see `docs/specifications/m0-agent-protocol-contract.md` "Transfer authorization". Their addition does not reopen WSS, pinned TLS, `AuthRequest`/`SessionEstablished`, or `BootstrapEvidence`.

### Per-request proof of possession

Every HTTPS data-plane chunk request carries **both**:

1. the sender-constrained transfer capability; and
2. a fresh proof, signed by the ephemeral private key, that the presenter possesses the private key the capability is bound to.

The signed proof is a **fixed, domain-separated/versioned structure** — the Server does not sign, and the Agent does not sign into this proof, an arbitrary caller-controlled byte string. At minimum, the signed proof binds:

- a proof-contract discriminator/version (so a proof cannot be confused with an unrelated signed structure);
- the capability identifier, or a cryptographic hash/identity of the exact capability being presented (binds this proof to that one capability, not a different, possibly stolen one);
- the HTTP operation/method (binds the proof to the specific operation — upload vs. download — preventing a captured read-proof from being replayed as a write, or vice versa);
- `transfer_id`;
- `artifact_id`;
- direction;
- `chunk_index` or equivalent exact chunk identity (binds the proof to the specific chunk request, preventing replay against a different chunk);
- `proof_id` — a cryptographically unpredictable, unique identifier for this proof (the replay-detection key, see "Replay and freshness semantics");
- `issued_at` — the proof's creation time (the freshness input, see "Replay and freshness semantics").

`transfer_id`/`artifact_id`/direction are already carried by the capability itself (and covered by its own signature); binding the proof to the capability's identity is what transitively inherits them for the proof rather than duplicating claims that could otherwise drift out of sync.

### Per-request verification

The Server verifies, for every chunk request, **all** of the following — the complete authorization decision succeeds only if all required checks succeed:

- capability signature/integrity;
- capability expiry;
- capability scope (`endpoint_id`/`artifact_id`/direction/`transfer_id` match the request exactly);
- proof signature validity;
- the proof's public key matches the capability's bound proof-key thumbprint;
- the proof's capability identifier/hash matches the capability actually presented alongside it;
- the proof's operation/chunk binding matches the request actually being made;
- proof freshness (within the accepted window, see "Replay and freshness semantics");
- proof replay status (`proof_id` not already accepted for this authorization context);
- current durable transfer/Attempt/Artifact authorization state (transfer not terminal, owning Attempt not closed `Indeterminate`, Endpoint credential currently `CredentialActive`).

**Terminology, made explicit and precise**: capability signature verification, and proof signature verification, can each be performed **statelessly** — neither requires a durable lookup to check the cryptography itself. Replay detection uses **bounded transient runtime state** (see below) — not durable storage, but not "nothing" either. The **complete authorization decision is state-aware**, because it additionally revalidates current durable transfer/Attempt/Artifact/credential state on every request. No sentence in this Specification should be read as claiming the complete mechanism is stateless; only the cryptographic verification steps are.

**Fail-closed, non-enumerable denial**: every failure above is denied with a single generic outcome that does not reveal *which* specific check failed, to avoid cross-tenant/cross-Endpoint/cross-Artifact enumeration; the Server may record the specific internal reason in its own audit/diagnostic trail (`docs/specifications/m0-persistence-observability-and-domain-events.md`) without exposing it to the requester. This covers at minimum: authorization absent, malformed, or cryptographically invalid; expired; issued for another `transfer_id`/`artifact_id`/`endpoint_id`/direction than the one presented; a proof signed by a key not matching the capability's bound thumbprint; a proof bound to a different capability; a proof for a different operation/chunk than the request being made; a replayed `proof_id`; a stale (out-of-window) proof; the transfer already terminal; the owning Attempt closed `Indeterminate`; presented against the wrong Server; and the Endpoint's credential no longer `CredentialActive` (explicit `CredentialRevoked` cascades to deny outstanding capabilities for that Endpoint, even before their own expiry — see "Relationship with Agent session lifetime").

**Critical invariant**: authorization renewal, or an expired/renewed capability or proof key, never creates a new logical Artifact and never invalidates already-verified chunks. Capability/key expiry and renewal affect only the authorization layer; `transfer_id`, the chunk manifest, and the chunk-resume logic in "Chunk transfer" above are completely unaffected — a chunk already durably received and matching its manifest digest remains valid and is never re-transferred merely because the security capability or proof key was renewed. Existing ADR-0008 manifest/chunk digest verification remains the sole authoritative mechanism for Artifact/chunk byte integrity; proof of possession authorizes the request, it does not duplicate or replace that integrity check. If a concrete HTTP-framing reason later requires including a request-body digest in the proof, that may be specified at wire-contract implementation time without changing this architecture or replacing existing Artifact digest semantics.

### Replay and freshness semantics

- Each proof carries a unique `proof_id`, unpredictable to anyone who has not seen it generated.
- Proofs are accepted only inside a bounded freshness window measured from `issued_at`. **Exact window duration is implementation-time** (see "Out of scope") — not chosen in this architecture round.
- The Server maintains a bounded **transient** replay cache of accepted `proof_id` values for the applicable authorization context, covering at least the acceptance window. This is high-frequency security/runtime state, per ADR-0007's durable/transient boundary — it is **not** written into the durable domain database merely to survive restart.
- Reuse of an already-accepted `proof_id` for that authorization context fails closed (see "Per-request verification").
- This is intentionally conceptually similar to established proof-of-possession anti-replay patterns (e.g., DPoP's `jti`/`iat` handling) without adopting DPoP, OAuth, or OIDC as a protocol dependency — Bamep's version is scoped to exactly the fields this contract needs.

### Lifetime and scope

- Single `transfer_id`, single direction, single `artifact_id`, single `endpoint_id`, single proof-key thumbprint — never reusable across any of those.
- Reusable across any number of chunk requests belonging to the same transfer, within its validity window — each request still requires its own fresh, unique-`proof_id` proof; only the capability itself is multi-use, never a proof.
- Short-lived and bounded; renewable/reissuable for the same `transfer_id` under the conditions above. **Exact TTL is implementation-time** (see "Out of scope") — this Specification requires only that it be short-lived, bounded, and renewable, not a specific duration.
- Denied for further use, and denied for renewal, once the transfer reaches a terminal state (`Verified` or `Failed`) or its owning Attempt is closed `Indeterminate` (`docs/specifications/m0-job-lifecycle-and-scheduling.md` "Reconciliation and the Indeterminate outcome") with no further Attempt authorized.

### Reconnect and restart behavior

- **WSS disconnect while an HTTPS transfer continues**: the data-plane channel does not depend on the WSS socket remaining open. A temporary WSS disconnect, by itself, does not revoke `CredentialActive`, does not change durable transfer state, does not automatically invalidate an otherwise-valid capability, and does not itself authorize renewal or continuation either. An already-issued, still-valid capability plus a matching proof key remains usable, revalidated per request exactly as above. If Job/Attempt state changes such that continuation is no longer authorized, subsequent HTTP requests fail closed regardless of capability lifetime. If the capability expires before the Agent reconnects Agent Protocol, the Agent reconnects (unchanged existing reconnect handling, `m0-agent-protocol-contract.md` "Reconnect / stale-command handling") and then requests a fresh capability for the same `transfer_id`.
- **Agent reconnect**: standard existing Agent Protocol reconnect and Attempt reconciliation (`AwaitingReconciliation`, `StatusQuery`/`StatusReport`) apply unchanged, independent of data-plane authorization state. Data-plane authorization never substitutes for, or shortcuts, Attempt reconciliation.
- **Agent process restart**: the ephemeral private proof key is intentionally non-durable and is lost — the prior sender-constrained capability becomes unusable by that Agent as a direct consequence, by design; it must not be persisted merely to avoid this flow. The Agent re-authenticates over Agent Protocol, the Server reconciles existing durable transfer/Attempt state, and if continuation remains authorized, the Agent generates a new ephemeral keypair and requests a new capability — the same `transfer_id`, Artifact, and already-verified chunks continue unaffected.
- **Server restart**: outstanding capabilities whose replay-protection continuity cannot be guaranteed (the transient replay cache, "Replay and freshness semantics," does not survive restart) are treated as invalid and must be reissued after the Agent re-establishes the authenticated control-plane context — this is authorization renewal only, and never creates a new `transfer_id`, Artifact, or Attempt, and never implies destructive retry. The implementation must ensure this explicitly (for example, via an authorization epoch, a fresh ephemeral capability-signing context, or an equivalent mechanism) — replay protection must never be silently weakened merely because an in-memory replay cache was lost; a pre-restart capability must not simply resume being accepted once the cache is empty again. The owning Attempt's actual current state (which may itself be `AwaitingReconciliation` after a Server restart, per ADR-0006) governs whether a new authorization is granted — never assumed either way.
- **Attempt `AwaitingReconciliation`**: an outstanding or renewed capability remains usable while the owning Attempt is `AwaitingReconciliation` and not yet closed — reconciliation is reused, not duplicated, by this contract. Once the Attempt is closed `Indeterminate`, or reaches any terminal outcome, further authorization is denied (see "Lifetime and scope").

### Renewal

Capability renewal repeats the `TransferAuthorizationRequest`/`Grant` exchange for the same, still-legitimate (non-terminal) `transfer_id`, and may either:

- reuse the same still-held ephemeral proof key; or
- bind a newly generated ephemeral proof key.

Neither choice changes durable transfer identity, and a new capability must **not**: create a new `transfer_id`; create a new Artifact; discard verified chunks; reset the manifest; imply a new Attempt; or imply destructive retry. Renewal is independent of JobStep/Attempt retry (ADR-0006) — it is not a retry. The Server re-evaluates current durable authorization state before issuing every renewal, exactly as for initial issuance.

### Relationship with Agent session lifetime

A transient WSS control-plane disconnect is **not** authorization revocation — an already-issued, still-valid capability plus matching proof key remains usable for the duration of its own bounded lifetime, revalidated per request as above. This does not make the data plane an indefinitely reusable independent access channel: every capability remains short-lived, single-transfer-scoped, sender-constrained, and revalidated against durable state and replay history on every use. Authenticated Agent identity, current WebSocket presence, transfer authorization, and durable transfer state remain four distinct facts, never conflated: presence can drop without revoking authorization, but authorization can never outlive the durable transfer's own terminal state or an explicit credential revocation, regardless of presence.

### Durable vs. transient authorization state (ADR-0007 boundary)

**Durable**: the transfer's authorization bindings (`endpoint_id`, `transfer_id`, `artifact_id`, direction, `attempt_id`) — recorded once, as part of the same durable transfer record ADR-0008 point 10 already requires, not a separate write; the Server's capability-signing secret (durable Server-side operational/configuration secret, exact storage mechanism implementation-time); an audit record of transfer-authorization issuance where the transfer feeds a destructive JobStep, reusing the already-established destructive-dispatch audit pattern (ADR-0007 point 6) rather than inventing new audit infrastructure.

**Transient**: the individual issued capability itself; the ephemeral proof keypair; the per-request proof-of-possession replay cache (`proof_id` values within the acceptance window). None of these are persisted as durable, reusable rows — the capability and proof are each verified using stateless cryptographic checks plus a durable-state cross-check (never described as making the complete mechanism stateless — see "Per-request verification"), and the replay cache is bounded, high-frequency runtime state, consistent with "do not persist plaintext reusable secrets merely for convenience" (ADR-0007) and with `ActionProgress`'s already-accepted non-durable treatment.

### Threat-model statement

The accepted mechanism protects against:

- passive provisioning-LAN capture, through HTTPS;
- use of a stolen capability by a party that does not possess the bound ephemeral private proof key;
- cross-Endpoint capability substitution;
- cross-transfer/cross-Artifact/cross-direction use;
- straightforward replay of an already-accepted request proof;
- stale/revoked/terminal authorization use;
- Server confused-deputy mistakes covered by the explicit bindings above.

It does **not** claim protection if an attacker compromises the authenticated Agent deeply enough to obtain **both** the valid transfer capability **and** the corresponding ephemeral private proof key — consistent with M0's already-accepted assurance boundary for a fully-compromised Endpoint (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` "M0 threat-model boundary"). This mechanism does not introduce, and should not be read as introducing, any stronger attestation claim than that boundary already accepts.

## Source reproducibility: M0/V1 offline maintenance capture

If a chunk cannot be (re)produced to match its recorded digest — because the underlying source has changed since that chunk was identified, per `docs/reference/transfer-resumability-spike.md` Experiment E — that chunk transfer fails. This Specification requires that failure to be explicit (the chunk, and depending on the artifact's consistency requirements, the artifact itself, moves toward `Failed` — see "Artifact lifecycle"), never silently accepted or approximated, and the recorded digest is never rewritten to fit the changed source (see "Manifest construction and sealing").

**For M0/V1, source consistency is resolved through offline maintenance capture, not a live-Windows backup or a snapshot technology** (ADR-0008 point 5). The accepted V1 flow: the endpoint reboots through PXE into the Linux maintenance environment; the Bamep Agent starts and performs inventory; backup/capture, when requested, happens while the installed Windows OS is **not running**; provisioning/restoration follows afterward.

- **Volume/Image**: the installed Windows OS is not running; the source disk/volume is a non-destructive read source; Bamep must not write to the source merely to perform the safety backup.
- **Selective**: the installed Windows OS is not running; filesystems needed to read selected files are accessed read-only; Bamep must not mount the original source read-write merely to perform the backup.

Snapshot/VSS/live-quiescing technology is **not required** for the normal M0/V1 workflow — the absence of a running OS on the source already removes the concurrent writer. Live backup while the installed Windows OS is running is explicitly outside V1 scope and is not designed here.

**Offline capture does not prove application-level or filesystem-level semantic correctness.** A Windows filesystem may already be dirty, hibernated, or hold unclean-shutdown application state before PXE boot. Offline capture establishes only that Bamep captured a stable source with no concurrent writer during capture — see "Capture/source-consistency fact" below for what this does and does not certify.

## Capture/source-consistency fact

`Verified` (see "Artifact lifecycle") means cryptographic integrity only. It is **not** redefined to mean capture consistency. A separate, independent durable fact on the Artifact:

`capture_consistency: NotApplicable | NotEstablished | Established`

- `NotApplicable` — the Artifact is not a capture of mutable client state for which source-writer consistency is a meaningful question.
- `NotEstablished` — the Artifact is a capture of mutable client state, and the maintenance workflow has not positively confirmed the offline/read-only source conditions above held for its duration.
- `Established` — the maintenance workflow has **positively confirmed** those conditions held for the capture's duration. This is an explicit, recorded confirmation, never a default.

`capture_consistency == Established` means "the bytes belong to a stable capture under the declared capture conditions" — it does **not** mean "the filesystem/application state was logically healthy."

**A critical backup gating a destructive provisioning operation must satisfy both, when the Artifact type requires capture consistency**: `Verified` **and** `capture_consistency == Established`. A `Verified` backup whose `capture_consistency` is `NotEstablished` must not authorize the destructive step. This is additive to, not a replacement of, the `Verified`-only check below.

The exact mechanism/point at which the maintenance workflow positively confirms these conditions (e.g., which component asserts it, and how) is not designed by this Specification.

## Artifact lifecycle

States: `Incomplete`, `PendingVerification`, `Verified`, `Failed`.

Transitions:

- `(created)` → `Incomplete`: an artifact capture/transfer begins; the manifest may still be under construction (see "Manifest construction and sealing").
- `Incomplete` → `PendingVerification`: the manifest is **sealed**, every expected chunk is present, and each has individually passed its chunk digest check.
- `PendingVerification` → `Verified`: the full-artifact digest (`artifact_digest`) is computed over the reassembled content and matches the manifest. This transition is atomic and is the point at which the artifact becomes visible/usable to any consumer (ADR-0008 point 6) — no partially-written or not-yet-verified artifact is ever observable as complete.
- `PendingVerification` → `Failed`: the full-artifact digest does not match, despite every individual chunk having passed its own check (e.g., chunk-ordering error, manifest corruption). This is distinguished from a chunk-level failure for diagnostic purposes.
- `Incomplete` → `Failed`: a required chunk cannot be (re)produced to match its manifest digest (source-reproducibility boundary above), or the capture is otherwise abandoned/cancelled per the owning JobStep's cancellation (ADR-0006).
- `Verified` and `Failed` are terminal for that artifact. A `Failed` artifact is not retried in place; a new capture/transfer attempt (a new Attempt on the owning JobStep, per ADR-0006's retry policy) produces a new artifact.

**An Artifact is an atomic integrity/completeness unit.** Failure of any single required chunk — chunk-level digest mismatch, or a chunk that cannot be reproduced/verified per "Source reproducibility" above — means that Artifact as a whole cannot become `Verified`; it becomes `Failed`. There is no partial success within one Artifact: a `Failed` Artifact is never treated as partially usable, and no subset of its chunks is ever exposed to a consumer independent of the whole. This is definitional for M0, not an open question: the manifest sealing and lifecycle rules above already establish it, and this paragraph only makes it explicit.

If a future Selective-backup workflow is composed of multiple independent Artifacts (for example, one Artifact per selected file), those Artifacts may succeed or fail independently of one another — that is independent-Artifact behavior, not partial recovery of any single Artifact, and whether a Job/workflow may accept such partial success across multiple Artifacts is a future workflow-policy question. This Specification does not redesign Selective backup or ADR-0006 to answer it.

Only a `Verified` artifact may be consumed by a destructive operation, and — where the Artifact type requires capture consistency — only one whose `capture_consistency` is `Established` (see "Capture/source-consistency fact"). A destructive JobStep's precondition revalidation (ADR-0006 "Revalidation immediately before dispatch") must include checking both facts for any artifact it depends on, at dispatch time, not merely when the JobStep was first evaluated.

**These two Artifact-specific gates (`Verified`, `capture_consistency == Established`) are additive to, and never replace or narrow, the complete destructive-operation precondition set owned by `docs/specifications/m0-endpoint-identity-lifecycle.md` and composed by `docs/specifications/m0-job-lifecycle-and-scheduling.md` "Destructive dispatch preconditions."** That base set now includes trusted current bootstrap context (precondition 7, `docs/decisions/0010-trusted-bootstrap-and-secure-boot-baseline.md`), alongside the six preconditions already in force. A `Verified` Artifact whose `capture_consistency` is `Established` does not by itself authorize destructive use — every base precondition, including trusted bootstrap, must independently hold at dispatch time. This Specification does not restate that full list; see the two Specifications above for its authoritative content.

## Artifact source provenance and multi-disk endpoints

An Endpoint is not modeled as having one implicit disk — Bamep supports endpoints with multiple physical disks and/or volumes (e.g., an NVMe/SSD holding Windows plus an HDD holding user data, multiple data disks, or an old HDD as a capture source while a new SSD/NVMe becomes the provisioning target).

An Artifact's **source provenance** identifies the concrete disk/volume/filesystem it was captured from, not merely the Endpoint that owns it. This Specification requires that correlation to be preservable at the contract/Domain level; it does not define the exact schema or field set for the provenance record.

**Artifact source provenance is not the same fact as future destructive-target identity** — see "Source identity vs. target-disk identity" below.

## Source identity vs. target-disk identity

A valid Bamep workflow: an old HDD is backed up offline; the disk is physically replaced; a new SSD/NVMe is installed; inventory is revalidated; the new disk is provisioned; the retained data is restored onto it.

- Restoring/migrating retained data must **not** require the destination disk's fingerprint to equal the source Artifact's source-disk fingerprint.
- **Source identity** answers "where did these bytes come from?" (Artifact provenance, above).
- **Target-disk identity** answers "which currently installed disk is the destructive Job authorized to modify?" — the existing target-disk identity/fingerprint revalidation already required immediately before destructive dispatch (`docs/specifications/m0-endpoint-identity-lifecycle.md`; ADR-0006 "Revalidation immediately before dispatch"). This Specification does not weaken that safety invariant in any way — the new target disk must still satisfy it, in full, immediately before execution.

A disk replacement may legitimately change an Endpoint's observed hardware inventory. The full planned-hardware-change authorization mechanism is not designed here — it remains for the identity Work Package's own model to eventually extend — but an operator-authorized disk replacement is recorded as a valid use case that must not be automatically interpreted as a different Endpoint solely because the disk changed.

## Storage capability model

A Storage Target exposes:

- `roles` — a **set** of `SYSTEM`, `CACHE`, `ARCHIVE` (already-accepted vocabulary), not a single mutually-exclusive value — one physical location may satisfy multiple roles simultaneously (already accepted).
- `available_capacity` — for scheduling (Attempt-scoped storage leases, ADR-0006).
- read/write throughput characteristics relevant to scheduling — exact fields are implementation-time, not enumerated exhaustively here.

Storage Targets are addressed through the `storage` Port (`docs/specifications/m0-stack-and-boundaries-baseline.md`); Domain and Application code never assume a RAID layout, filesystem, or raw device name — those are Adapter concerns.

### Role usage semantics

- **`SYSTEM`**: storage required for Bamep's own operational durable state (persistence/configuration, ADR-0007). Not implicitly the preferred bulk-artifact target unless the same Storage Target also exposes another applicable role.
- **`CACHE`**: optional working/staging/performance-oriented artifact storage. May hold `Incomplete` artifacts and additional copies of completed artifacts. Must not be assumed to be the sole retained copy when an artifact's retention requirement calls for durable preservation.
- **`ARCHIVE`**: optional storage eligible for retained completed/`Verified` artifacts.

**Verification and retention are independent.** `Verified` is a property of an artifact's content (its digest matches), established once, independent of where or how many copies exist. Placing a copy in `ARCHIVE` is a retention/placement decision, not a substitute for cryptographic verification, and does not itself make an unverified artifact `Verified`. A `Verified` artifact is not automatically archived — retention placement is a separate decision not made here.

Migration mechanics between roles, multi-copy consistency, and retention-duration policy are not defined by this Specification — implementation-time or future-work concerns.

## Volume/Image vs. Selective backup

- **Volume/Image**: one artifact per capture, chunked at fixed byte boundaries directly over the linear disk/volume byte range (ADR-0008 point 8).
- **Selective**: file-granular — each selected file is its own artifact (or its own unit within a larger selective-backup artifact), using the same Artifact lifecycle above; a large individual file may apply the same chunking mechanism internally. **This is a design candidate informed by the chunking evidence, not an empirically tested finding** (`docs/reference/transfer-resumability-spike.md`) — per-file behavior has not been exercised. Individual files can change during capture and are subject to the same source-reproducibility boundary as Volume/Image; per-file granularity does not exempt Selective backup from it.

## Out of scope

- exact chunk size — implementation-time tuning;
- the digest algorithm (`digest_algorithm` value) — an implementation/interoperability decision fixed before the concrete wire contract, not chosen here;
- live-Windows backup and any snapshot/VSS/live-quiescing technology — explicitly outside V1 scope, not designed here;
- the exact mechanism/component that positively confirms offline/read-only capture conditions to set `capture_consistency = Established` — not designed here;
- exact transfer-capability TTL, proof-freshness window duration, concrete signature/wire/serialization formats, the concrete asymmetric algorithm for the ephemeral proof key, and HTTP-level details (header names, status codes) — implementation-time, consistent with the pattern already established for `digest_algorithm` and chunk size; the sender-constrained mechanism, bindings, issuance sequence, proof-of-possession fields, replay semantics, and revocation semantics themselves are accepted (see "Transfer-session authentication");
- the planned-hardware-change (disk-replacement) authorization mechanism — not designed here, remains for the identity Work Package's model;
- exact schema/field set for Artifact source-provenance records — not decided here;
- final production backup/snapshot format — explicitly out of M0 scope;
- RAID/filesystem/device layout, exact database schema — implementation-time, not defined here;
- HTTP wire-level details (headers, status codes, request/response framing) beyond "one chunk per request-response unit" — implementation-time;
- domain-event catalog additions for artifact/transfer events — owned by `docs/specifications/m0-persistence-observability-and-domain-events.md`'s existing extensible catalog (Issue #5), not redefined here;
- the future pre/post provisioning diagnostics workflow — recorded as Discovery/product context (`docs/discovery/architecture-redesign.md`), not part of this contract.

## Acceptance criteria

- Data-plane transport/resumability strategy is defined and grounded in the Spike's evidence (Issue #6 acceptance criterion).
- Storage capability model and artifact lifecycle invariants are defined (Issue #6 acceptance criterion).
- Destructive operations have a specified safety invariant tying execution to artifact `Verified` state and, where applicable, `capture_consistency == Established` (Issue #6 acceptance criterion).
- Multi-disk source provenance and the independence of source identity from destructive-target identity are represented (owner decision, disk-replacement use case).
- Transfer-session authentication is fully specified — bindings, mechanism, TLS requirement, issuance sequence, lifetime/renewal, revocation/fail-closed behavior, and durable/transient state split (Issue #15 acceptance criterion; ADR-0008 point 9).

## Validation expectations

Per `docs/development/testing.md` "Data transfer and artifact tests": interrupted transfer (partial chunk set); incomplete `.part`/`Incomplete`-state artifact rejected by any destructive consumer; digest mismatch at chunk level and at full-artifact level; duplicate chunk transfer request (idempotent, no double-write); storage exhaustion during chunk write; producer/consumer disconnect mid-chunk; restart behavior (an `Incomplete` artifact survives restart as `Incomplete`, consistent with ADR-0007's durable state); atomic `PendingVerification` → `Verified` commit; failed verification before destructive provisioning proceeds.

Per `docs/development/testing.md` "Unit and domain tests": Artifact lifecycle state-transition tests (valid and rejected transitions); chunk-manifest verification logic (chunk digest, full-artifact digest) as pure domain tests, decoupled from actual network transfer.

Per `docs/development/testing.md` "Simulator": chunked transfer at the M0 20–24 concurrent-endpoint target, including interrupted/corrupted-chunk scenarios and a simulated source-mutation scenario reproducing `docs/reference/transfer-resumability-spike.md` Experiment E's finding (a missing chunk that cannot be honestly regenerated must be reported as failed, never silently substituted); a destructive JobStep must be rejected when `capture_consistency` is `NotEstablished` even if the artifact is `Verified`; a simulated disk-replacement scenario (source Artifact provenance from one disk identity, destructive target a different, newly installed disk identity) must succeed without requiring the two to match; a destructive JobStep must also be rejected when the Artifact is `Verified` and `capture_consistency` is `Established` but the independent trusted-bootstrap precondition (`docs/specifications/m0-endpoint-identity-lifecycle.md` precondition 7) is not established — a fully valid, verified Artifact never by itself authorizes destructive use.

Per `docs/development/testing.md` "Simulator" and "Security-negative tests" (Transfer-session authentication, Issue #15), at minimum: a valid capability with a valid matching proof accepted when durable state authorizes; a valid capability presented with no proof rejected; a valid capability presented with a proof signed by the wrong key rejected; a capability "stolen" by another Simulated Endpoint (presented without possession of the bound private proof key) rejected; a proof bound to a different capability than the one presented rejected; a proof for a different chunk/operation/direction than the request actually made rejected; replay of an already-accepted `proof_id` rejected; a stale (out-of-freshness-window) proof rejected; a capability for another Endpoint rejected; a capability for another `transfer_id` rejected; a capability for another Artifact rejected; a capability presented for the wrong direction rejected; an expired or explicitly-revoked-via-`CredentialRevoked` capability rejected; a legitimately interrupted transfer obtaining a renewed capability (with either a reused or a freshly generated proof key) and resuming without re-transferring already-verified chunks and without a new `transfer_id`; a WSS reconnect that does not, by itself, grant, revoke, or imply a new authorization; a simulated Server restart, after which the old capability is rejected and legitimate reauthorization continues the same `transfer_id` and verified chunks; a simulated Agent restart, after which the old key/capability are unusable and a legitimate new key/capability continue the same durable transfer; all of the above exercised concurrently across 20–24 Simulated Endpoints, each retaining an isolated sender-constrained transfer-authorization context. None of these scenarios require real Secure Boot or physical hardware. Per the Simulator's already-accepted real-transport fidelity rule (`docs/specifications/m0-simulator-contract-and-validation-strategy.md` "Simulator fidelity boundary"), these scenarios exercise the real `TransferAuthorizationRequest`/`Grant`/`Denied` messages, real ephemeral proof-key generation, and real per-request proof-of-possession and revalidation — the Simulator must not bypass transfer authorization merely because it is a Simulator.

Per `docs/development/testing.md` "Contract tests" (Transfer-session authentication): `TransferAuthorizationRequest`/`Grant`/`Denied` serialization per the wire-encoding conventions in `m0-agent-protocol-contract.md`, including the proof public key field; a request for an unknown or another Endpoint's `transfer_id` denied without revealing which case applied; capability signature verification — valid accepted, tampered/invalid rejected; proof signature verification — valid accepted, tampered/invalid rejected, wrong-key rejected; proof structure field-binding checks (discriminator/version, capability identity, operation, chunk identity, `proof_id`, `issued_at`) — as contract-level negative cases, without selecting a concrete signing algorithm, serialization, or library here.

Per "Local development environments," these are expected to run in the Linux reference environment (WSL2 or containers from Windows).

Manual: owner approval of this Specification — confirmed (see Status), including the sender-constrained transfer-authorization design (owner security review, Issue #15). Remaining open items (chunk size, `digest_algorithm` selection, live-Windows backup consistency, the concrete mechanism establishing `capture_consistency = Established`, exact transfer-capability TTL/proof-freshness window/wire format, disk-replacement authorization, and Artifact source-provenance schema) are explicitly non-blocking implementation/future-work detail, not unresolved architecture.

## Related ADRs

- ADR-0008 — Data-plane transport, chunking, and resumability strategy (`Accepted`), including point 9's transfer-session authentication decision this Specification details.
- ADR-0004 — Endpoint identity (destructive-operation preconditions consuming artifact `Verified`/`capture_consistency` state; target-disk identity revalidation independent of Artifact source provenance; `CredentialActive`/`CredentialRevoked` transfer-authorization revalidation checks against).
- ADR-0006 — Job/JobStep/Attempt model (revalidation before dispatch consuming artifact state; retry policy for `Failed` artifacts; Attempt reconciliation reused by transfer-authorization lifetime).
- ADR-0007 — Persistence backend and durable/transient boundary (artifact/manifest durability; `transfer_id` correlation; durable-vs-transient split for transfer-authorization state).
- ADR-0010 / ADR-0011 — Trusted bootstrap and site trust-anchor baseline (`Accepted`) — source of the trusted-bootstrap precondition this Specification's Artifact gates are additive to; also source of the pinned Server TLS identity the data-plane HTTPS requirement reuses.

## Related work

- Issue #6 — `[WP] Define data-plane and storage contracts`.
- Issue #9 — `[Spike] Evaluate resumable volume/image transfer` (evidence this Specification applies).
- Issue #8 — `[Spike] Validate WinPE boot mechanism` (validates the boot mechanism only; may later constrain, but does not itself resolve, the open source-consistency requirement).
- Issue #15 — `[WP] Define authenticated data-plane transfer-session binding` (resolves transfer-session authentication; see "Transfer-session authentication").
- Issue #2 / ADR-0004, Issue #3 / ADR-0005 — constrain, and (with Issue #15) now resolve, transfer-session authentication; also own target-disk identity revalidation and the Endpoint identity model any future disk-replacement authorization would extend.
- Issue #4 / ADR-0006 — Attempt model; destructive-operation revalidation.
- Issue #5 / ADR-0007 — persistence of artifact/manifest state.
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (must simulate this contract, including transfer-authorization scenarios).
- Issue #10 / ADR-0010, Issue #13 / ADR-0011 — trusted bootstrap and site trust-anchor establishment (complete; source of the trusted-bootstrap precondition this Specification's Artifact-specific gates are additive to, and of the pinned Server TLS identity transfer-session authentication reuses).

## Open questions

1. Exact chunk size — implementation-time tuning.
2. Digest algorithm (`digest_algorithm` value) — implementation/interoperability decision, not chosen here.
3. Live-Windows backup consistency mechanism — explicitly out of V1 scope; a future architecture decision if ever pursued.
4. The exact mechanism/component that positively confirms offline/read-only capture conditions (`capture_consistency = Established`) — not designed here.
5. Exact transfer-capability TTL, proof-freshness window duration, concrete signature/wire/serialization formats, the asymmetric algorithm for the ephemeral proof key, and HTTP-level details — implementation-time; the sender-constrained mechanism itself is accepted (see "Transfer-session authentication").
6. Planned-hardware-change (disk-replacement) authorization mechanism — not designed here, remains for the identity Work Package's model.
7. Exact schema/field set for Artifact source-provenance records — not decided here.

None of the above are blocking for owner approval of Issue #6 or Issue #15 — each is explicitly deferred implementation/future-work detail, not an unresolved architectural fork.

Status: Approved.
