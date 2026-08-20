# Persistence

## Purpose

This document defines the current Bamep Server persistence and schema-evolution
conventions: PostgreSQL/SQLx usage, ORM policy, the Domain/persistence boundary,
query style, and migration rules.

It records ordinary implementation conventions, not a new architectural decision.
The backend choice itself is decided by
`docs/decisions/0013-postgresql-persistence-backend-baseline.md` (ADR-0013).

Related responsibilities:

* `AGENTS.md`: mandatory repository-wide agent rules;
* ADR-0013: PostgreSQL as the only persistence backend, driver/migration tooling
  left as an implementation-time choice;
* `docs/development/testing.md`: general testing and validation policy;
* `docs/development/workflow.md`: execution workflow.

## PostgreSQL and SQLx

* PostgreSQL is the only supported persistence backend in the current baseline,
  per ADR-0013.
* SQLx is the current Rust PostgreSQL toolkit used by Bamep Server
  (`crates/server/Cargo.toml`).
* Do not add another persistence framework or toolkit without a concrete
  requirement.
* Do not add SQLite/MySQL support or a database-abstraction layer for
  hypothetical portability. ADR-0013 explicitly rejects dual-backend support.
* SQLx/PostgreSQL-specific types (`sqlx::PgPool`, `sqlx::Row`,
  `sqlx::Transaction`, etc.) must remain inside the PostgreSQL Adapter
  (`crates/server/src/adapters/postgres/`) and must not leak into Domain or
  Application code, consistent with the repository Port/Adapter boundary.

## ORM policy

Bamep currently does **not** use an ORM.

Explicit SQL behind the Repository Port / PostgreSQL Adapter boundary is the
baseline.

Do not introduce an ORM merely to reduce SQL boilerplate.

An ORM may be reconsidered later only if a concrete requirement demonstrates a
clear benefit worth the additional abstraction and dependency. This is a current
baseline choice, not a permanent prohibition.

## Domain versus persistence model

Preserve:

```text
database row/model != Domain entity
```

Persistence DTO/row structs may exist inside the PostgreSQL Adapter when useful
for mapping query results.

Domain types must not gain SQLx/ORM derives or PostgreSQL-specific annotations
merely to make persistence easier.

Queryable lifecycle, correlation, and safety state is relational first-class
data (real columns, constraints, indexes) — see ADR-0013 "Modeling:
relational-first, JSONB selective".

`JSONB` is reserved for genuinely variable payloads (for example an event's
type-specific `payload`), not a shortcut for serializing whole aggregates.

## Closed categorical values

A durable column that holds a closed, low-cardinality vocabulary — a fixed
set of state/type/kind labels that is part of the persistence/Domain
contract (e.g. an identity-lifecycle state, a domain-event type, an actor
kind) — should not default to repeated free-form `TEXT`.

* Prefer a native PostgreSQL `ENUM` type when the vocabulary is
  intentionally closed. It gives a compact fixed-size internal
  representation (avoiding repeated textual labels in large tables/indexes),
  keeps SQL human-readable, and lets PostgreSQL itself enforce the closed
  set.
* `TEXT` remains appropriate for open-ended values (free-form labels,
  descriptive/error detail, arbitrary identifiers).
* A numeric code (e.g. `SMALLINT`) requires a demonstrated storage/
  performance need before use — it saves little over `ENUM` while losing
  SQL readability and semantic clarity.
* PostgreSQL `ENUM` evolution (adding/renaming/removing a label) is itself
  schema evolution and goes through a versioned migration like any other
  schema change — never an implicit/ad-hoc value.
* SQLx/PostgreSQL enum representations (`#[derive(sqlx::Type)]`) stay
  inside the PostgreSQL Adapter, mapped explicitly to/from the Domain type;
  they must not leak into Domain, consistent with "Domain versus
  persistence model" above.

## Query style

The current baseline uses runtime-checked SQL:

* `sqlx::query`;
* `sqlx::query_scalar`;
* binds and `Row` mapping where appropriate.

Do not require the `query!` / `query_as!` compile-time macros as the project
baseline today.

Reason: Bamep should remain buildable without requiring a live database or
generated `.sqlx` metadata merely to compile.

Compile-time SQL checking may be reconsidered later if its maintenance/build
trade-off becomes worthwhile.

Do not remove the `macros` SQLx feature: `sqlx::migrate!` (used in
`crates/server/src/adapters/postgres/mod.rs` to embed migrations at build time)
currently requires it.

## Migrations

1. Schema changes use versioned SQL migrations.
2. Migrations live under `crates/server/migrations/`.
3. Do not hide schema evolution in Rust startup strings or ad-hoc `ALTER` logic.
4. Use readable monotonic names, e.g. `0001_wp1_endpoint_enrollment.sql`,
   `0002_add_boot_context.sql`.
5. An applied/released migration is immutable. Never edit a historical
   migration to change already-shipped schema behavior; create a new migration
   instead.
6. Migrations are forward-only as Bamep's normal production evolution policy. A
   bad applied migration is repaired with a new forward migration rather than
   relying on a production down-migration workflow.
7. Migrations should be transactional by default when PostgreSQL supports the
   operation.
8. A deliberately non-transactional migration requires explicit justification
   and appropriate validation.
9. Server startup applies pending embedded migrations
   (`sqlx::migrate::Migrator`) before the Server becomes operational.
10. Migration failure must fail startup clearly rather than allowing operation
    on an incompatible partial schema.
11. No external SQLx CLI should be required at Server runtime; migrations are
    compiled in and applied by the Server itself.
12. Constraints that protect durable invariants should exist in PostgreSQL when
    appropriate (`NOT NULL`, `UNIQUE`, foreign keys, `CHECK`, indexes), not only
    as Rust-side assumptions.
13. Destructive or compatibility-sensitive migrations (`DROP`, incompatible
    type changes, large data rewrites, etc.) require explicit
    upgrade/backup/recovery consideration before implementation.

Bamep does not yet have a specified production backup/version-retention policy;
do not assume or invent one when writing a migration.

## Migration testing

* Component/integration persistence tests use real PostgreSQL, per
  `docs/development/testing.md`.
* Migrations must apply cleanly to a fresh disposable test database.
* Test databases must be isolated and safely disposable.
* Do not use SQLite or in-memory behavior as proof of PostgreSQL behavior.
* As released upgrade paths begin to exist, migrations should also be
  validated from supported prior schema/release state to current state.
* Do not claim that upgrade-path testing already exists if it does not.

## SQL file line endings

Because development occurs on both Windows and Linux, SQL migration files must
use LF consistently so migration checksums do not vary because of checkout line
endings.

`.gitattributes` enforces this narrowly for migration files:

```text
crates/server/migrations/*.sql text eol=lf
```

Do not broadly change unrelated line-ending policy when working in this area.

## Guiding rule

Keep the persistence layer boring: explicit SQL, relational-first modeling,
versioned forward-only migrations, and a strict Adapter boundary that keeps
SQLx and PostgreSQL specifics out of Domain and Application code.
