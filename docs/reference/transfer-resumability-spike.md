# Transfer/Snapshot Resumability — Technical Spike Findings

Source: Issue #9, `[Spike] Evaluate resumable volume/image transfer` (M0 required Technical Spike).

These are validated empirical findings from local, disposable experiments. They are evidence for the data-plane and transfer-resumability decisions (Issue #6), not an accepted architecture or contract themselves.

## Question

Which transfer/snapshot approaches can provide meaningful resumability, integrity verification, and safe recovery for large volume/image transfers — specifically, resolving the flagged risk that "resumability must not be faked through byte offsets when the source cannot reproduce the stream from an arbitrary offset" (`docs/discovery/architecture-redesign.md` "Data plane").

## Why existing evidence was insufficient

No prior Bamep evidence establishes which resumability strategy is actually safe for Bamep's producer/consumer characteristics. The FORGE PoC evidence (`docs/reference/poc-lessons.md`) does not cover this question. This was explicitly flagged as requiring empirical investigation, not a routine implementation choice.

## Constraints and assumptions

- No real hardware or production data used; all experiments ran against disposable local files in a temporary scratch directory, deleted after the experiment.
- The final production backup/snapshot format is out of scope; this Spike evaluates *strategies*, not a final format.
- Environment: Windows 11, MSYS2/git-bash (MINGW64), coreutils 8.32, gzip 1.14, OpenSSL 3.5.4. Not the Linux reference environment (`docs/development/testing.md` "Local development environments"); results depend only on POSIX-portable tools (`dd`, `split`, `sha256sum`, `gzip`) and are expected to reproduce identically on Linux, but this was not independently confirmed on Linux in this Spike.
- Test artifact: 32 MiB of OpenSSL-generated pseudo-random data, standing in for a volume/image chunk. Random data does not compress, so no compression-*ratio* conclusions are drawn here — only resumability and integrity behavior were evaluated.

## Investigation method

Five local experiments, each simulating an interrupted transfer and a resume attempt, verified by full-artifact or per-chunk SHA-256 comparison against the known-good source:

- **A — baseline**: byte-offset resume against a static, unchanging source file.
- **B — byte-offset resume against a regenerated stream**: same source compressed, first 60% of the compressed stream "delivered," then the source is mutated (4 bytes changed near the start, simulating a live/changing disk or a non-deterministic producer) and the *tail* of a freshly-regenerated compressed stream is naively appended onto the already-delivered head, exactly as a byte-offset-trusting resume implementation would do.
- **C — chunked, content-addressed transfer**: source split into fixed-size 4 MiB chunks with a SHA-256 manifest; partial delivery simulated (some chunks missing, one corrupted); resume pass re-fetches only chunks whose hash doesn't match the manifest; reassembled artifact verified against the source.
- **D — per-chunk compression**: same as C, but each chunk is compressed independently before hashing/transfer, to test whether chunking and compression can coexist without reintroducing Experiment B's failure.
- **E — regenerating a missing chunk after the source mutates inside that chunk's own byte range**: source and original 8-chunk manifest created; one chunk (`chunk_006`) treated as never delivered; the source is then mutated at a byte offset that falls strictly inside that missing chunk's own range (not any other chunk's); the missing chunk is regenerated from the now-mutated source and compared against the *original* manifest entry for that chunk.

## Evidence collected

### Experiment A — byte-offset resume, static source: succeeded

Reading the untouched source file from an arbitrary byte offset and appending to a truncated partial copy reproduced the original file exactly (SHA-256 match). Confirms byte-offset resume is honest **only when the source is guaranteed to reproduce identical bytes at that offset**, e.g., a static file or a quiesced block device that is not being concurrently mutated.

### Experiment B — byte-offset resume, source changed between attempts: failed, and failed unsafely

- Compressed size of the original 32 MiB random source: 33,559,581 bytes (gzip).
- After changing 4 bytes near the start of the source and regenerating the compressed stream, splicing "old head" (first 60%, ~20.1 MB) with "new tail" produced a compressed artifact that:
  - **failed `gunzip -t` integrity verification** (`invalid compressed data--crc error`, `invalid compressed data--length error`), but
  - **still produced decompressed output** (33,554,434 bytes — 2 bytes off from the correct 33,554,432) when decompression was attempted without checking the CRC/length trailer.
- This is the concrete failure mode the source documentation warned about: a byte-offset "resume" of a stream whose producer cannot guarantee byte-identical regeneration **can silently produce a wrong artifact that looks approximately right in size** unless the consumer explicitly validates the gzip trailer (or, more generally, an independent content hash) rather than merely checking that decompression completed.
- This is not a claim that gzip/deflate is unusually unsafe, nor that *every* continuous transformed stream is unrecoverable in principle. The evidence applies specifically to an **unframed continuous stream with no independently verifiable restart/checkpoint boundaries**, which is what a plain gzip stream is: any divergence before a given byte propagates undetected past it unless the consumer independently re-verifies content. An explicitly seekable/framed compression format (one that defines independent restart points with their own boundaries) was not tested here and remains a design option for Issue #6 — see Conclusion 2 and 4.

### Experiment C — chunked, content-addressed transfer: succeeded, including corruption detection

- 32 MiB source split into 8 chunks of 4 MiB each, each with a SHA-256 manifest entry.
- Simulated: 5 of 8 chunks delivered correctly, 1 chunk delivered but corrupted (4 bytes overwritten), 2 chunks never delivered.
- Resume pass compared each destination chunk's hash against the manifest before deciding whether to (re)transfer it; it correctly identified and re-fetched exactly the 3 chunks that were missing or wrong (the corrupted chunk plus the 2 missing ones) and left the 5 already-correct chunks untouched.
- Reassembled artifact matched the source's full SHA-256 exactly.
- Unlike Experiment B, corruption in an already-"received" chunk was **detected before reassembly**, not silently propagated.

### Experiment D — per-chunk compression: succeeded, resolves Experiment B's failure mode

- Each of the 8 chunks compressed independently (chunk-then-compress), each with its own SHA-256 manifest entry over the *compressed* bytes.
- Same partial/corrupted delivery pattern as Experiment C, applied to the compressed chunks.
- Resume pass correctly re-fetched the 3 affected compressed chunks; every chunk independently passed `gunzip -t`; decompressing each chunk separately and concatenating reproduced the original source exactly (SHA-256 match).
- Demonstrates that resumability and compression are not in tension **as long as compression is applied per-chunk (or otherwise in independently-decodable framed units), not as one continuous stream sliced by raw byte offset**. This experiment reused the same, unmutated source for both delivery and regeneration — it shows chunked compression preserves independent verification/retransmission boundaries; it does not by itself show anything about a chunk's source becoming unreproducible (see Experiment E).

### Experiment E — regenerating a missing chunk after the source mutates inside that chunk's range: hash correctly mismatched — detection succeeded, completion did not

- Source and original manifest: 32 MiB, 8 chunks of 4 MiB, same as Experiments C/D. `chunk_006` (source byte range `[25,165,824, 29,360,128)`) treated as never delivered.
- The source was mutated (8 bytes changed) at absolute offset 26,165,824 — inside `chunk_006`'s own byte range, not any other chunk's.
- `chunk_006` was regenerated from the mutated source (`dd skip=24 count=4 bs=1M`) and compared against the *original* manifest hash recorded before the mutation.
- **Result: hash mismatch.** The regenerated chunk's SHA-256 did not match the original manifest entry for `chunk_006`.
- A sanity check confirmed the regeneration itself was correct and deterministic: independently re-splitting the mutated source produced the exact same hash for `chunk_006` as the `dd`-based regeneration (`61f73ca4...` both times) — the mismatch against the *original* manifest is attributable solely to the source change, not to a flaw in how the chunk was regenerated.
- **What this shows**: integrity verification correctly rejected the regenerated chunk — corruption/change was detected as designed. It does **not** show that the original artifact could still be completed: with the source already mutated, no regeneration of `chunk_006` from that source will ever match the original manifest again. Detecting the mismatch and completing the original artifact are two different guarantees, and this experiment demonstrates that chunking only provides the first one on its own.

## Conclusion

1. **Byte-offset resumability is honest only when the producer can deterministically reproduce identical bytes at that offset** — true for a static file or a quiesced, unchanging block device; not guaranteed for anything regenerated on demand (live disk reads during concurrent changes, non-deterministic or re-invoked compressors, any transformation pipeline without stable internal checkpoints).
2. **An unframed continuous compressed/transformed stream with no independently verifiable restart points provides no safe internal resume points** (Experiment B, gzip specifically). If a byte-offset "resume" is attempted against such a stream and the two halves are not byte-identical to what a single continuous run would have produced, the result can be a corrupted artifact that partially decodes without necessarily raising an obvious error — corruption is only reliably caught by explicit trailer/hash verification, not by "did decompression run to completion." This is not a claim that no continuous transformed stream can ever be resumed safely — an explicitly seekable/framed format that defines independent restart units was not tested and remains a design option (see 4).
3. **Fixed-size chunking with a per-chunk content hash (a manifest) provides honest integrity verification and selective retransmission when each original chunk's bytes remain reproducible or durably available** (Experiments C, D). Chunking does **not** eliminate the requirement that the source be reproducible — it changes *where* that requirement applies: from "the whole stream must be reproducible from an arbitrary byte offset" to "each individual chunk must be reproducible, or already durably staged." Experiment E shows the boundary directly: if the live source changes before a missing chunk is (re)generated, the regenerated chunk correctly *fails* the original manifest — the change is detected, but the original artifact cannot be completed from that changed source. A mutable/live source therefore still requires a consistency mechanism of some kind — for example a stable snapshot or quiesced source at capture time, durable staging/materialization of produced chunks so they no longer depend on the live source being unchanged, or another mechanism that preserves the exact bytes belonging to that capture. This Spike does not choose that mechanism; it remains for Issue #6 and future backup-format work.
4. **Chunking and compression compose safely when compression is applied per chunk**, not to the whole stream before chunking (Experiment D) — this avoids Experiment B's failure mode for the chunks that are transferred. Per-chunk compression does not, by itself, make a changed source's chunk reproducible again; it is compatible with resumable chunk transfer only to the extent the underlying logical chunk can still be reproduced or was already staged (see 3). This directly informs the data-plane Work Package: resumable transfer design should chunk first (or use an explicitly seekable/framed compression format defining its own independent restart units), never slice a single continuous compressed stream by raw byte offset — and must still address source-consistency separately from chunking/compression mechanics.
5. **Volume/Image vs. Selective backup**: this Spike tested only synthetic whole-buffer chunking and did not empirically exercise per-file Selective backup behavior. Per-file granularity (optionally with the same chunking scheme applied internally to large individual files) is a **plausible design candidate** for Selective backup in Issue #6, not an empirically established finding of this Spike. Individual files can also change during capture, so Selective backup remains subject to the same capture-consistency/reproducibility distinction established in point 3 — file-level granularity does not exempt it. This is consistent with, but does not resolve, `docs/discovery/architecture-redesign.md` "Backup model" treating Volume/Image and Selective backup as requiring independently specified strategies.

## Remaining uncertainty

- **Chunk size** was not tuned or evaluated for a trade-off (manifest overhead vs. re-transfer granularity vs. hashing cost); 4 MiB was chosen arbitrarily for this Spike's convenience. This is implementation-time tuning for Issue #6, not resolved here.
- **Digest algorithm** (SHA-256 here) was chosen for tooling convenience, not evaluated against alternatives (e.g., BLAKE3) for throughput on large volume images. Not decided here.
- **Real compressibility behavior** was not evaluated — the test source was incompressible pseudo-random data by design, appropriate for testing resumability/integrity logic but not representative of real disk-image compression ratios.
- **Live, concurrently-changing block-device reads** were not tested against real hardware or a real changing disk; Experiment B approximates this by mutating a file between two read passes, which is evidence for the *mechanism* of the risk, not a measurement of how frequently or severely a real Windows endpoint's disk changes during a live capture.
- **Network-level partial-transfer mechanics** (HTTP Range requests, WebSocket resumption, actual dropped-connection behavior) were not exercised — these experiments operated entirely on local files to isolate the resumability/integrity question from transport mechanics, consistent with this Spike's scope (transfer *strategy*, not the data-plane wire protocol itself, which is Issue #6's responsibility).
- This Spike does not evaluate Secure Boot, WinPE, or driver-provider concerns; those remain separate M0 Technical Spikes (Issues #8, #10, #11).
- **The source-consistency/snapshot mechanism itself was not evaluated or chosen.** Experiment E demonstrates that chunking alone does not solve source mutability; it does not evaluate candidate solutions (stable snapshot, quiesced source, durable chunk staging, or another approach) against each other. That choice is explicitly left for Issue #6 and future backup-format work.
- **Per-file Selective backup was not empirically tested** — the per-file granularity conclusion (5) is a design implication drawn from the chunking evidence (A–E), not a directly tested scenario.

## Related work

- Issue #9 — this Technical Spike.
- Issue #6 — `[WP] Define data-plane and storage contracts` (consumes this evidence for the resumability strategy and artifact chunk/manifest design).
- `docs/decisions/0007-persistence-backend-and-durable-transient-boundary.md` — artifact metadata durability already assumes per-artifact-lifecycle writes; a chunk manifest is consistent with, and does not conflict with, that boundary.
- `docs/discovery/architecture-redesign.md` — "Data plane", "Backup model" (the open questions this Spike was scoped to address).
