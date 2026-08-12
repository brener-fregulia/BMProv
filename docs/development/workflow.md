# Development Workflow

## Purpose

This document defines how approved BMProv work is executed.

Related responsibilities:

* `docs/development/sdd.md`: how work is discovered, specified, approved, and classified;
* `docs/development/testing.md`: testing and validation policy;
* `AGENTS.md`: mandatory repository, safety, Git, and publication rules.

The workflow should preserve owner control, traceability, safety, and continuity without unnecessary ceremony.

## Principles

* Keep `main` stable.
* Execute one approved Work Package, Technical Spike, or reduced-SDD responsibility at a time.
* Do not silently expand approved scope.
* Work should be resumable from repository and GitHub context, not conversation history.
* Relevant automated validation is part of execution completeness.
* Owner manual validation is the acceptance gate before `Done`.
* Significant architectural decisions must follow the ADR process.
* Git and publication remain owner-controlled as defined in `AGENTS.md`.

## Operational flow

Normal approved work follows:

```text
Approved work
      ↓
Ready
      ↓
Branch from main
      ↓
In Progress
      ↓
Validation
      ↓
Done
```

Execution may include implementation, documentation, architecture work, Technical Spikes, simulator work, or integration preparation.

A failed validation returns the affected work to `In Progress`.

## Starting work

Before editing:

1. identify the approved work item;
2. inspect its scope, acceptance criteria, and relevant requirements;
3. inspect related architecture and ADRs;
4. inspect relevant implementation and tests when they exist;
5. verify that no unresolved architectural or safety question blocks execution.

If persistent context is insufficient, return to the appropriate SDD stage instead of guessing.

## Ready

`Ready` means another session can begin execution without relying on previous conversation history.

Before entering `Ready`, the work should have:

* explicit scope;
* sufficient acceptance criteria;
* known architectural constraints;
* blocking decisions resolved or isolated;
* relevant safety requirements;
* known validation expectations;
* required dependencies available.

Having a GitHub Issue alone does not make work `Ready`.

A Technical Spike may be `Ready` while the Feature or decision depending on its result remains blocked.

## Branch model

Planned work normally uses a branch created from `main`.

Use:

```text
feature/<name>
fix/<name>
refactor/<name>
spike/<name>
docs/<name>
```

Examples:

```text
feature/endpoint-enrollment
fix/lease-release
refactor/job-state
spike/winpe-boot
docs/sdd-workflow
```

Work Packages normally share the branch of their parent Feature, Fix, Refactor, or documentation effort.

Do not create a branch per Work Package unless isolation or risk justifies it.

Technical Spike code normally uses `spike/<name>` and must not become production code automatically.

Reduced-SDD work may use a simpler branch strategy when explicitly chosen by the owner.

Branch creation, switching, commits, merges, pushes, pulls, tags, and publication remain subject to `AGENTS.md`.

## In Progress

`In Progress` is active execution.

During execution:

* implement only approved scope;
* preserve unrelated working-tree changes;
* follow accepted architecture and ADRs;
* add or update focused tests when applicable;
* prefer simulators, fakes, fixtures, and temporary storage at external or destructive boundaries;
* run relevant automated validation;
* review the result for regressions, safety violations, architectural violations, and accidental scope expansion.

Do not silently add unrelated:

* cleanup;
* refactoring;
* dependencies;
* formatting;
* release changes;
* architectural changes.

Report useful out-of-scope findings separately.

## Architectural decisions discovered during execution

If execution reveals a durable architectural decision with meaningful alternatives:

1. record the unresolved question;
2. inspect existing ADRs;
3. stop work that depends on the unresolved choice;
4. gather relevant alternatives and evidence;
5. use a Technical Spike if empirical evidence is required;
6. obtain owner approval;
7. create or update the ADR;
8. resume affected execution.

Implementation must not establish architectural policy silently.

Unrelated work may continue only when it does not depend on the unresolved decision.

## Technical Spikes

Technical Spikes gather evidence.

During a Spike:

* preserve the approved question;
* keep experimentation focused;
* distinguish observation from interpretation;
* record relevant environment and constraints;
* preserve failures and negative results when useful;
* avoid production-hardening experimental code unless explicitly required;
* do not expand the Spike into implementation of the final solution.

A Spike is complete when it answers its approved question sufficiently or documents why uncertainty remains.

A successful experiment does not automatically become accepted architecture.

Persist reusable conclusions in the appropriate Specification, ADR, or reference document.

## Safety-sensitive execution

Before executing destructive or privileged behavior, verify the safety requirements defined by the approved Specification or Work Package.

This particularly applies to:

* disk preparation;
* partitioning and formatting;
* deployment and restore;
* recovery artifact deletion;
* endpoint identity;
* enrollment and authentication;
* privileged execution.

Prefer safe validation through:

* fake adapters;
* simulated endpoints;
* deterministic fixtures;
* temporary storage;
* disposable test data.

Real hardware validation belongs to the Integration Environment when local simulation cannot adequately represent the risk.

Destructive real-world operations require the explicit authorization defined in `AGENTS.md`.

## Automated validation

Testing details belong in `docs/development/testing.md`.

Operationally:

1. run the narrowest relevant validation first;
2. investigate unexpected failures;
3. fix failures caused by the current work;
4. broaden validation when scope or risk requires it;
5. record actual results.

Work may leave `In Progress` when:

* approved execution scope is complete;
* required focused tests exist when applicable;
* required automated validation is not known to be failing;
* remaining manual validation is identified.

## Validation

`Validation` is the owner's manual acceptance stage.

Before handoff, report:

* what changed;
* automated validation performed and actual results;
* known limitations;
* exact manual checks remaining;
* Integration Environment requirements when applicable.

Manual validation is especially relevant for:

* PXE and UEFI behavior;
* GRUB and Alpine boot;
* Windows or WinPE;
* real network infrastructure;
* real storage devices;
* hardware-specific behavior;
* destructive workflows;
* complete provisioning flows.

Agents may define manual validation procedures but must not complete owner validation on the owner's behalf.

If validation fails:

```text
Validation
    ↓
In Progress
    ↓
correction + validation
    ↓
Validation
```

## Done

`Done` means the owner accepted the work.

Preserve only outcome information that remains useful.

When applicable:

```text
Outcome
- implemented or validated result;
- relevant deviation from the approved plan.

Automated validation
- relevant checks and results.

Manual validation
- relevant scenarios and owner result.

Related changes
- ADRs, architecture, Specifications, or reference material updated.
```

Do not preserve conversation transcripts or duplicate information already obvious from the repository.

Parent work is complete only when its required child work and acceptance criteria are complete.

## Session handoff

Before ending unfinished work, update the appropriate persistent source with:

* current status;
* completed work;
* remaining work;
* validation results;
* relevant evidence;
* blockers;
* unresolved architectural or safety questions.

Use the existing Issue, Work Package, Specification, Spike, or other authoritative source.

Do not create a separate handoff document when one is unnecessary.

## Review

Review compares executed work against:

* approved scope;
* acceptance criteria;
* Specifications;
* ADRs;
* implemented architecture;
* safety invariants;
* tests and validation.

Prioritize findings involving:

* correctness;
* data loss;
* destructive-operation safety;
* identity and authorization;
* stale state;
* regressions;
* architectural violations;
* missing error handling;
* missing tests;
* unintended coupling;
* out-of-scope changes.

Review is read-only unless corrections are explicitly requested.

## Commit strategy

Use concise Conventional Commits.

Examples:

```text
feat(agent): add enrollment handshake
fix(scheduler): release lease after cancellation
refactor(jobs): separate transition validation
test(protocol): cover duplicate acknowledgement
docs(sdd): define technical spike workflow
```

Implementation and directly related tests may share one commit when they form one coherent change.

Use a dedicated `test(...)` commit when test work is independently useful.

Do not split or combine work merely for ceremony.

Detailed execution history belongs in the relevant work item, not the commit message.

Agents may suggest commits but must not execute them without explicit authorization.

## GitHub workflow state

GitHub Projects uses:

```text
Backlog
Ready
In Progress
Validation
Done
```

Statuses must reflect actual state.

Do not:

* move incomplete work to `Ready`;
* move work with known required validation failures to `Validation`;
* move work to `Done` before owner acceptance.

GitHub represents operational state. It does not replace repository documentation, ADRs, Specifications, or implementation.

## Documentation during execution

Update permanent documentation when completed work changes information useful beyond the current task.

Examples:

* accepted architectural decisions;
* implemented architecture;
* reusable compatibility findings;
* engineering process changes.

Do not duplicate the same information across Issues, Specifications, ADRs, architecture, and reference documents.

Planned architecture does not belong in `docs/architecture/` until implemented.

Detailed placement rules belong in `docs/development/documentation-policy.md`.

## Reduced workflow

Reduced SDD may use:

```text
scope confirmation
      ↓
execution
      ↓
proportional validation
      ↓
owner validation when relevant
      ↓
Done
```

Reduced workflow does not bypass:

* repository inspection;
* scope control;
* architecture constraints;
* safety requirements;
* validation;
* owner-controlled Git and publication rules.

If meaningful ambiguity, architecture impact, or safety risk appears, return to the normal SDD lifecycle.

## Guiding rule

By the time work reaches `Ready`, important questions about intent, scope, architecture, safety, and validation should already be answered or explicitly isolated.

The workflow exists to execute approved decisions reliably, not to make hidden decisions during implementation.
