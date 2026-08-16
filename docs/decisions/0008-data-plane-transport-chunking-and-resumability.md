# ADR-0008: Data-plane transport, chunking, and resumability strategy

Status: Accepted

## Context

M0 requires resolving the data-plane transfer contract and storage capability model (`docs/discovery/adr-triage.md` candidates 8, 9, 10; `docs/specifications/m0-architecture-baseline.md` scope item "data-plane contract"). Issue #6 executes this Work Package.

`docs/discovery/architecture-redesign.md` "Data plane": large transfers remain separate from the control plane; "HTTP streaming or chunk-oriented transfer is a strong direction"; "resumability must not be faked through byte offsets when the source cannot reproduce the stream from an arbitrary offset."

`docs/discovery/architecture-redesign.md` "Backup model": every completed artifact requires metadata, expected size when applicable, a cryptographic digest, explicit incomplete state, atomic completion/commit semantics, and an explicit verification state; Volume/Image backup and Selective backup are minimum strategies to specify independently; no generic `backup=true` semantic.

Issue #6 was explicitly blocked from reaching a final Accepted state on resumability until Issue #9 (`[Spike] Evaluate resumable volume/image transfer`) produced evidence. That Spike is complete and its findings, confirmed by the owner, are recorded in `docs/reference/transfer-resumability-spike.md`. This ADR applies that evidence rather than re-deriving it.

## Decision

### 1. Data-plane is a separate channel from the control plane

Large transfers use a dedicated HTTP-based data-plane channel, distinct from the Agent Protocol WebSocket control-plane connection (ADR-0005). The control plane carries only transfer-related progress metadata (`ActionProgress`) and correlation; transfer bytes never flow over the WebSocket connection. This formalizes the already-accepted separation in `docs/discovery/architecture-redesign.md` "Data plane."

### 2. Chunk-oriented transfer, not raw streaming or byte-offset resume

Transfers are **chunk-oriented**: an artifact is divided into fixed-size chunks, each transferred as an independently addressable, independently verifiable unit, with a manifest recording each chunk's index and cryptographic digest. This is the direct architectural application of the evidence in `docs/reference/transfer-resumability-spike.md`:

- byte-offset resume (plain HTTP Range semantics against a continuous artifact stream) is rejected as the general mechanism, because it is only honest when the underlying bytes are already guaranteed reproducible at that offset (Spike Experiment A) — which cannot be assumed in general (Spike Experiments B, E);
- chunking with a per-chunk digest manifest provides honest integrity verification and selective retransmission (Spike Experiments C, D) and is therefore the accepted mechanism.

Chunk-oriented transfer applies **symmetrically** to both directions Bamep needs — Agent → Server (backup/capture) and Server → Agent (provisioning/restore) — using the same chunk/manifest/verification pattern; no concrete requirement was found for a direction-specific mechanism.

### 3. Cryptographic digest is required; the algorithm is not selected here

Every chunk, and the complete Artifact, requires a cryptographic digest (`docs/discovery/architecture-redesign.md` "Backup model"). The manifest/contract carries an explicit `digest_algorithm` identifier so the digest is unambiguous to any implementation reading it — this ADR does not itself choose or benchmark a specific algorithm (e.g., SHA-256 vs. BLAKE3). No accepted M0 evidence currently selects one: `docs/reference/transfer-resumability-spike.md` used SHA-256 for tooling convenience in its experiments and explicitly did not evaluate it against alternatives, and ADR-0004 does not define a digest algorithm for disk identity/fingerprint revalidation either. Algorithm selection remains an implementation/interoperability decision that must be fixed before the concrete Agent/Server wire contract is implemented, informed by whatever throughput or ecosystem evidence is available at that time — not decided by this Work Package absent that evidence.

### 4. Chunk size is not fixed by this ADR

Exact chunk size is implementation-time tuning (trade-off between manifest overhead, re-transfer granularity, and hashing cost), consistent with the Spike's own finding that chunk size "was not tuned or evaluated for a trade-off." This ADR fixes the mechanism (fixed-size chunking with a digest manifest), not the parameter.

### 5. Resumability is bounded by source reproducibility — resolved for M0/V1 via offline maintenance capture; live capture remains open

Per the Spike's confirmed conclusion: chunking and per-chunk digests detect missing or changed chunks and allow selective retransmission, but do **not** by themselves make a changed source reproducible. Spike Experiment E demonstrated directly that a chunk regenerated from a since-mutated source correctly *fails* the original manifest — the artifact cannot be completed from that changed source, only correctly rejected. A source-consistency mechanism is therefore required.

**For M0/V1, the owner has resolved this: the normal Bamep backup workflow is not a live-Windows backup.** The accepted V1 flow is:

1. the endpoint boots its installed Windows environment only when explicitly required;
2. for maintenance/provisioning, the endpoint reboots through PXE;
3. the Linux maintenance environment boots;
4. the Bamep Agent starts and performs inventory;
5. backup/capture, when requested, is performed while the installed Windows OS is **not running**;
6. provisioning/restoration follows afterward.

Volume/Image and Selective backup are, for M0/V1, **offline maintenance capture operations**. The consistency guarantee rests on the source being non-writable by the installed OS during capture — there is no concurrent writer to race against, because the OS that would write to it is not running:

- **Volume/Image**: the installed Windows OS is not running; the source disk/volume is treated as a non-destructive read source; Bamep must not write to the source filesystem/device merely to perform the safety backup.
- **Selective backup**: the installed Windows OS is not running; filesystems required for reading selected files are accessed read-only from the maintenance environment; Bamep must not mount the original source read-write merely to perform the backup.

**This means snapshot/VSS/live-quiescing technology is not required for the normal M0/V1 backup workflow.** Live backup while the installed Windows OS is running is explicitly outside V1 scope and would require a future architecture/consistency decision this ADR does not make and does not design.

**Offline capture does not prove application-level or filesystem-level semantic correctness.** A Windows filesystem may already be dirty, hibernated, or contain application data from an unclean shutdown *before* PXE boot. Offline capture establishes only that Bamep captured a stable source with no concurrent writer during the capture itself — it says nothing about whether the captured filesystem/application state was logically healthy at the moment capture began. This distinction is load-bearing for point 5a below (capture consistency is a narrower fact than "the backup is good").

Therefore:

- an artifact's manifest is valid only for chunks whose source remained stable for the duration of that artifact's capture — for M0/V1, "stable" is established by the offline maintenance-capture conditions above;
- if a chunk cannot be regenerated to match its original manifest entry, the affected chunk — and, depending on the artifact's own consistency requirements, potentially the whole artifact — must be marked failed/incomplete, never silently completed or approximated;
- the Simulator (Issue #7) can represent the offline-capture guarantee directly (a simulated source made stable for the duration of a simulated capture) without waiting for any additional real-endpoint mechanism, since M0/V1 does not require one beyond the offline workflow itself.

### 5a. Capture/source consistency is a distinct durable fact, independent of `Verified`

`Verified` (point 6) means cryptographic integrity only — stored bytes match the sealed Artifact/chunk identity. It must **not** be redefined to mean capture consistency. Capture consistency is a separate, independent durable fact on the Artifact:

`capture_consistency: NotApplicable | NotEstablished | Established`

- `NotApplicable`: the Artifact is not a capture of mutable client state for which source-writer consistency is a meaningful question (e.g., not every Artifact kind this Work Package's model could ever cover is a disk/file capture).
- `NotEstablished`: the Artifact is a capture of mutable client state, and the maintenance workflow has not (yet, or ever) positively confirmed the offline/read-only source conditions in point 5 held for its duration.
- `Established`: the maintenance workflow has **positively established** the relevant offline/read-only source conditions in point 5 for the duration of the capture (for M0/V1: the installed Windows OS was not running and the source was read-only for the capture's duration). This is an explicit, recorded confirmation, not a default — an Artifact does not become `Established` merely by existing.

`capture_consistency == Established` means "the bytes belong to a stable capture under the declared capture conditions" — it does **not** mean "the filesystem/application state was logically healthy" (see point 5's semantic-correctness caveat).

**A critical backup gating a destructive provisioning operation must satisfy both**, when that Artifact type requires capture consistency:

- `Verified` (cryptographic integrity), **and**
- `capture_consistency == Established`.

A `Verified` backup whose `capture_consistency` is `NotEstablished` must **not** authorize the destructive step. This is an addition to, not a replacement of, the existing "only a `Verified` artifact may be consumed by a destructive operation" invariant in point 6.

### 6. Manifest construction, sealing, and the Artifact lifecycle

An Artifact's chunk manifest is not necessarily complete when capture begins. Requiring the complete source to be pre-read solely to construct a manifest before any chunk can be transferred is **not** an M0 architectural requirement — that would be impractical for an Agent → Server capture of a large volume/image.

- **Manifest construction** happens while the Artifact is `Incomplete`: as each logical chunk is produced, that chunk's identity — index, size, digest — becomes durable artifact metadata at that time. This per-chunk identity metadata is durable domain data (ADR-0007), not high-frequency progress/telemetry: it is written once per chunk as that chunk is produced, not repeatedly per byte or per retry.
- **`digest_algorithm` is fixed before construction begins**: the algorithm is fixed for the Artifact before the first chunk identity using it is committed — it cannot change partway through a capture.
- **The expected `artifact_digest` is computed by the producer during production, incrementally** — while the logical Artifact is being produced, not by a second pre-read of the completed source after the fact. This is the *expected* value; it is not itself a verification act.
- **Manifest sealing**: once the producer has identified/produced every expected chunk for that capture (e.g., reached the end of the source range for Volume/Image, or finished enumerating and sizing the selected files for Selective), the manifest is **sealed** — `digest_algorithm`, the expected `artifact_digest`, `chunk_count`, and the full set of chunk identities become fixed and immutable. No chunk identity may be added, removed, or have its recorded digest changed after sealing.
- **Verification is independent of the expected value it checks**: the receiving/verifying side independently computes the digest over the stored, correctly ordered Artifact content and compares it against the sealed expected `artifact_digest` — it never *defines* or *rewrites* the value it is supposed to verify against. Verification that silently accepted its own computed value as ground truth would not be verification.
- The Artifact may leave `Incomplete` for `PendingVerification` only once its manifest is **sealed** and every sealed chunk identity has a durably received, verified chunk matching it.
- **Capture continuation vs. transfer resume/retransmission** are distinct: *capture continuation* is producing a chunk whose identity does not yet exist in the manifest (forward progress, extending the still-under-construction manifest); *transfer resume/retransmission* is when a chunk identity already exists (sealed or not) and the same bytes must be reproduced from the source, or retrieved from durable staging, to satisfy that existing identity — this is the scenario `docs/reference/transfer-resumability-spike.md` Experiments C, D, and E evaluated.
- If a previously identified chunk cannot be reproduced to match its recorded digest, that digest is **never rewritten** to accept different bytes under the same chunk identity — doing so would silently redefine what the artifact is, the exact failure mode the Spike's evidence warns against (point 5). The affected chunk, and the Artifact per the states below, moves toward `Failed` instead.

Once manifest handling is understood this way, the Artifact has an explicit lifecycle satisfying `docs/discovery/architecture-redesign.md` "Backup model":

`Incomplete → PendingVerification → {Verified | Failed}`

- `Incomplete`: the manifest may still be under construction, or sealed but not yet fully chunk-verified; the artifact is not usable for any purpose, especially not as input to a destructive operation.
- `PendingVerification`: the manifest is sealed, every expected chunk is present, and each has individually passed its chunk digest check, but the artifact's own full-content digest has not yet been confirmed.
- `Verified`: the full-artifact digest is independently confirmed (see the manifest-sealing bullets above). Only a `Verified` artifact may be used as input to a destructive operation — this is the concrete mechanism satisfying the already-accepted invariant "critical backups must pass integrity verification before destructive provisioning proceeds" (`docs/discovery/architecture-redesign.md` "Security invariants"). Where the Artifact type requires it, the destructive operation must additionally check `capture_consistency == Established` (point 5a) — `Verified` alone is cryptographic integrity, not a claim about capture consistency.
- `Failed`: verification failed, or a required chunk could not be completed (see point 5). A `Failed` artifact is never silently retried into a different state; recovery requires a new capture/transfer attempt, subject to the same Job/JobStep/Attempt retry policy already established (ADR-0006) for the JobStep that produced it.

Completion (the `PendingVerification` → `Verified` transition, and the point at which the artifact becomes visible/usable at all) is atomic: no destructive step, or any other consumer, can observe a partially-written artifact as if it were complete. This is implemented at the storage layer (e.g., write to a temporary/incomplete location and atomically rename/commit on verified completion) — the exact mechanism is implementation-time, but the atomicity requirement itself is not optional.

### 7. Storage capability model, and SYSTEM/CACHE/ARCHIVE usage semantics

Storage Targets (the Domain concept already named in `docs/specifications/m0-stack-and-boundaries-baseline.md`) expose **capabilities** — a **set of roles** (not a single mutually-exclusive role), available capacity, and read/write characteristics relevant to scheduling (ADR-0006's Attempt-scoped storage leases) — never RAID layout assumptions or raw device names, consistent with the already-accepted direction in `docs/discovery/architecture-redesign.md` "Storage." A single physical device may satisfy multiple roles simultaneously (already accepted); modeling roles as a set is what makes that fact representable rather than forcing an artificial single choice.

`SYSTEM`, `CACHE`, and `ARCHIVE` are already-accepted vocabulary (`docs/discovery/adr-triage.md`); this Work Package owns their logical usage semantics, defined here at the capability level only — no filesystem, device, or RAID layout is assumed:

- **`SYSTEM`**: storage required for Bamep's own operational durable state (persistence/configuration, ADR-0007). Not implicitly the preferred bulk-artifact target unless the same Storage Target also exposes another applicable role.
- **`CACHE`**: optional working/staging/performance-oriented artifact storage. May hold `Incomplete` artifacts and additional copies of completed artifacts. Must not be assumed to be the sole retained copy when an artifact's retention requirement calls for durable preservation.
- **`ARCHIVE`**: optional storage eligible for retained completed/`Verified` artifacts.

**Verification and retention are independent concerns.** `Verified` (point 6) is a property of an artifact's content — its digest matches — established once, independent of where or how many copies of it exist. Placing a copy of an artifact in `ARCHIVE` is a retention/placement decision, not a substitute for cryptographic verification, and does not itself make an unverified artifact `Verified`. Conversely, a `Verified` artifact is not automatically "archived" — retention placement is a separate decision this ADR does not make. This Work Package does not define migration mechanics between roles, multi-copy consistency, or retention duration policy — those are implementation-time or future-work concerns.

This model is exposed through the existing `storage` Port (`docs/specifications/m0-stack-and-boundaries-baseline.md`); adapters implement it per storage backend without the Domain depending on any specific one.

### 8. Volume/Image vs. Selective backup

Consistent with the already-accepted requirement that these are independently specified strategies (`docs/discovery/architecture-redesign.md` "Backup model"; no generic `backup=true`):

- **Volume/Image backup** uses the chunking model in points 2–6 directly: the source is inherently a linear byte range (a disk/volume), so fixed-size chunk boundaries are sufficient, matching the Spike's evidence.
- **Selective backup** is file-granular: each selected file is its own artifact (or its own unit within a larger selective-backup artifact), following the same Artifact lifecycle (point 6); a large individual file may internally apply the same chunking mechanism. **This file-granularity design is a design implication drawn from the Spike's evidence, not an independently tested finding** — the Spike explicitly did not exercise per-file Selective backup behavior. Individual files can also change during capture and are subject to the same source-reproducibility boundary as point 5; file-level granularity does not exempt Selective backup from that requirement.

### 9. Transfer-session authentication — ACCEPTED: sender-constrained, transfer-scoped capability over HTTPS (Issue #15)

Every data-plane transfer must be bound to the Endpoint's already-authenticated Agent session — an unauthenticated or unbound data-plane transfer is not permitted. Issues #2/#3 (ADR-0004, ADR-0005) define the accepted Endpoint identity/session and Agent trust contracts that this binding builds on; they do not themselves own the concrete mechanism, which this point now resolves, executing Issue #15 (`[WP] Define authenticated data-plane transfer-session binding`).

**A pure bearer capability was evaluated first and rejected.** A bearer capability — even one narrowly scoped to one `transfer_id`/`artifact_id`/direction/`endpoint_id`, short-lived, and revalidated against durable state — proves only what is authorized, never who is presenting it. It does not satisfy Issue #15's explicit threat-model requirement "one valid Endpoint trying to use another Endpoint's transfer authorization": possession of the capability's bytes, by any means, would be sufficient to use it. The accepted mechanism below adds sender-constraint (proof of possession) to close this gap, without introducing OAuth, OIDC, DPoP as a protocol dependency, mTLS, client certificates, a new PKI, or a new persistent Endpoint identity mechanism.

**Accepted mechanism:**

1. **Data-plane transport is HTTPS, not plain HTTP.** ADR-0008 point 1's "HTTP-based data plane" did not itself decide HTTP vs. HTTPS. Given the provisioning network is never a trust anchor (`AGENTS.md`), any authorization material would otherwise be exposed to capture/replay in plaintext, and Artifact contents may themselves carry sensitive user/customer data, M0/V1 requires HTTPS for the data plane. Server identity reuses the **same pinned Server TLS certificate/fingerprint** already authenticated for the Agent Protocol WSS connection via trusted bootstrap (ADR-0010/ADR-0011) — no second trust relationship is introduced. The Agent does not present a client certificate for the data plane, exactly as it does not for Agent Protocol (ADR-0005) — **mTLS is rejected**, superseded by the narrower application-level sender-constraint below.
2. **Authorization is a short-lived, transfer-scoped, Server-signed capability, sender-constrained to an ephemeral Agent-held asymmetric proof key** — not a plain bearer capability. It is bound to exactly one `transfer_id`, `artifact_id`, `endpoint_id`, direction, the `attempt_id` of the transfer JobStep's Attempt that caused it to be issued, and the thumbprint of the ephemeral proof key it is constrained to. It is **not** derived from, or a synonym for, the Endpoint's long-lived Agent runtime credential (rejected: least-authority violation, large blast radius, cannot be revoked independently — see "Alternatives considered"), and the ephemeral proof key is **never** an Endpoint identity credential and is **never** persisted as durable Endpoint identity/trust state.
3. **Issuance rides the already-authenticated Agent Protocol control-plane channel**, via three new, strictly additive message types (`docs/specifications/m0-agent-protocol-contract.md` "Transfer authorization"): `TransferAuthorizationRequest{transfer_id, proof_public_key}` (Agent → Server, now also carrying the ephemeral public proof key or its canonical representation), `TransferAuthorizationGrant{transfer_id, token, expires_at}` (Server → Agent — the resulting sender-constrained capability), and `TransferAuthorizationDenied{transfer_id, reason}` (Server → Agent). This addition does **not** reopen WSS, pinned TLS, `AuthRequest`/`SessionEstablished`, or `BootstrapEvidence` — it follows the exact precedent already set when `BootstrapEvidence` was added additively (Issue #13). Bulk Artifact bytes still never flow over Agent Protocol — only the small authorization capability does, exactly as `ActionDispatch` parameters already carry action-type-specific data without becoming a bulk-transfer channel.
4. **Every HTTPS data-plane request carries the capability plus a fresh proof, signed by the ephemeral private key, of possession of the key the capability is bound to.** Capability signature verification and proof signature verification can each be performed statelessly (no durable lookup needed for the cryptographic check itself), but the **complete authorization decision is state-aware**: every request is additionally revalidated against current **durable** transfer/Attempt/credential state (transfer not terminal, Endpoint/direction/artifact match, Endpoint credential still `CredentialActive`) and against a **bounded transient** replay cache of already-accepted proof identifiers. This combination gives real-time revocation effectiveness (denying a cancelled transfer, a revoked credential, or a replayed proof immediately) without persisting the capability or any individual proof as a durable, reusable secret.
5. **Authorization lifetime — including the proof key's own lifetime — is decoupled from durable transfer identity.** The capability is short-lived and renewable, and the ephemeral proof key may be reused or rotated across renewals of the same `transfer_id`; `transfer_id`, the chunk manifest, and already-verified chunks (point 6) are wholly unaffected by capability/key expiry or renewal — a transfer never restarts, and no chunk is ever re-verified, merely because its authorization secret or proof key changed. Renewal reuses the same `TransferAuthorizationRequest`/`Grant` pair for the same, still-legitimate (non-terminal) `transfer_id`, independent of JobStep/Attempt retry (ADR-0006) — renewal is not a retry and never creates a new Attempt.
6. **A transient WSS control-plane disconnect does not revoke an already-issued, still-valid, sender-constrained capability** — the HTTPS data-plane channel does not depend on the WSS socket remaining open, and disconnect alone neither invalidates nor renews it. It never becomes an indefinitely reusable independent access channel: it remains short-lived, scoped to one transfer, sender-constrained, and revalidated against durable state and replay history on every use, and an **explicit** Endpoint credential revocation (`CredentialRevoked`) cascades to deny further use of any outstanding capability for that Endpoint, even before its own expiry.
7. **On Server restart, outstanding capabilities whose replay-protection continuity cannot be guaranteed are treated as invalid** and must be reissued after the Agent re-establishes the authenticated control-plane context and the Server reconciles durable transfer/Attempt state — an authorization-renewal event only, never a new `transfer_id`, Artifact, Attempt, or destructive retry, and never a weakening of replay protection merely because an in-memory replay cache was lost.
8. **On Agent process restart, the lost ephemeral private key renders the prior capability unusable by that Agent** — by design, not as an accepted gap: the key is intentionally non-durable and is never persisted merely to avoid this flow. The Agent re-authenticates over Agent Protocol, the Server reconciles existing durable transfer/Attempt state, and if continuation remains authorized, the Agent generates a fresh ephemeral keypair and requests a fresh capability for the same `transfer_id`.

Exact capability/proof TTL and freshness-window duration, concrete signature/wire/serialization formats, the concrete asymmetric algorithm for the proof key (which need not be the same as the capability-signing mechanism), and HTTP-level details (header names, status codes) remain implementation-time, consistent with this ADR's existing pattern for `digest_algorithm` and chunk size (points 3–4). The full operational contract — bindings, issuance sequence, proof-of-possession fields, replay semantics, revocation/fail-closed cases, reconnect/restart behavior, and Simulator scenarios — is defined in `docs/specifications/m0-data-plane-and-storage-contracts.md` "Transfer-session authentication", consistent with how points 1–8 above already split decision (here) from detailed contract (there).

**Threat-model statement.** The accepted mechanism protects against: passive provisioning-LAN capture (via HTTPS); use of a stolen capability by a party that does not possess the bound ephemeral private proof key; cross-Endpoint capability substitution; cross-transfer/cross-Artifact/cross-direction use; straightforward replay of an already-accepted request proof; stale/revoked/terminal authorization use; and Server confused-deputy mistakes covered by the explicit bindings. It does **not** claim protection if an attacker compromises the authenticated Agent deeply enough to obtain **both** the valid capability and its corresponding ephemeral private proof key — consistent with M0's already-accepted assurance boundary for a fully-compromised Endpoint (`docs/specifications/m0-trusted-bootstrap-and-server-fingerprint-contract.md` "M0 threat-model boundary").

### 10. Correlation: `transfer_id`

`transfer_id` (reserved by `docs/specifications/m0-persistence-observability-and-domain-events.md`'s correlation model) is the **durable identity of one logical data-plane transfer operation** — not one network/HTTP session. A single logical transfer may span multiple HTTP requests, connection loss and reconnect, and more than one JobStep Attempt when retry/reconciliation policy permits recovery or resume (ADR-0006) — none of that starts a new `transfer_id`, because the artifact and its chunk manifest (point 6) persist independently of any single Attempt or connection.

`transfer_id` is kept distinct from `attempt_id` (the Server-side JobStep Attempt that requested the transfer) for the same reason `attempt_id` and `action_id` are kept distinct (ADR-0007): a transfer is a data-plane concept durably correlated to, but not identical with, the control-plane Attempt that triggered it. A **new logical transfer** of the same Artifact — for example, delivering an already-`Verified` Artifact to a different Endpoint — receives a **new** `transfer_id`, because it is a different logical operation even though it moves the same bytes.

HTTP connection/request identities are transient transport details and do not need a Domain identity in M0 — only the logical transfer operation (`transfer_id`) and the chunk identities within its manifest (point 6) are durable.

### 11. Multi-disk endpoints and artifact source provenance

An Endpoint is not modeled as having one implicit disk. Bamep must support endpoints with multiple physical disks and/or volumes — for example, an NVMe/SSD containing Windows plus an HDD containing user data, multiple data disks, or an old HDD used as a capture source while a new SATA SSD/NVMe becomes the provisioning target.

An Artifact's **source provenance** identifies the concrete disk/volume/filesystem it was captured from, not merely "the Endpoint" that owns it. At the contract/Domain level, an Artifact must be able to correlate to the relevant source disk/volume/filesystem identity distinct from the Endpoint's own identity (ADR-0004). This ADR does not define the exact schema or field set for that provenance record — only that the correlation must be preservable.

The load-bearing invariant this establishes: **Artifact source provenance is not the same fact as future destructive-target identity** — see point 12.

### 12. Source identity vs. target-disk identity are independent (disk replacement)

A valid Bamep workflow: an old HDD is backed up offline; the physical disk is replaced; a new SSD/NVMe is installed; inventory is revalidated; the new disk is provisioned; the retained user data is restored onto it.

Therefore:

- restoring/migrating retained data must **not** require the destination disk's fingerprint to equal the source Artifact's source-disk fingerprint (point 11);
- **source identity** answers "where did these bytes come from?" (the Artifact's provenance, point 11);
- **target-disk identity** answers "which currently installed disk is the destructive Job authorized to modify?" — this is the existing target disk identity/fingerprint revalidation already required by the destructive dispatch precondition composition (point 6; ADR-0004; ADR-0006 "Revalidation immediately before dispatch");
- these are independent facts, and this ADR does not weaken the existing target-disk revalidation safety invariant (Issues #2/#4) in any way — the new target disk must still satisfy it, in full, immediately before execution, exactly as already required.

A disk replacement may also legitimately change an Endpoint's observed hardware inventory (a new disk fingerprint appears where an old one was recorded). This ADR does **not** design the full planned-hardware-change authorization mechanism — that remains for the identity Work Package's own model (`docs/specifications/m0-endpoint-identity-lifecycle.md`) to eventually extend, not something this Work Package redesigns. It records, as a use case this Work Package's contract must not obstruct: an operator-authorized disk replacement is valid and must not be automatically interpreted as meaning a different Endpoint solely because the disk changed.

## Alternatives considered

- **Plain HTTP Range-based byte-offset resume**: rejected as the general mechanism — dishonest exactly in the cases the Spike demonstrated (Experiments B, E). Not excluded as a possible future optimization layered *within* a chunk once a chunk itself is known-incomplete (e.g., resuming a partially-received chunk by byte offset within that one chunk, re-verified by the chunk's own digest on completion), but this ADR does not require that optimization and it is not decided here.
- **A single continuous streaming HTTP body per artifact, no chunk manifest**: rejected — provides no safe resume points (Spike Experiment B) and no selective corruption detection (Spike Experiment C's advantage over B).
- **Deciding a general-purpose (live-capable) source-consistency/snapshot mechanism now**: rejected — no evidence exists yet for a live-Windows scenario; the Spike explicitly left this open, and live backup is explicitly outside V1 scope (point 5). Inventing a mechanism now would be establishing architecture without evidence, which `docs/development/sdd.md` prohibits. What M0/V1 accepted instead is narrower: offline maintenance capture, which sidesteps the general problem by removing the concurrent writer rather than solving live-source consistency.
- **Requiring VSS or another snapshot technology for M0/V1**: rejected — unnecessary given the accepted offline maintenance-capture workflow (point 5); the installed OS is not running, so there is no concurrent writer to snapshot against. Snapshot technology remains a candidate for a future live-backup decision, not chosen or ruled out here.
- **Coupling restore/migration authorization to source-disk identity matching the destination disk**: rejected (point 12) — would make the disk-replacement use case impossible; source provenance and destructive-target identity are independent facts, and only the latter is a safety precondition on the destructive step.
- **A generic `backup=true` flag instead of distinct Volume/Image and Selective strategies**: rejected — explicitly excluded by already-accepted Discovery direction.
- **Reusing the Agent Protocol WebSocket connection for transfer bytes**: rejected — already-accepted control/data-plane separation (`docs/discovery/architecture-redesign.md`), and would couple large-transfer backpressure to the same connection carrying safety-relevant control messages (cancellation, status queries).
- **Deriving data-plane authorization directly from the long-lived Agent runtime credential ("Candidate B", point 9)**: rejected — grants authority disproportionate to one transfer (least-authority violation), has a much larger blast radius and replay window if leaked over the data plane, and cannot be revoked/expired independently of the whole Agent session without collateral damage to unrelated in-flight work.
- **A per-transfer asymmetric keypair / client-certificate (mTLS-style) capability ("Candidate C", point 9)**: considered and rejected as disproportionate complexity for the *capability-signing* mechanism — the Server is both sole issuer and sole verifier, so a symmetric Server-held signing secret suffices there. This is distinct from, and does not conflict with, the ephemeral asymmetric *proof key* the accepted mechanism uses for sender-constraint (point 9) — that key exists only to let the Agent prove possession, and is never a client certificate or a PKI-managed identity.
- **A plain bearer capability, narrowly scoped and short-lived but without sender-constraint (owner-review "Choice A", point 9)**: rejected — does not satisfy Issue #15's explicit threat-model requirement that one Endpoint must not be able to use another Endpoint's transfer authorization if it somehow obtains the capability's bytes. Accepting this would have required explicitly narrowing an already-stated M0 security requirement rather than meeting it, inconsistent with the rest of M0's demonstrated posture (no TOFU; independent, non-inferred destructive preconditions; rejecting derivation of data-plane authority from the long-lived credential precisely for blast-radius reasons).
- **OAuth 2.0 / OIDC, or DPoP adopted as a formal protocol dependency**: rejected — M0 needs only a narrow, Bamep-internal sender-constraint property (the Server is both sole issuer and sole verifier, with no third-party relying party ever involved), not a general-purpose delegated-authorization framework or its accompanying specification/library surface. The accepted mechanism is conceptually similar to established proof-of-possession patterns (e.g., DPoP) without adopting them as a dependency.
- **Plain HTTP for the data plane**: rejected (point 9) — would expose the transfer-scoped capability, its per-request proof-of-possession material, and potentially sensitive Artifact content, to any device with provisioning-network position, which is explicitly never a trust anchor.
- **A persisted server-side session/token table for transfer authorization**: rejected — a self-verifying signed capability, revalidated against already-durable transfer/Attempt/credential state on each use, achieves equivalent real-time revocation without persisting a separate reusable secret per issued token (ADR-0007's durable/transient boundary).
- **A single token reusable across multiple transfers, Artifacts, or Endpoints**: rejected — violates least authority and reintroduces exactly the cross-Endpoint/cross-Artifact confused-deputy risk the transfer-scoped binding exists to prevent.

## Consequences

- Issue #7 (Simulator) must simulate chunk-oriented transfer, including interrupted/corrupted-chunk scenarios and `Incomplete`/`PendingVerification`/`Verified`/`Failed` artifact transitions, at the M0 20–24 endpoint target, plus offline-capture and `capture_consistency` scenarios (point 5a).
- Issue #5's persistence model (ADR-0007) must persist Artifact lifecycle transitions, chunk manifests, `capture_consistency`, and source-provenance records as durable domain state, consistent with its durable/transient boundary (chunk *manifests* are durable; raw chunk transfer progress is the data-plane's own concern, not a domain-event-per-chunk).
- Any destructive JobStep consuming a backup artifact must verify the artifact is `Verified`, and — where the Artifact type requires it — that `capture_consistency == Established`, as part of its own destructive-operation preconditions (ADR-0004, ADR-0006) — this ADR does not redefine those preconditions, only supplies the artifact-state facts they must check.
- Transfer-session authentication (point 9) is resolved as a **sender-constrained** capability: `docs/specifications/m0-agent-protocol-contract.md` gains three new, strictly additive message types (`TransferAuthorizationRequest`, `TransferAuthorizationGrant`, `TransferAuthorizationDenied`), and `docs/specifications/m0-data-plane-and-storage-contracts.md` gains the full operational contract, including ephemeral proof-key handling and per-request proof-of-possession — no future implementation Work Package inherits this as a hidden architectural decision.
- The M0 data plane requires HTTPS, reusing the Server TLS identity already pinned via trusted bootstrap; no separate data-plane PKI, mTLS, OAuth, OIDC, or DPoP protocol dependency is introduced.
- The Server must maintain a small, bounded, **transient** (non-durable) replay cache of accepted proof identifiers, scoped to the applicable freshness window — a new runtime-state responsibility, distinct from ADR-0007's durable domain state.
- On Server restart, outstanding capabilities lose replay-protection continuity and must be treated as invalid pending reissuance — implementations must ensure this explicitly (e.g., an authorization epoch or equivalent), not merely hope the replay cache loss is harmless.
- Issue #7's Simulator must exercise the real `TransferAuthorizationRequest`/`Grant`/`Denied` messages, real ephemeral proof-key generation, and real per-request proof-of-possession and revalidation, not a faked authorization boundary, consistent with the Simulator's already-accepted real-transport fidelity rule (`docs/specifications/m0-simulator-contract-and-validation-strategy.md`).
- Live-Windows backup consistency remains an open architectural question for any future work that proposes it; this ADR does not evaluate or partially decide it.
- The disk-replacement use case (point 12) must remain representable by any future planned-hardware-change authorization work in the identity Work Package's model — this ADR does not design that mechanism, only records the constraint it must satisfy.

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Data plane", "Storage", "Backup model", "Security invariants".
- `docs/discovery/adr-triage.md` — candidates 8, 9, 10.
- `docs/reference/transfer-resumability-spike.md` — empirical evidence this ADR applies.
- ADR-0004 — Endpoint identity (destructive-operation preconditions an artifact's `Verified` state feeds; target-disk identity revalidation, point 12; credential state, `CredentialRevoked`, transfer-authorization revalidation cascades against, point 9).
- ADR-0005 — Agent control-plane protocol (control/data-plane separation; `ActionProgress`; `action_id`).
- ADR-0006 — Job/JobStep/Attempt model (a transfer JobStep's Attempt; `attempt_id`; reconciliation semantics point 9's authorization lifetime reuses).
- ADR-0007 — Persistence backend and durable/transient boundary (`transfer_id` correlation; artifact durability; durable-vs-transient split for transfer-authorization state, point 9).
- ADR-0010 / ADR-0011 — Trusted bootstrap and site trust-anchor baseline (pinned Server TLS identity point 9's HTTPS requirement reuses, unchanged).
- `docs/specifications/m0-stack-and-boundaries-baseline.md` — `storage` Port; Artifact/Snapshot, Transfer, Storage Target Domain concepts.
- `docs/specifications/m0-data-plane-and-storage-contracts.md` — detailed contract and validation expectations, including the full point 9 operational contract.
- `docs/specifications/m0-agent-protocol-contract.md` — additively amended (Issue #15) to carry `TransferAuthorizationRequest`/`Grant`/`Denied`.

## Related work

- Issue #6 — `[WP] Define data-plane and storage contracts`.
- Issue #9 — `[Spike] Evaluate resumable volume/image transfer` (evidence this ADR applies).
- Issue #8 — `[Spike] Validate WinPE boot mechanism` (validates the boot mechanism only; may later constrain which source-consistency mechanisms are practical in the real maintenance environment, but does not itself resolve that open requirement).
- Issue #15 — `[WP] Define authenticated data-plane transfer-session binding` (resolves point 9).
- Issue #2 / ADR-0004, Issue #3 / ADR-0005 — constrain, and (with Issue #15) now resolve, transfer-session authentication (point 9); also own target-disk identity revalidation (point 12) and the Endpoint identity model any future disk-replacement authorization would extend.
- Issue #4 / ADR-0006 — Attempt model a transfer belongs to.
- Issue #5 / ADR-0007 — persistence of artifact/chunk-manifest durable state; `transfer_id` correlation.
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (must simulate this contract's scenarios, including point 9's authorization messages).
- Issue #13 / ADR-0011 — site trust-anchor establishment (source of the pinned Server TLS identity point 9's HTTPS requirement reuses, unchanged).
