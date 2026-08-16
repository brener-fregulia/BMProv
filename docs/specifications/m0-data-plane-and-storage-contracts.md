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

Accepted (ADR-0008 point 9, executing Issue #15): every data-plane transfer is authorized and authenticated by a **short-lived, transfer-scoped, Server-signed bearer capability**, delivered over the already-authenticated Agent Protocol control-plane channel and presented on the HTTPS data-plane channel. This section is the full operational contract; ADR-0008 records the decision and its rationale.

### Transport

The data plane is **HTTPS**, not plain HTTP. Server identity reuses the same pinned Server TLS certificate/fingerprint already authenticated for the Agent Protocol WSS connection via trusted bootstrap (`docs/decisions/0010-trusted-bootstrap-and-secure-boot-baseline.md`, `docs/decisions/0011-site-trust-anchor-operator-verified-pairing.md`) — no second trust relationship is introduced, and no new site trust-anchor question is reopened. The Agent does not present a client certificate; the data plane is not mTLS, consistent with Agent Protocol (`m0-agent-protocol-contract.md` "Transport and handshake").

### Authorization bindings

One authorization capability is bound to exactly:

- `endpoint_id`;
- `transfer_id`;
- `artifact_id`;
- direction (Agent → Server, or Server → Agent);
- `attempt_id` of the transfer JobStep's Attempt (`docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md`) that caused it to be issued.

No other identifier is bound merely because it exists — `job_id`/`jobstep_id` are reachable transitively through `attempt_id` and are not separately embedded. A capability authorizes only the exact `(endpoint_id, transfer_id, artifact_id, direction)` tuple it was issued for; it never authorizes another transfer, another Artifact, the opposite direction, or another Endpoint, even for the same Agent session.

### Issuance sequence

```text
1. Agent is authenticated over Agent Protocol (SessionEstablished,
   BootstrapEvidence already sent) — unchanged.
2. The transfer JobStep's Attempt is dispatched normally via
   ActionDispatch{action_id, action_type, parameters: {transfer_id,
   artifact_id, direction, ...}} — unchanged Job/Attempt dispatch flow
   (ADR-0006, ADR-0007 persist-before-send), no special-casing.
3. Agent sends ActionAck{outcome: Accepted}.
4. Agent sends TransferAuthorizationRequest{transfer_id} over the
   already-authenticated control-plane connection.
5. Server checks: does a non-terminal Attempt exist for this
   transfer_id, bound to the requesting Endpoint's identity/session,
   and is the Endpoint's credential CredentialActive?
     - yes → TransferAuthorizationGrant{transfer_id, token, expires_at}
     - no  → TransferAuthorizationDenied{transfer_id, reason}
6. Agent opens the HTTPS data-plane connection and presents the token
   with each chunk request alongside transfer_id/artifact_id.
7. Server revalidates the token and current durable state on every
   chunk request (see "Chunk-request revalidation" below); on success,
   chunk transfer proceeds exactly as already specified in "Chunk
   transfer" above.
8. If the token expires, or the Agent no longer holds it (e.g. after
   an Agent process restart) while the underlying transfer remains
   legitimately active, the Agent repeats step 4 for the same
   transfer_id — a renewal, never a new Attempt and never a new
   transfer_id.
```

`TransferAuthorizationRequest`/`TransferAuthorizationGrant`/`TransferAuthorizationDenied` are new, strictly additive Agent Protocol v1 message types — see `docs/specifications/m0-agent-protocol-contract.md` "Transfer authorization". Their addition does not reopen WSS, pinned TLS, `AuthRequest`/`SessionEstablished`, or `BootstrapEvidence`.

### Mechanism: why a signed capability, not the long-lived credential

The capability is **self-verifying** (cryptographically signed by a Server-held signing secret) rather than looked up in a persisted session table, and is **never itself persisted as a durable, reusable secret** — only the *durable transfer binding* it is checked against (the bindings above, already part of the durable transfer record per ADR-0008 point 10) is durable. Every use is revalidated against that durable state, not merely against the token's own signature and expiry — this is what makes real-time revocation possible (a cancelled transfer, or a revoked credential, is denied immediately, even before the token's natural expiry) without a persisted blacklist.

It is deliberately **not** derived from the Endpoint's long-lived Agent runtime credential: that would grant authority disproportionate to one transfer, and could not be revoked without collateral damage to the Endpoint's entire Agent session. See ADR-0008 "Alternatives considered" for the full evaluation against the rejected alternatives.

### Lifetime and scope

- Single `transfer_id`, single direction, single `artifact_id`, single `endpoint_id` — never reusable across any of those.
- Reusable across any number of chunk requests belonging to the same transfer, within its validity window — not single-use-per-chunk.
- Short-lived and bounded; renewable/reissuable for the same `transfer_id` under the conditions above. **Exact TTL is implementation-time** (see "Out of scope") — this Specification requires only that it be short-lived, bounded, and renewable, not a specific duration.
- Denied for further use, and denied for renewal, once the transfer reaches a terminal state (`Verified` or `Failed`) or its owning Attempt is closed `Indeterminate` (`docs/specifications/m0-job-lifecycle-and-scheduling.md` "Reconciliation and the Indeterminate outcome") with no further Attempt authorized.

### Chunk-request revalidation

Every chunk request — not only the first — is revalidated against **current durable state**, in addition to the token's own signature/expiry check:

- the transfer is not in a terminal state;
- the `endpoint_id`/`artifact_id`/direction match the token's bindings exactly;
- the Endpoint's credential is currently `CredentialActive` (not `CredentialRevoked`).

**Critical invariant**: authorization renewal, or an expired/renewed token, never creates a new logical Artifact and never invalidates already-verified chunks. Token expiry and renewal affect only the authorization layer; `transfer_id`, the chunk manifest, and the chunk-resume logic in "Chunk transfer" above are completely unaffected — a chunk already durably received and matching its manifest digest remains valid and is never re-transferred merely because the security token was renewed.

### Reconnect and restart behavior

- **WSS disconnect while an HTTP(S) transfer continues**: the data-plane channel does not depend on the WSS socket remaining open. An already-issued, still-valid token remains usable. If the token expires before the Agent reconnects Agent Protocol, the Agent reconnects (unchanged existing reconnect handling, `m0-agent-protocol-contract.md` "Reconnect / stale-command handling") and then requests a fresh token for the same `transfer_id`.
- **Agent reconnect**: standard existing Agent Protocol reconnect and Attempt reconciliation (`AwaitingReconciliation`, `StatusQuery`/`StatusReport`) apply unchanged, independent of data-plane token state. A data-plane token never substitutes for, or shortcuts, Attempt reconciliation.
- **Agent process restart**: any token held only in Agent memory is lost; whether the Agent persists a token to disposable local staging state is implementation-time, not decided here. Once Agent Protocol reconciliation has re-established what the Agent's own local state is for the owning Attempt, if the transfer is still legitimate, the Agent requests a fresh token for the same `transfer_id`.
- **Server restart**: does not invalidate outstanding, unexpired tokens by itself — verification is a signature check (using a Server-durable signing secret, not an in-memory-only session table) plus a durable-state lookup, both of which survive restart. The owning Attempt's actual current state (which may itself be `AwaitingReconciliation` after a Server restart, per ADR-0006) governs whether further use is still authorized — never assumed either way.
- **Attempt `AwaitingReconciliation`**: an outstanding or renewed token remains usable while the owning Attempt is `AwaitingReconciliation` and not yet closed — reconciliation is reused, not duplicated, by this contract. Once the Attempt is closed `Indeterminate`, or reaches any terminal outcome, further authorization is denied (see "Lifetime and scope").

### Revocation and fail-closed behavior

Every case below fails closed, denying the request with a single generic outcome that does not reveal *which* specific check failed — this Specification does not distinguish these cases on the wire, to avoid cross-tenant/cross-Endpoint/cross-Artifact enumeration; the Server may record the specific internal reason in its own audit/diagnostic trail (`docs/specifications/m0-persistence-observability-and-domain-events.md`) without exposing it to the requester:

- authorization absent, malformed, or cryptographically invalid;
- expired;
- issued for another `transfer_id`, `artifact_id`, `endpoint_id`, or direction than the one presented;
- the transfer is already terminal;
- the owning Attempt has been closed `Indeterminate` with no further Attempt authorized;
- presented against the wrong Server (signature does not verify against this Server's signing secret);
- the Endpoint's credential is no longer `CredentialActive` (explicit `CredentialRevoked` cascades to deny outstanding tokens for that Endpoint, even before their own expiry — see "Relationship with Agent session lifetime").

### Relationship with Agent session lifetime

A transient WSS control-plane disconnect is **not** authorization revocation — an already-issued, still-valid token remains usable for the duration of its own bounded lifetime, revalidated per request as above. This does not make the data plane an indefinitely reusable independent access channel: every token remains short-lived, single-transfer-scoped, and revalidated against durable state on every use. Authenticated Agent identity, current WebSocket presence, transfer authorization, and durable transfer state remain four distinct facts, never conflated: presence can drop without revoking authorization, but authorization can never outlive the durable transfer's own terminal state or an explicit credential revocation, regardless of presence.

### Durable vs. transient authorization state (ADR-0007 boundary)

**Durable**: the transfer's authorization bindings (`endpoint_id`, `transfer_id`, `artifact_id`, direction, `attempt_id`) — recorded once, as part of the same durable transfer record ADR-0008 point 10 already requires, not a separate write; the Server's token-signing secret (durable Server-side operational/configuration secret, exact storage mechanism implementation-time); an audit record of transfer-authorization issuance where the transfer feeds a destructive JobStep, reusing the already-established destructive-dispatch audit pattern (ADR-0007 point 6) rather than inventing new audit infrastructure.

**Transient**: the individual issued token/capability itself. It is never separately persisted as a durable, reusable row — it is verified statelessly (signature + durable-state cross-check) at the moment of each use, consistent with "do not persist plaintext reusable secrets merely for convenience" (ADR-0007).

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
- exact transfer-authorization token TTL, concrete signature/wire format, and HTTP-level details (header names, status codes) — implementation-time, consistent with the pattern already established for `digest_algorithm` and chunk size; the mechanism, bindings, issuance sequence, and revocation semantics themselves are accepted (see "Transfer-session authentication");
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

Per `docs/development/testing.md` "Simulator" and "Security-negative tests" (Transfer-session authentication, Issue #15): a valid authorized transfer completing normally; a chunk request with missing authorization rejected; a token for another Endpoint rejected; a token for another `transfer_id` rejected; a token for another Artifact rejected; a token presented for the wrong direction rejected; an expired or explicitly-revoked-via-`CredentialRevoked` token rejected; replay of a token after its owning transfer reached a terminal state rejected; a legitimately interrupted transfer obtaining a renewed token and resuming without re-transferring already-verified chunks and without a new `transfer_id`; a WSS reconnect that does not, by itself, grant or imply a new authorization; all of the above exercised concurrently across 20–24 Simulated Endpoints with independent transfer authorization. Per the Simulator's already-accepted real-transport fidelity rule (`docs/specifications/m0-simulator-contract-and-validation-strategy.md` "Simulator fidelity boundary"), these scenarios exercise the real `TransferAuthorizationRequest`/`Grant`/`Denied` messages and real per-request revalidation — the Simulator must not bypass transfer authorization merely because it is a Simulator.

Per `docs/development/testing.md` "Contract tests" (Transfer-session authentication): `TransferAuthorizationRequest`/`Grant`/`Denied` serialization per the wire-encoding conventions in `m0-agent-protocol-contract.md`; a request for an unknown or another Endpoint's `transfer_id` denied without revealing which case applied; token signature verification — valid accepted, tampered/invalid rejected — as contract-level negative cases, without selecting a concrete signing algorithm or library here.

Per "Local development environments," these are expected to run in the Linux reference environment (WSL2 or containers from Windows).

Manual: owner approval of this Specification — confirmed (see Status). Remaining open items (chunk size, `digest_algorithm` selection, live-Windows backup consistency, the concrete mechanism establishing `capture_consistency = Established`, exact transfer-authorization token TTL/wire format, disk-replacement authorization, and Artifact source-provenance schema) are explicitly non-blocking implementation/future-work detail, not unresolved architecture.

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
5. Exact transfer-authorization token TTL, concrete signature/wire format, and HTTP-level details — implementation-time; the mechanism itself is accepted (see "Transfer-session authentication").
6. Planned-hardware-change (disk-replacement) authorization mechanism — not designed here, remains for the identity Work Package's model.
7. Exact schema/field set for Artifact source-provenance records — not decided here.

None of the above are blocking for owner approval of Issue #6 or Issue #15 — each is explicitly deferred implementation/future-work detail, not an unresolved architectural fork.

Status: Approved.
