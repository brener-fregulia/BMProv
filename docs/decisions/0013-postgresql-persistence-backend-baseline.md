# ADR-0013: PostgreSQL persistence backend baseline

Status: Accepted

## Context

ADR-0007 (persistence backend and durable/transient boundary, `Superseded by this
ADR`) accepted SQLite as the M0 persistence backend, reasoning primarily from
embedded simplicity, single-node deployment, a low expected durable-write volume,
and the repository Port/Adapter boundary as the mechanism that would defer a future
PostgreSQL adapter until a concrete trigger emerged (higher concurrent-write
pressure, remote database operation, multiple Server instances, HA, multi-site).

ADR-0007's central technical observation — **"endpoint concurrency is not
equivalent to database writer concurrency"; the relevant variable is the actual
durable write model, not raw endpoint count** — remains correct and is not
contradicted here. This ADR does not reopen that reasoning; it responds to
additional considerations ADR-0007's original evaluation did not adequately weigh.

This decision emerged while executing Issue #17 (`[WP] Establish simulated
Endpoint trust, enrollment, and Agent session`, WP1 of the M1 Milestone), during
the first real implementation checkpoint after M0's architecture/contract phase
closed. At that point:

- persistence implementation existed only as a first, uncommitted-then-committed
  checkpoint on a feature branch (`feature/wp1-endpoint-trust-enrollment-session`,
  commit `cdbebda3ee9ee2e1ca8720aa44afddae65a52ede`) — no release, no production
  database, no installed base, no data migration, and no historical compatibility
  constraint existed yet;
- inspection of that checkpoint showed the `EndpointRepository` Port/Adapter
  boundary already held cleanly: the SQLite-specific dependency (`rusqlite`) was
  confined to a single Adapter module, with zero references from Domain or
  Application code — confirming a backend replacement at this point is a
  Adapter-only rewrite, not a data-migration problem;
- Bamep is a Server/appliance product, not a lightweight desktop application — the
  "embedded, zero-external-service" argument that most strongly favors SQLite
  carries less weight for a persistent server process than it would for a
  single-user desktop tool;
- the owner has significant operational experience with, and a genuine technical
  preference for, PostgreSQL — a real maintainability/operability factor for a
  primarily solo-maintained project, in the same category of consideration ADR-0003
  already treated as legitimate ("the cost of operating a polyglot stack as a
  primarily solo-maintained project");
- Bamep's control plane (`docs/decisions/0005-agent-control-plane-protocol-and-typed-actions.md`)
  is async/Tokio-based end to end; SQLite's underlying C library is inherently
  synchronous, requiring a blocking-thread bridge somewhere in an async Rust
  server (the checkpoint's own `SqliteEndpointRepository` already needed a
  `tokio::sync::Mutex`-guarded single connection, serializing all persistence
  access behind one lock), while mature native-async PostgreSQL drivers exist in
  the Rust ecosystem and integrate directly with the Tokio runtime already in use;
- the M1 20–24 concurrent-endpoint persistence-load validation obligation
  (`docs/specifications/m0-simulator-contract-and-validation-strategy.md`
  "Persistence-load validation"; Issue #21) is exactly the empirical measurement
  that would surface a genuine SQLite writer-concurrency limitation late, after
  WP2–WP4 have already compounded the number of durable tables, migrations, and
  Work Packages built against that backend — a materially more expensive point to
  discover the need for this decision than the present one.

Full Discovery comparing SQLite and PostgreSQL specifically for Bamep's actual
persistence model, deployment profile, and evolution horizon was conducted with
the owner before this ADR; its conclusions are reflected directly in the Decision
and Alternatives-considered sections below rather than duplicated as a separate
persistent Discovery document, consistent with how ADR-0012's Context section
already compresses its own preceding analysis.

## Decision

### 1. PostgreSQL baseline

PostgreSQL is the only persistence backend supported by Bamep's current baseline.
SQLite is no longer a production/baseline persistence backend. No dual
SQLite/PostgreSQL support exists or is planned for M1 — exactly one Adapter is
built, consistent with ADR-0007's already-accepted "no dual-backend support in M0"
principle, now applied to PostgreSQL instead of SQLite.

### 2. Distribution/version policy

Bamep does not fix an architecturally-mandatory PostgreSQL major version. The
accepted policy is:

> Bamep uses a supported PostgreSQL major provided by the reference Linux
> distribution, unless a concrete Bamep requirement requires a different
> supported major.

On the current reference target (Debian 13 "trixie"), this presently means
PostgreSQL 17 — stated here as **current factual reality, not a permanent
architectural requirement**. A future Debian release shipping a newer supported
major does not require revisiting this ADR; only a concrete requirement this
policy cannot satisfy would. PGDG (`apt.postgresql.org`) is not a baseline
requirement — the reference distribution's own packages are preferred whenever
they satisfy Bamep's requirements, consistent with
`docs/specifications/m0-stack-and-boundaries-baseline.md` "Packaging and
versioning baseline" (native `.deb` packages, no requirement on an external
third-party repository).

### 3. Standalone topology

In the V1 standalone deployment profile, the Bamep Server and its PostgreSQL
instance remain co-located on the same appliance/host by default — PostgreSQL is
a local dependency of the Server, not a separately deployed service. Remote
database operation, PostgreSQL HA/clustering, and multi-site deployment remain
out of scope, unchanged from the M0 baseline
(`docs/specifications/m0-stack-and-boundaries-baseline.md` "Product boundary and
domain vocabulary"; ADR-0001's single-node V1 scope). Exact local connection
mechanics (Unix socket vs. localhost TCP, `systemd` service ordering, role/database
provisioning) are implementation-time packaging detail, not decided by this ADR,
except insofar as existing security requirements already constrain them.

### 4. Preserved ADR-0007 invariants (carried forward, authoritative here)

The following requirements were established by ADR-0007 independently of the
SQLite/PostgreSQL choice. They are **not reopened** and are restated here as
**directly authoritative under this ADR**, so no future session needs to interpret
a `Superseded` ADR-0007 to reconstruct them:

1. **Durable vs. transient/high-frequency boundary** — durable writes are bounded
   by domain-state *transitions*, not by message/sample count or endpoint count
   directly; transient/high-frequency data (Agent presence/connection state,
   `ActionProgress` ticks, general logs, high-frequency telemetry) is not written
   as one durable row per message or sample.
2. **Job/JobStep/Attempt state transitions are durable**
   (`docs/decisions/0006-job-jobstep-attempt-state-model-and-scheduling.md`).
3. **Endpoint identity/credential/hardware-confidence state transitions are
   durable** (`docs/specifications/m0-endpoint-identity-lifecycle.md`).
4. **Inventory is durably persisted only on revision change**, never on every
   report/poll cycle.
5. **Artifact/Snapshot lifecycle metadata is durable**, written on lifecycle
   transition, not per byte or per chunk.
6. **Domain events are curated, coarse-grained durable facts** describing
   completed state transitions — not a firehose of raw activity, and not an
   event-sourcing log (see point 12).
7. **Safety-relevant audit records are durable and immutable once written**
   (operator decisions; destructive-dispatch commitment and outcome).
8. **Agent presence/connectivity is transient**, independent of credential
   validity.
9. **`ActionProgress` ticks are not one durable insert per message** — latest-value
   only, in memory or a single overwritten record at most.
10. **General logs and high-frequency telemetry are not domain database
    history.**
11. **The repository Port/Adapter boundary remains mandatory** — Domain and
    Application depend only on the `repositories` Port
    (`docs/specifications/m0-stack-and-boundaries-baseline.md`); Domain/Application
    code must not depend on PostgreSQL-specific APIs, types, or query syntax
    directly, exactly as it previously must not have depended on SQLite directly.
12. **Exactly one active persistence backend** — no speculative dual-backend
    support, unchanged in principle from ADR-0007 §4, now applied to PostgreSQL.
13. **The Domain model is not constrained to a lowest-common-denominator SQL
    subset** carried merely for hypothetical portability; portability is the
    repository Port's responsibility.
14. **Atomic state+event+audit commit** — when a durable domain transition
    requires a domain event and/or an audit record, the domain-state change, its
    event, and any required audit record are persisted atomically in the same
    database transaction. A crash must never leave committed state without its
    required event/audit record, or the reverse.
15. **This is not event sourcing.** Current durable domain state remains the
    source of truth; domain events are facts persisted in the same transaction as,
    and describing, committed transitions — not a replay log Bamep reconstructs
    state from.
16. **Persist-before-send remains mandatory** for Agent dispatch
    (`ActionDispatch`, ADR-0006's `Dispatched` semantics) and for runtime
    credential issuance (ADR-0012 "Persist-before-send ordering"): the durable
    transaction commits before the Server attempts delivery over WSS. A database
    transaction and a WebSocket send are never atomic with each other, regardless
    of which database backs the transaction.

### 5. PostgreSQL-specific rationale (context, not inflated requirements)

Recorded as the reasoning for this decision, not as newly-invented product
requirements:

- genuine concurrent writers under MVCC, rather than SQLite's single-writer
  serialization — a better structural fit for the durable write pattern ADR-0007
  itself defines (state transitions across many concurrent Endpoints/Jobs/
  Attempts), and the exact property the M1 persistence-load validation (point 4 of
  ADR-0007's carried-forward list; Issue #21) exists to measure;
- native integration with Bamep's async/Tokio control plane, avoiding a
  synchronous-driver blocking-thread bridge;
- a more natural evolution path as Job/JobStep/Attempt, Artifact/Snapshot, and
  audit history accumulate real relational query needs (state filtering, joins,
  Administrative API reads) across WP2–WP4;
- richer constraints, indexing, and selective JSONB availability than SQLite
  offers, without requiring a document-oriented redesign (see "Modeling" below);
- the owner's PostgreSQL operational experience concretely lowers the
  adoption/maintenance cost that would otherwise weigh against introducing an
  external service dependency;
- the cost of migrating a future installed base (schema conversion, data
  migration tooling, upgrade/rollback paths, backup/restore conversion, integrity
  verification, support burden) would be substantially higher than the cost of
  replacing today's single-file, zero-Domain-coupling Adapter.

This ADR explicitly does **not** claim SQLite was incapable of serving the M0
20–24 concurrent-endpoint target — that empirical question was never conclusively
tested, and this decision is not conditioned on SQLite having failed it.

### 6. Modeling: relational-first, JSONB selective

Adopting PostgreSQL does **not** mean adopting document-oriented persistence.
Queryable lifecycle/correlation/state data — Endpoint identity/credential
dimensions, Job/JobStep/Attempt state, inventory revisions, domain-event
correlation fields, audit-record correlation fields — must be modeled relationally
wherever it participates in constraints, lifecycle transitions, scheduling,
reconciliation, filtering, joins, Administrative API queries, or safety decisions.
`JSONB` may be used selectively for genuinely variable/flexible payloads (e.g., an
event's type-specific `payload`, an opaque credential/assertion blob), consistent
with how `docs/specifications/m0-persistence-observability-and-domain-events.md`
already separates a domain event's indexed envelope fields from its opaque
`payload`. The M1 WP1 checkpoint's pattern of serializing an entire
`EndpointAggregate` into one JSON column is **not** carried forward as a general
persistence architecture by this ADR — it was a checkpoint-scoped implementation
shortcut, independent of the SQLite/PostgreSQL choice, and remains subject to
ordinary implementation-time review when the PostgreSQL Adapter is built. The
concrete table/schema design remains implementation-time, subject to the
constraints above and to the existing contracts (`m0-persistence-observability-and-domain-events.md`,
`m0-endpoint-identity-lifecycle.md`, `m0-job-lifecycle-and-scheduling.md`).

### 7. Driver and migration tooling

Not decided here. The Rust PostgreSQL driver/connection-pool choice (e.g., `sqlx`,
`tokio-postgres` with a pooling crate, `diesel-async`, or another option) and the
schema-migration tooling are implementation-time decisions, to be evaluated
against the actual Adapter implementation requirements before the PostgreSQL
Adapter is (re)built. Schema evolution must be versioned/controlled as the
persistence implementation matures; the concrete mechanism is not selected by
this ADR.

## Alternatives considered

1. **Keep SQLite as the long-term baseline.** Rejected: does not address the
   async-ergonomics mismatch with Bamep's Tokio-based control plane, does not
   reflect the owner's operational preference/experience, and leaves the M1
   persistence-load validation (Issue #21) as the first real test of a
   single-writer model under the exact concurrent Job/JobStep/Attempt/credential-
   rotation/domain-event/audit write pattern ADR-0007 itself identifies as the
   relevant scaling variable.
2. **SQLite now, PostgreSQL later.** Rejected. The repository Port/Adapter
   boundary isolates *Domain and Application code* from the backend choice; it
   does **not** eliminate the cost of a *future* migration once a real installed
   base exists — production data migration, schema conversion, migration tooling,
   an installer upgrade path, rollback, backup/restore conversion, integrity
   verification, and ongoing support burden. Inspection of the current WP1
   checkpoint (commit `cdbebda3ee9ee2e1ca8720aa44afddae65a52ede`) confirms the
   Adapter/Domain boundary already holds cleanly (SQLite-specific code confined to
   one module, zero Domain/Application coupling) — meaning the cost of replacing
   it *today* is close to the practical minimum this architecture can ever offer.
   That minimum only grows as WP2–WP4 add Job/JobStep/Attempt, Artifact/Snapshot,
   and audit-history persistence on top of it. Deferring the switch trades a
   near-zero cost today for a strictly larger and still-growing cost later, without
   evidence that waiting purchases anything.
3. **PostgreSQL now.** Adopted, for the reasons in "Decision" and "Context" above.
4. **Dual SQLite/PostgreSQL support.** Rejected, for the same reason ADR-0007 §4
   already rejected it in the other direction: building and maintaining two
   Adapters to validate a hypothetical need neither M0 nor M1 requires adds cost
   without present benefit.

## Consequences

- `docs/decisions/0007-persistence-backend-and-durable-transient-boundary.md` is
  marked `Superseded by ADR-0013`. Its historical Context, original evaluation,
  and "Alternatives considered" reasoning remain in place, unedited, as the
  accurate record of the original M0 decision and its reasoning; only its backend
  selection (§1) and the specific "wait for a trigger" framing of §3/§4 are
  superseded. Every invariant listed in "Preserved ADR-0007 invariants" above is
  authoritative under this ADR, not under the now-`Superseded` ADR-0007.
- `docs/specifications/m0-persistence-observability-and-domain-events.md` is
  amended to reference PostgreSQL as the current backend baseline and ADR-0013 as
  its authority, without altering the durable/transient boundary, domain-event
  model, correlation model, or auditability requirements it already defines.
- `docs/specifications/m0-endpoint-identity-lifecycle.md` is amended to remove
  backend-specific ("SQLite transaction") wording in favor of backend-neutral
  ("atomic persistence transaction") wording, without altering the credential
  chain, identity lifecycle, or destructive-operation preconditions it defines.
- `docs/decisions/0012-runtime-agent-credential-issuance-rotation-and-reconnect-recovery.md`
  is amended for the same backend-specific wording correction only. ADR-0012's
  credential-chain model (predecessor/successor, grace, replacement, confirmation,
  revocation, `SessionEstablished`) is not reopened and remains `Accepted`
  unchanged.
- `docs/specifications/m0-simulator-contract-and-validation-strategy.md` is
  amended: the persistence-load validation scenario is reframed from "SQLite
  viability" to validating the adopted persistence baseline under representative
  M1 load. The 20–24 concurrent-endpoint requirement, and the obligation to
  measure and record actual durable write volume, contention, latency, and
  backpressure rather than assume PostgreSQL is "obviously sufficient," are
  unchanged.
- `docs/specifications/m1-simulated-vertical-slice-and-baseline-validation.md` is
  amended (Scope, NF-001, Architecture constraints, Traceability) to reference
  PostgreSQL and ADR-0013 as the current persistence baseline. M1's functional
  requirements, safety invariants, and Integration Environment boundary are
  unchanged.
- Issue #17's body is amended for backend-specific wording only ("atomic SQLite
  transaction" → backend-neutral phrasing under the PostgreSQL baseline); its
  scope, acceptance criteria substance, and status (`In Progress`) are unchanged.
- Issue #21 is updated to reference ADR-0013 alongside ADR-0007 as the
  architectural source of the persistence-load validation obligation, so it does
  not depend exclusively on a now-`Superseded` ADR.
- The M1 WP1 checkpoint's SQLite Adapter (commit
  `cdbebda3ee9ee2e1ca8720aa44afddae65a52ede`, `crates/server/src/adapters/sqlite/`)
  becomes obsolete and will be replaced by a PostgreSQL Adapter in a future
  implementation round. Per owner decision, this commit does not require special
  preservation (branch/tag) beyond its existing Git history. No code is changed by
  this ADR itself.
- No Rust PostgreSQL driver, connection-pool, or migration-tooling choice is made
  by this ADR; that evaluation precedes the PostgreSQL Adapter's (re)implementation.
- Issue #21 (WP5) remains required. Its purpose changes from an implicit
  "SQLite-viability" framing to explicitly validating the adopted persistence
  baseline's behavior under the M1 20–24 concurrent-endpoint representative load;
  its acceptance criteria (report, don't silently absorb, an unacceptable result)
  are unchanged and now point at this ADR rather than ADR-0007 for the
  "revisit the persistence backend decision" contingency.

## Related architecture

- `docs/specifications/m0-persistence-observability-and-domain-events.md` — the
  durable/transient boundary, domain-event model, and auditability requirements
  this ADR carries forward.
- `docs/specifications/m0-endpoint-identity-lifecycle.md` — the credential-chain
  persistence semantics (ADR-0012) whose backend-specific wording this ADR
  corrects.
- `docs/specifications/m0-simulator-contract-and-validation-strategy.md` — the
  persistence-load validation scenario this ADR reframes without weakening.
- `docs/specifications/m1-simulated-vertical-slice-and-baseline-validation.md` —
  the M1 scope and NF-001 this ADR updates to the PostgreSQL baseline.
- `docs/specifications/m0-stack-and-boundaries-baseline.md` — the `repositories`
  Port boundary this ADR's Adapter-replacement principle depends on, and the
  packaging/versioning baseline (`.deb`, no external repository requirement) this
  ADR's distribution/version policy is consistent with.

## Related work

- ADR-0007 — Persistence backend and durable/transient boundary (`Superseded by
  this ADR`) — the original M0 decision this ADR supersedes in part and carries
  forward in part; remains the historical record of the original evaluation and
  reasoning.
- ADR-0001 — Runtime topology: modular monolith (`Accepted`) — the single-node V1
  scope this ADR's "Standalone topology" section is consistent with.
- ADR-0002, ADR-0003 — Rust across Server/Worker/Agent (`Accepted`) — the
  async/Tokio ecosystem this ADR's async-driver rationale is consistent with; not
  reopened.
- ADR-0005 — Agent control-plane protocol (`Accepted`) — the async/Tokio control
  plane this ADR's driver-ergonomics rationale references; not reopened.
- ADR-0006 — Job/JobStep/Attempt state model and scheduling (`Accepted`) — the
  durable state transitions this ADR's carried-forward invariants (point 2) cover;
  not reopened.
- ADR-0012 — Runtime Agent credential issuance, rotation, and reconnect recovery
  (`Accepted`) — the persist-before-send and atomic-transaction requirements this
  ADR carries forward (point 16); not reopened; only backend-specific wording is
  corrected in its host Specification and its own text.
- Issue #17 — `[WP] Establish simulated Endpoint trust, enrollment, and Agent
  session` — the Work Package whose first implementation checkpoint (commit
  `cdbebda3ee9ee2e1ca8720aa44afddae65a52ede`) surfaced this reconsideration, and
  whose remaining execution now targets a PostgreSQL Adapter instead of the
  SQLite Adapter that checkpoint introduced.
- Issue #21 — `[WP] Validate Simulator concurrency and M1 persistence baseline` —
  the Work Package that empirically validates this ADR's adopted backend under
  representative M1 load, discharging the same obligation ADR-0007 originally
  established for SQLite.
