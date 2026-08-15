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

### 5. Resumability is bounded by source reproducibility — the consistency mechanism is not decided here

Per the Spike's confirmed conclusion: chunking and per-chunk digests detect missing or changed chunks and allow selective retransmission, but do **not** by themselves make a changed source reproducible. Spike Experiment E demonstrated directly that a chunk regenerated from a since-mutated source correctly *fails* the original manifest — the artifact cannot be completed from that changed source, only correctly rejected.

Therefore:

- an artifact's manifest is valid only for chunks whose source remained stable for the duration of that artifact's capture;
- if a chunk cannot be regenerated to match its original manifest entry (source changed, chunk lost with no stable source to regenerate from), the affected chunk — and, depending on the artifact's own consistency requirements, potentially the whole artifact — must be marked failed/incomplete, never silently completed or approximated;
- **the concrete mechanism that keeps a source stable for a capture's duration (e.g., a snapshot, a quiesced source, or durable staging of already-produced chunks) is not decided by this ADR.** The Spike explicitly did not evaluate or choose one, and no M0 evidence yet exists to choose one responsibly. This is kept as an **explicit unresolved architectural requirement**: a real capture must preserve or reproduce the bytes belonging to that capture through a snapshot, a quiesced source, durable staging, or another evidence-backed mechanism — which mechanism is not chosen here. Issue #8 (`[Spike] Validate WinPE boot mechanism`) validates the WinPE boot mechanism only; it does not investigate or produce evidence for source consistency, snapshotting, quiescing, or durable capture staging, and this ADR does not depend on it for that question. Issue #8's eventual findings may later constrain which consistency mechanisms are practically available inside the real maintenance environment, but they do not themselves resolve this requirement. This unresolved mechanism does not invalidate the M0 transfer contract: the contract can require source reproducibility/materialization (points 2, 6) without selecting how production Volume/Image capture achieves it. Production Volume/Image capture cannot be safely implemented until an actual mechanism is chosen through further evidence and a future decision.
- the Simulator (Issue #7) is not subject to this open dependency in the same way, since a simulated source can be made trivially stable for the duration of a simulated capture; this ADR's chunking/manifest/verification mechanism is fully implementable and testable against the Simulator without waiting for the real-endpoint consistency mechanism.

### 6. Manifest construction, sealing, and the Artifact lifecycle

An Artifact's chunk manifest is not necessarily complete when capture begins. Requiring the complete source to be pre-read solely to construct a manifest before any chunk can be transferred is **not** an M0 architectural requirement — that would be impractical for an Agent → Server capture of a large volume/image.

- **Manifest construction** happens while the Artifact is `Incomplete`: as each logical chunk is produced, that chunk's identity — index, size, digest — becomes durable artifact metadata at that time. This per-chunk identity metadata is durable domain data (ADR-0007), not high-frequency progress/telemetry: it is written once per chunk as that chunk is produced, not repeatedly per byte or per retry.
- **Manifest sealing**: once the producer has identified/produced every expected chunk for that capture (e.g., reached the end of the source range for Volume/Image, or finished enumerating and sizing the selected files for Selective), the manifest is **sealed** — `chunk_count` and the full set of chunk identities become fixed. A sealed manifest is immutable for that Artifact: no chunk identity may be added, removed, or have its recorded digest changed after sealing.
- The Artifact may leave `Incomplete` for `PendingVerification` only once its manifest is **sealed** and every sealed chunk identity has a durably received, verified chunk matching it.
- **Capture continuation vs. transfer resume/retransmission** are distinct: *capture continuation* is producing a chunk whose identity does not yet exist in the manifest (forward progress, extending the still-under-construction manifest); *transfer resume/retransmission* is when a chunk identity already exists (sealed or not) and the same bytes must be reproduced from the source, or retrieved from durable staging, to satisfy that existing identity — this is the scenario `docs/reference/transfer-resumability-spike.md` Experiments C, D, and E evaluated.
- If a previously identified chunk cannot be reproduced to match its recorded digest, that digest is **never rewritten** to accept different bytes under the same chunk identity — doing so would silently redefine what the artifact is, the exact failure mode the Spike's evidence warns against (point 5). The affected chunk, and the Artifact per the states below, moves toward `Failed` instead.

Once manifest handling is understood this way, the Artifact has an explicit lifecycle satisfying `docs/discovery/architecture-redesign.md` "Backup model":

`Incomplete → PendingVerification → {Verified | Failed}`

- `Incomplete`: the manifest may still be under construction, or sealed but not yet fully chunk-verified; the artifact is not usable for any purpose, especially not as input to a destructive operation.
- `PendingVerification`: the manifest is sealed, every expected chunk is present, and each has individually passed its chunk digest check, but the artifact's own full-content digest has not yet been confirmed.
- `Verified`: the full-artifact digest is confirmed. Only a `Verified` artifact may be used as input to a destructive operation — this is the concrete mechanism satisfying the already-accepted invariant "critical backups must pass integrity verification before destructive provisioning proceeds" (`docs/discovery/architecture-redesign.md` "Security invariants").
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

### 9. Transfer session authentication is not designed here

Every data-plane transfer must be bound to the Endpoint's already-authenticated Agent session (ADR-0004 identity/credential model; ADR-0005 control-plane handshake) — an unauthenticated or unbound data-plane transfer is not permitted. The concrete mechanism for establishing that binding (e.g., a short-lived transfer-scoped token issued over the control plane) is **not designed by this Work Package**, consistent with Issue #6's explicit out-of-scope boundary for transfer-session authentication (owned by the identity/control-protocol Work Packages, Issues #2/#3).

### 10. Correlation: `transfer_id`

`transfer_id` (reserved by `docs/specifications/m0-persistence-observability-and-domain-events.md`'s correlation model) is the **durable identity of one logical data-plane transfer operation** — not one network/HTTP session. A single logical transfer may span multiple HTTP requests, connection loss and reconnect, and more than one JobStep Attempt when retry/reconciliation policy permits recovery or resume (ADR-0006) — none of that starts a new `transfer_id`, because the artifact and its chunk manifest (point 6) persist independently of any single Attempt or connection.

`transfer_id` is kept distinct from `attempt_id` (the Server-side JobStep Attempt that requested the transfer) for the same reason `attempt_id` and `action_id` are kept distinct (ADR-0007): a transfer is a data-plane concept durably correlated to, but not identical with, the control-plane Attempt that triggered it. A **new logical transfer** of the same Artifact — for example, delivering an already-`Verified` Artifact to a different Endpoint — receives a **new** `transfer_id`, because it is a different logical operation even though it moves the same bytes.

HTTP connection/request identities are transient transport details and do not need a Domain identity in M0 — only the logical transfer operation (`transfer_id`) and the chunk identities within its manifest (point 6) are durable.

## Alternatives considered

- **Plain HTTP Range-based byte-offset resume**: rejected as the general mechanism — dishonest exactly in the cases the Spike demonstrated (Experiments B, E). Not excluded as a possible future optimization layered *within* a chunk once a chunk itself is known-incomplete (e.g., resuming a partially-received chunk by byte offset within that one chunk, re-verified by the chunk's own digest on completion), but this ADR does not require that optimization and it is not decided here.
- **A single continuous streaming HTTP body per artifact, no chunk manifest**: rejected — provides no safe resume points (Spike Experiment B) and no selective corruption detection (Spike Experiment C's advantage over B).
- **Deciding the source-consistency/snapshot mechanism now**: rejected — no evidence exists yet; the Spike explicitly left this open. Issue #8 does not supply this evidence either (it validates the WinPE boot mechanism only). Inventing a mechanism now would be establishing architecture without evidence, which `docs/development/sdd.md` prohibits.
- **A generic `backup=true` flag instead of distinct Volume/Image and Selective strategies**: rejected — explicitly excluded by already-accepted Discovery direction.
- **Reusing the Agent Protocol WebSocket connection for transfer bytes**: rejected — already-accepted control/data-plane separation (`docs/discovery/architecture-redesign.md`), and would couple large-transfer backpressure to the same connection carrying safety-relevant control messages (cancellation, status queries).

## Consequences

- Issue #7 (Simulator) must simulate chunk-oriented transfer, including interrupted/corrupted-chunk scenarios and `Incomplete`/`PendingVerification`/`Verified`/`Failed` artifact transitions, at the M0 20–24 endpoint target.
- Issue #5's persistence model (ADR-0007) must persist Artifact lifecycle transitions and chunk manifests as durable domain state, consistent with its durable/transient boundary (chunk *manifests* are durable; raw chunk transfer progress is the data-plane's own concern, not a domain-event-per-chunk).
- Production Volume/Image capture cannot be safely implemented until the source-consistency mechanism (point 5) is resolved through further evidence — this is a concrete, tracked gap, not an oversight.
- Any destructive JobStep consuming a backup artifact must verify the artifact is `Verified` as part of its own destructive-operation preconditions (ADR-0004, ADR-0006) — this ADR does not redefine those preconditions, only supplies the artifact-state fact they must check.
- The transfer-session authentication mechanism (point 9) remains an open, tracked requirement on whichever future work extends the identity/control-protocol Work Packages to cover it.

## Related architecture

- `docs/discovery/architecture-redesign.md` — "Data plane", "Storage", "Backup model", "Security invariants".
- `docs/discovery/adr-triage.md` — candidates 8, 9, 10.
- `docs/reference/transfer-resumability-spike.md` — empirical evidence this ADR applies.
- ADR-0004 — Endpoint identity (destructive-operation preconditions an artifact's `Verified` state feeds; transfer-session authentication boundary).
- ADR-0005 — Agent control-plane protocol (control/data-plane separation; `ActionProgress`; `action_id`).
- ADR-0006 — Job/JobStep/Attempt model (a transfer JobStep's Attempt; `attempt_id`).
- ADR-0007 — Persistence backend and durable/transient boundary (`transfer_id` correlation; artifact durability).
- `docs/specifications/m0-stack-and-boundaries-baseline.md` — `storage` Port; Artifact/Snapshot, Transfer, Storage Target Domain concepts.
- `docs/specifications/m0-data-plane-and-storage-contracts.md` — detailed contract and validation expectations.

## Related work

- Issue #6 — `[WP] Define data-plane and storage contracts`.
- Issue #9 — `[Spike] Evaluate resumable volume/image transfer` (evidence this ADR applies).
- Issue #8 — `[Spike] Validate WinPE boot mechanism` (validates the boot mechanism only; may later constrain which source-consistency mechanisms are practical in the real maintenance environment, but does not itself resolve that open requirement).
- Issue #2 / ADR-0004, Issue #3 / ADR-0005 — transfer-session authentication boundary.
- Issue #4 / ADR-0006 — Attempt model a transfer belongs to.
- Issue #5 / ADR-0007 — persistence of artifact/chunk-manifest durable state; `transfer_id` correlation.
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (must simulate this contract's scenarios).
