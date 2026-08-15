# M0 — Data-Plane and Storage Contracts

Status: **Proposed - awaiting owner approval**

## Context

This Specification details the chunk-oriented data-plane contract, artifact lifecycle, and storage capability model accepted in ADR-0008, executing Issue #6 (`[WP] Define data-plane and storage contracts`). It applies the empirical evidence in `docs/reference/transfer-resumability-spike.md`.

## Chunk manifest

Every artifact transferred over the data plane has a **chunk manifest**:

- `artifact_id` — identifies the artifact this manifest belongs to.
- `chunk_size` — fixed for this manifest; exact value is implementation-time tuning (ADR-0008).
- `chunk_count`.
- per chunk: `chunk_index`, `digest` (SHA-256 of that chunk's content), `size` (the last chunk may be shorter than `chunk_size`).
- `artifact_digest` — SHA-256 over the full reassembled artifact content, independent of the per-chunk digests, checked at `PendingVerification` → `Verified` (see "Artifact lifecycle").

The manifest is itself durable domain data once an artifact capture begins (per ADR-0007's durable/transient boundary) — it is not high-frequency data and is written on creation and on completion, not per chunk transfer attempt.

## Chunk transfer

- Each chunk is transferred as an independently addressable unit (e.g., one HTTP request per chunk), carrying `artifact_id`, `chunk_index`, and enough information for the receiver to verify the chunk's digest immediately upon receipt.
- A chunk that fails digest verification on receipt is rejected and not written to durable chunk storage as if it were valid — the sender is expected to retry that chunk.
- Resume logic: before (re)transferring any chunk, the receiver checks whether it already holds a chunk at that index matching the manifest digest; if so, that chunk is skipped. Only missing or mismatching chunks are (re)transferred — directly implementing the pattern validated in `docs/reference/transfer-resumability-spike.md` Experiments C and D.
- Chunk transfer direction is symmetric: the same manifest/verification pattern applies whether the Agent is producing (backup/capture) or consuming (provisioning/restore) the artifact.

## Source-reproducibility boundary (not resolved here)

If a chunk cannot be (re)produced to match its manifest digest — because the underlying source has changed since the manifest was created, per `docs/reference/transfer-resumability-spike.md` Experiment E — that chunk transfer fails. This Specification requires that failure to be explicit (the chunk, and depending on the artifact's consistency requirements, the artifact itself, moves toward `Failed` — see "Artifact lifecycle"), never silently accepted or approximated.

The mechanism that keeps a source stable for a capture's duration (snapshot, quiesced source, durable staging of produced chunks, or another approach) is **not defined by this Specification** — see ADR-0008 point 5. This is a tracked, open architectural dependency, not an oversight.

## Artifact lifecycle

States: `Incomplete`, `PendingVerification`, `Verified`, `Failed`.

Transitions:

- `(created)` → `Incomplete`: an artifact capture/transfer begins; the manifest exists but not all chunks are yet present and chunk-verified.
- `Incomplete` → `PendingVerification`: every chunk in the manifest is present and has individually passed its chunk digest check.
- `PendingVerification` → `Verified`: the full-artifact digest (`artifact_digest`) is computed over the reassembled content and matches the manifest. This transition is atomic and is the point at which the artifact becomes visible/usable to any consumer (ADR-0008 point 6) — no partially-written or not-yet-verified artifact is ever observable as complete.
- `PendingVerification` → `Failed`: the full-artifact digest does not match, despite every individual chunk having passed its own check (e.g., chunk-ordering error, manifest corruption). This is distinguished from a chunk-level failure for diagnostic purposes.
- `Incomplete` → `Failed`: a required chunk cannot be (re)produced to match its manifest digest (source-reproducibility boundary above), or the capture is otherwise abandoned/cancelled per the owning JobStep's cancellation (ADR-0006).
- `Verified` and `Failed` are terminal for that artifact. A `Failed` artifact is not retried in place; a new capture/transfer attempt (a new Attempt on the owning JobStep, per ADR-0006's retry policy) produces a new artifact.

Only a `Verified` artifact may be consumed by a destructive operation. A destructive JobStep's precondition revalidation (ADR-0006 "Revalidation immediately before dispatch") must include checking that any artifact it depends on is still `Verified` at dispatch time, not merely was `Verified` when the JobStep was first evaluated.

## Storage capability model

A Storage Target exposes:

- `role` — `SYSTEM`, `CACHE`, or `ARCHIVE` (already-accepted vocabulary; one physical location may satisfy multiple roles).
- `available_capacity` — for scheduling (Attempt-scoped storage leases, ADR-0006).
- read/write throughput characteristics relevant to scheduling — exact fields are implementation-time, not enumerated exhaustively here.

Storage Targets are addressed through the `storage` Port (`docs/specifications/m0-stack-and-boundaries-baseline.md`); Domain and Application code never assume a RAID layout, filesystem, or raw device name — those are Adapter concerns.

## Volume/Image vs. Selective backup

- **Volume/Image**: one artifact per capture, chunked at fixed byte boundaries directly over the linear disk/volume byte range (ADR-0008 point 8).
- **Selective**: file-granular — each selected file is its own artifact (or its own unit within a larger selective-backup artifact), using the same Artifact lifecycle above; a large individual file may apply the same chunking mechanism internally. **This is a design candidate informed by the chunking evidence, not an empirically tested finding** (`docs/reference/transfer-resumability-spike.md`) — per-file behavior has not been exercised. Individual files can change during capture and are subject to the same source-reproducibility boundary as Volume/Image; per-file granularity does not exempt Selective backup from it.

## Out of scope

- exact chunk size — implementation-time tuning;
- the source-consistency/snapshot mechanism for real endpoint capture — depends on Issue #8 and future evidence, not decided here;
- transfer-session authentication mechanism (token format, issuance) — owned by Issues #2/#3, not designed here;
- final production backup/snapshot format — explicitly out of M0 scope;
- HTTP wire-level details (headers, status codes, request/response framing) beyond "one chunk per request-response unit" — implementation-time;
- domain-event catalog additions for artifact/transfer events — owned by `docs/specifications/m0-persistence-observability-and-domain-events.md`'s existing extensible catalog (Issue #5), not redefined here.

## Acceptance criteria

- Data-plane transport/resumability strategy is defined and grounded in the Spike's evidence (Issue #6 acceptance criterion).
- Storage capability model and artifact lifecycle invariants are defined (Issue #6 acceptance criterion).
- Destructive operations have a specified safety invariant tying execution to artifact `Verified` state (Issue #6 acceptance criterion).

## Validation expectations

Per `docs/development/testing.md` "Data transfer and artifact tests": interrupted transfer (partial chunk set); incomplete `.part`/`Incomplete`-state artifact rejected by any destructive consumer; digest mismatch at chunk level and at full-artifact level; duplicate chunk transfer request (idempotent, no double-write); storage exhaustion during chunk write; producer/consumer disconnect mid-chunk; restart behavior (an `Incomplete` artifact survives restart as `Incomplete`, consistent with ADR-0007's durable state); atomic `PendingVerification` → `Verified` commit; failed verification before destructive provisioning proceeds.

Per `docs/development/testing.md` "Unit and domain tests": Artifact lifecycle state-transition tests (valid and rejected transitions); chunk-manifest verification logic (chunk digest, full-artifact digest) as pure domain tests, decoupled from actual network transfer.

Per `docs/development/testing.md` "Simulator": chunked transfer at the M0 20–24 concurrent-endpoint target, including interrupted/corrupted-chunk scenarios and a simulated source-mutation scenario reproducing `docs/reference/transfer-resumability-spike.md` Experiment E's finding (a missing chunk that cannot be honestly regenerated must be reported as failed, never silently substituted).

Per "Local development environments," these are expected to run in the Linux reference environment (WSL2 or containers from Windows).

Manual: owner approval of this Specification.

## Related ADRs

- ADR-0008 — Data-plane transport, chunking, and resumability strategy (`Accepted`).
- ADR-0004 — Endpoint identity (destructive-operation preconditions consuming artifact `Verified` state).
- ADR-0006 — Job/JobStep/Attempt model (revalidation before dispatch consuming artifact state; retry policy for `Failed` artifacts).
- ADR-0007 — Persistence backend and durable/transient boundary (artifact/manifest durability; `transfer_id` correlation).

## Related work

- Issue #6 — `[WP] Define data-plane and storage contracts`.
- Issue #9 — `[Spike] Evaluate resumable volume/image transfer` (evidence this Specification applies).
- Issue #8 — `[Spike] Validate WinPE boot mechanism` (blocks the source-consistency mechanism).
- Issue #2 / ADR-0004, Issue #3 / ADR-0005 — transfer-session authentication (not designed here).
- Issue #4 / ADR-0006 — Attempt model; destructive-operation revalidation.
- Issue #5 / ADR-0007 — persistence of artifact/manifest state.
- Issue #7 — `[WP] Define Simulator contract and M0 validation strategy` (must simulate this contract).

## Open questions

1. Exact chunk size — implementation-time tuning.
2. The source-consistency/snapshot mechanism — depends on Issue #8 and further evidence, not decided here.
3. Transfer-session authentication mechanism — owned by Issues #2/#3.
4. Whether a chunk-level failure always fails the whole artifact, or whether partial-artifact recovery is ever meaningful — not evidenced, not decided here.

Status: Proposed - awaiting owner approval.
