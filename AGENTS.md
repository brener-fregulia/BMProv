# AGENTS.md

## Purpose

This file defines the mandatory rules for any AI agent working in the Bamep repository.

Tool-specific instructions belong in files such as `CLAUDE.md` and `.claude/`.
Detailed procedures belong in `docs/development/`.

`README.md` is public product documentation and must not be used as a source of agent instructions.

## Sources of truth

The repository is the permanent source of Bamep technical context.

Implementation and tests are the source of truth for currently implemented behavior.

After approved work is materialized:

- GitHub Issues store approved specifications and Work Packages;
- GitHub Projects store operational workflow state and progress;
- GitHub Milestones group milestones or releases when applicable.

An AI session must never be the only place containing information required to understand, continue, validate, or maintain relevant work.

Before proposing or making changes:

- inspect the relevant implementation and nearby tests when they exist;
- read only the documentation needed for the task;
- inspect relevant architecture documents and ADRs;
- verify paths, commands, configuration, and conventions in the repository;
- report conflicts between the request, specification, ADRs, documentation, and actual repository state;
- do not invent files, APIs, behavior, requirements, commands, conventions, or validation results.

## Scope and SDD

Follow `docs/development/sdd.md`.

- Discovery is analysis, not implementation.
- Non-trivial work must have sufficient specification before implementation.
- Explicit owner approval is required before approved work is materialized or planned implementation begins.
- Implement one approved Work Package or one reduced-SDD responsibility at a time.
- Do not silently expand approved scope.
- Significant architectural decisions must not emerge silently through implementation.
- Relevant automated tests are part of implementation completeness.
- `Validation` is the owner manual validation stage before `Done`.

Use only as much process as necessary to preserve scope, decisions, validation, and continuity.

## Repository protection

Preserve existing working-tree changes, including changes not created by the agent.

- Never discard, overwrite, revert, or reformat unrelated work.
- Inspect a file before replacing or deleting it.
- Do not modify generated files, vendored dependencies, build output, or local configuration unless explicitly required.
- Prefer changing the responsible source or generator instead of generated output.
- Do not expose, store, or print secrets, credentials, signing keys, tokens, or private environment values.
- Do not weaken checks, tests, warnings, or security controls to make a task pass.

## Architecture and dependencies

`docs/architecture/` documents only architecture that actually exists.

Do not describe planned architecture as if it were already implemented.

Before introducing or changing a module, abstraction, service, adapter, worker, boundary, protocol, or dependency:

1. identify the current requirement that justifies the change;
2. identify the correct architectural responsibility;
3. inspect existing patterns and nearby solutions;
4. inspect relevant ADRs;
5. preserve accepted decisions or explicitly propose changing them.

Bamep must not inherit stacks, directories, protocols, runtime boundaries, or architectural patterns from FORGE, Pascoal, or any other project without justification based on Bamep's own requirements.

Do not introduce dependencies merely for convenience. Evaluate their impact on maintenance, deployment, security, runtime footprint, and operational support.

## Safety

Bamep will perform operations capable of modifying or destroying data and operating system installations.

Safety takes precedence over implementation convenience.

- Never weaken identity, inventory, authorization, or destructive-operation safeguards to make a workflow pass.
- Destructive operations must have explicit preconditions and safety invariants.
- A MAC address is an inventory signal, not authentication and not a permanent endpoint identity.
- Do not use unrestricted remote shell execution as a substitute for typed Agent actions.
- Do not execute real destructive filesystem, partitioning, formatting, deployment, or data operations without explicit and specific owner authorization for that environment and target.
- Automated tests must use appropriate fakes, fixtures, temporary storage, simulators, or disposable devices.
- Real hardware and destructive operations belong to the integration layer when they cannot be represented safely in local development.

## Development environment

The physical Bamep server and laboratory are an Integration Environment, not a required development environment.

Most development must be possible locally without:

- a physical Bamep server;
- real PXE infrastructure;
- MikroTik hardware;
- real client endpoints;
- destructive disks;
- production storage.

Use simulators, fake adapters, temporary storage, and deterministic fixtures at appropriate boundaries.

Linux is the primary development environment and the production target for Bamep Server.

Portable parts should remain reasonably developable and testable on Windows 11 when doing so does not compromise Linux-specific responsibilities.

Do not create artificial abstractions merely to pretend inherently Linux-specific responsibilities are platform-independent.

## Git and publication

The repository owner retains control of Git and publication.

Inspection commands such as `git status`, `git diff`, `git log`, and `git show` are allowed when relevant.

Unless explicitly and specifically authorized for the current task, do not perform Git or GitHub operations that modify:

- the working tree or index;
- branches or tags;
- commit history;
- remotes or synchronization state;
- pull requests;
- releases;
- publication state;
- GitHub Project state.

This includes staging, commits, amendments, checkout or restore operations, branch creation, merges, rebases, resets, stashes, pulls, pushes, tags, and release publication.

A request to implement, test, review, or document something does not implicitly authorize publication or Git state changes.

After local changes, suggest a Conventional Commit message when useful, but do not execute it without explicit authorization.

## Validation

Use the narrowest validation that meaningfully demonstrates the changed behavior, and broaden validation when risk or scope justifies it.

Follow `docs/development/testing.md`.

- Do not claim a test, build, lint, check, or validation passed unless it was actually executed.
- Do not hide failures or weaken checks.
- Do not increase timeouts, disable cases, or add retries merely to mask failures without understanding the cause.
- When evidence allows, distinguish failures caused by the current change from environment limitations or pre-existing repository failures.
- Clearly report which automated validations were executed and which manual checks remain.
- Never claim owner manual validation was completed on the owner's behalf.

## Documentation

Use documentation according to its responsibility.

Primary locations:

- `README.md`: public product overview;
- `docs/discovery/`: discovery and investigation;
- `docs/specifications/`: persistent specifications when appropriate;
- `docs/architecture/`: currently implemented architecture;
- `docs/decisions/`: architectural decisions and ADR history;
- `docs/development/`: engineering process;
- `docs/reference/`: factual knowledge, compatibility notes, and technical reference material.

Detailed documentation ownership belongs in `docs/development/documentation-policy.md` when that document exists.

Each piece of information should have one primary source. Avoid maintaining the same information in multiple places.

## Language

Use English for repository content, including:

- source code;
- identifiers;
- source filenames where appropriate;
- comments;
- docstrings;
- schemas;
- APIs;
- protocol fields;
- internal logs;
- domain events;
- architecture documentation;
- ADRs;
- Discovery;
- specifications;
- SDD;
- workflow documentation;
- testing documentation;
- reference documentation;
- GitHub Issues and Work Packages.

User-facing UI text must use localization boundaries rather than scattered hardcoded strings.

The initial UI locale is `pt-BR`.

The planned additional locale is `en-US`.

Academic and TCC-facing material may be written separately in Brazilian Portuguese and must not become a second authoritative copy of the engineering documentation.

## Final response

After changing files, report at minimum:

- a summary of the changes;
- files changed;
- validation actually performed and its results;
- limitations and remaining manual checks;
- relevant out-of-scope findings without implementing them;
- one suggested Conventional Commit message when appropriate.

When no files were changed, state that clearly.

## Instruction precedence

When instructions conflict, use this order:

1. safety, data protection, and prevention of destructive operations;
2. explicit owner instructions for the current task;
3. this `AGENTS.md`;
4. tool-specific instructions;
5. relevant project documentation;
6. established implementation patterns.

An operation that is normally restricted requires explicit, specific, and task-limited authorization. It must not be inferred implicitly.
