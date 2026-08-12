# Specification-Driven Development

## Purpose

BMProv uses Specification-Driven Development (SDD) so important work can be understood, approved, implemented, validated, and resumed without depending on conversation history or a specific AI session.

The repository stores permanent technical context.

GitHub stores approved work, operational workflow state, and milestone or release scope.

The process exists to preserve:

* explicit requirements;
* architectural decisions;
* safety constraints;
* scope control;
* validation expectations;
* continuity across development sessions.

Use only as much process as necessary to preserve those properties.

## Sources of truth

### Repository

The repository is the permanent technical source of truth.

Relevant responsibilities include:

* implementation and tests: currently implemented behavior;
* `AGENTS.md`: mandatory repository-wide agent rules;
* `docs/discovery/`: investigation and evidence gathered before decisions;
* `docs/specifications/`: persistent approved specifications when repository-level documentation is appropriate;
* `docs/architecture/`: currently implemented architecture;
* `docs/decisions/`: Architectural Decision Records;
* `docs/development/`: engineering process;
* `docs/reference/`: factual technical knowledge, compatibility findings, and validated external constraints.

`docs/architecture/` must describe implemented reality, not intended future architecture.

### GitHub

After approved work is materialized:

* Issues represent approved work such as Features, Fixes, Refactors, Technical Spikes, and Work Packages;
* Projects represent workflow state and progress;
* Milestones group milestones or release scope when useful.

An AI session or chat must never be the only place containing information required to continue, validate, review, or understand relevant work.

## Lifecycle

The normal BMProv lifecycle is:

```text
Idea
  ↓
Discovery
  ↓
Specification
  ↓
Owner approval
  ↓
GitHub materialization
  ↓
Execution
  ↓
Automated validation
  ↓
Owner manual validation
  ↓
Done
```

Not every change requires every formal artifact.

The hierarchy and process should remain as small as possible while preserving useful context, decisions, safety, and traceability.

## Discovery

Discovery is analysis, not implementation.

Its purpose is to establish enough evidence to define the problem correctly before selecting or implementing a solution.

Discovery should, when relevant:

1. understand the requested outcome;
2. inspect current implementation and nearby tests when they exist;
3. inspect relevant architecture documentation and ADRs;
4. inspect relevant historical or reference material;
5. distinguish current behavior from proposed behavior;
6. identify affected responsibilities and boundaries;
7. identify constraints and explicit non-goals;
8. identify safety implications;
9. identify unresolved architectural decisions;
10. determine validation expectations;
11. determine whether work decomposition is useful;
12. determine whether empirical evidence requires a Technical Spike.

Do not invent requirements to fill gaps.

When evidence is insufficient, preserve the uncertainty explicitly.

Discovery may produce:

* a Specification proposal;
* one or more architectural questions;
* a proposed Technical Spike;
* a recommendation that no implementation work is currently justified.

Discovery findings that remain useful beyond one session should be persisted in the repository or approved GitHub work rather than remaining only in conversation history.

## Technical Spikes

A Technical Spike is a focused empirical investigation used when existing evidence is insufficient to make a specification or architectural decision confidently.

Examples may include:

* evaluating a WinPE boot mechanism;
* determining whether a backup format can support meaningful resumability;
* validating Secure Boot behavior;
* measuring resource usage or throughput;
* testing PXE, firmware, network, or storage compatibility.

A Technical Spike must define, as applicable:

* the question being investigated;
* why existing evidence is insufficient;
* relevant constraints and assumptions;
* the experiment or validation method;
* success or evaluation criteria;
* evidence collected;
* conclusion;
* remaining uncertainty.

Spike implementation is experimental.

It must not be treated as production architecture merely because it works.

A Spike may inform:

* a Specification;
* an ADR;
* an architecture constraint;
* a future Work Package.

It must not silently establish architectural policy.

Reusable factual findings should be moved to the appropriate permanent source, such as `docs/reference/`, instead of preserving experimental notes indefinitely as the only source of truth.

## Specification

A Specification defines intended behavior before execution begins.

Before owner approval, a Specification is a proposal.

After owner approval, it becomes the approved specification for that scope.

Use only sections that contribute useful information.

Typical sections include:

* Classification
* Context
* Current behavior
* Goal
* Scope
* Out of scope
* Functional Requirements (`RF-###`)
* Non-Functional Requirements (`RNF-###`)
* Business Rules (`RN-###`)
* Safety invariants
* Acceptance Criteria
* Architecture impact
* Related ADRs
* Technical Spikes
* Work Package decomposition
* Automated validation expectations
* Manual validation expectations
* Open questions

Not every Specification needs RF, RNF, and RN sections.

Do not create empty categories merely to satisfy a template.

Requirements must describe intended behavior or constraints, not implementation choices unless the implementation choice has already been accepted as an architectural constraint.

Implementation must not begin merely because a Specification draft exists.

## Work classification

Use the smallest classification that accurately represents the work.

### Epic

An Epic is an optional grouping for a larger objective that contains multiple related Features and benefits from shared context.

Do not create an Epic when one Feature is sufficient.

### Feature

A Feature introduces one coherent product capability or behavior.

A Feature may contain multiple Work Packages when implementation, review, or validation benefits from decomposition.

### Fix

A Fix corrects existing behavior that does not match its intended or specified behavior.

Complex Fixes may be decomposed into Work Packages.

### Refactor

A Refactor changes internal structure while preserving intended externally observable behavior unless its approved Specification explicitly states otherwise.

Architectural refactors still require appropriate Discovery and ADR handling when they change durable boundaries.

### Technical Spike

A Technical Spike gathers evidence required before a decision or Specification can be completed.

A Spike is not production implementation by default.

### Work Package

A Work Package is the smallest planned execution unit.

It must contain enough persistent context for another development session to execute and validate it without relying on the conversation that created it.

Work Packages do not need to map one-to-one to commits.

## Work hierarchy

Use only as much hierarchy as necessary.

Possible structures include:

```text
Milestone
├── Feature
│   ├── Work Package
│   └── Work Package
├── Fix
│   └── Work Package
├── Refactor
│   └── Work Package
└── Technical Spike
```

Or, when shared context justifies it:

```text
Milestone
└── Epic
    ├── Feature
    │   └── Work Package
    └── Feature
        └── Work Package
```

Do not create hierarchy merely for ceremony.

## Work Package definition

A Work Package should contain only the information needed to execute and validate that unit of work.

Useful sections include:

```text
Objective
Scope
Out of scope
Related requirements
Relevant architecture
Related ADRs
Safety constraints
Acceptance criteria
Implementation notes
Automated validation
Manual validation
Outcome
```

A Work Package must not silently absorb responsibilities from another package or from unapproved future work.

If execution reveals that its scope is incomplete or architecturally incorrect, stop and return to the appropriate SDD stage instead of expanding implementation implicitly.

## Owner approval

Owner approval is the boundary between proposed work and approved work.

Approval is required before:

* materializing planned work as approved execution scope;
* beginning non-trivial planned implementation;
* accepting a significant architectural decision;
* expanding an approved Specification in a materially different direction.

Approval may be explicit in conversation or represented by approved persistent project state.

Do not infer approval merely because a proposal was discussed.

Material changes after approval must be surfaced rather than silently incorporated.

## GitHub materialization

GitHub materialization turns approved plans into operational work items.

Materialize only approved work.

Do not populate GitHub with speculative implementation tasks before Specification exists.

Materialization may create:

* Feature Issues;
* Fix Issues;
* Refactor Issues;
* Technical Spike Issues;
* Work Package Issues;
* Milestone relationships;
* Project items.

ADRs themselves remain permanent repository documents under `docs/decisions/`.

GitHub work may track investigation or implementation related to an ADR, but the Issue is not the authoritative architectural decision record.

## Status model

The default workflow states are:

| Status      | Meaning                                                                                                    |
| ----------- | ---------------------------------------------------------------------------------------------------------- |
| Backlog     | Work is known but definition, priority, dependencies, evidence, or ordering may remain unresolved.         |
| Ready       | Approved context is sufficient for another session to start without relying on prior conversation history. |
| In Progress | Execution and relevant automated validation are active.                                                    |
| Validation  | Execution and required automated validation are complete; owner manual validation is pending.              |
| Done        | The owner accepted the work after validation.                                                              |

A Work Package should enter `Ready` only when its persistent context is sufficient for execution.

Known required automated validation must not be failing when work moves to `Validation`.

A problem discovered during manual validation returns the affected work to `In Progress`.

## Execution

Execution follows approved scope.

Before making changes:

1. identify the approved work item;
2. inspect its Specification or Work Package;
3. inspect relevant architecture and ADRs;
4. inspect the current implementation and nearby tests;
5. verify that no unresolved question blocks execution.

During execution:

* implement only approved scope;
* preserve established boundaries;
* add or update relevant automated tests;
* use fakes and simulation at external or destructive boundaries where appropriate;
* record meaningful deviations instead of silently changing the plan;
* stop when a new durable architectural decision is required.

Execution may include documentation, Technical Spikes, architecture work, or implementation depending on the approved work.

Production code is not required for every SDD work item.

## Architectural decisions during execution

If Discovery or execution reveals a durable architectural decision with meaningful alternatives:

1. record the question in the current Specification, Spike, or Work Package;
2. inspect existing ADRs;
3. stop the affected architectural choice;
4. identify relevant alternatives and trade-offs;
5. obtain owner approval for the decision;
6. create or update the ADR;
7. continue work according to the accepted decision.

Agents must not establish architectural policy silently through generated code.

Accepted ADRs are current architectural constraints.

Reconsider an accepted ADR only when new requirements, evidence, or constraints justify doing so.

When a decision changes, preserve decision history rather than rewriting accepted historical reasoning as if the previous decision never existed.

## Safety-sensitive work

BMProv includes operations capable of modifying or destroying endpoint data and operating system installations.

Specifications involving any of the following require explicit safety treatment:

* disk preparation;
* partition modification;
* formatting;
* image deployment;
* backup deletion;
* restore operations;
* endpoint identity;
* enrollment and authentication;
* recovery artifacts;
* destructive storage mutation;
* privilege boundaries.

Relevant Specifications or Work Packages must define, when applicable:

* safety invariants;
* preconditions;
* identity assumptions;
* authorization requirements;
* stale-state handling;
* interruption behavior;
* retry semantics;
* cancellation semantics;
* recovery behavior;
* required verification before destructive execution.

Generic retry policies must never imply that destructive operations are automatically safe to replay.

When simulator or fake-based validation cannot adequately represent a destructive or hardware-dependent risk, define explicit Integration Environment and owner-manual validation.

## Automated validation

Relevant automated validation is part of execution completeness.

The exact testing policy belongs in `docs/development/testing.md`.

Validation should be proportional to the affected behavior and risk.

Depending on scope, it may include:

* domain tests;
* state-machine tests;
* protocol contract tests;
* adapter contract tests;
* simulator scenarios;
* integration tests;
* safety regression tests;
* data-transfer interruption tests;
* frontend tests.

Testing must verify specified behavior.

Do not use tests to invent missing requirements.

## Owner manual validation

Owner manual validation is the acceptance gate before `Done`.

Manual validation is particularly important when behavior depends on:

* PXE;
* firmware;
* GRUB;
* Alpine boot environments;
* real storage devices;
* network hardware;
* Windows or WinPE;
* destructive provisioning;
* hardware-specific compatibility;
* complete workflows that active automated layers cannot represent reliably.

Agents may define and report manual validation procedures.

Agents must not claim that owner validation has been completed on the owner's behalf.

## Outcome record

Before work reaches `Done`, preserve only outcome information that remains useful for future development, review, or release preparation.

A concise record may include:

```text
Outcome
- implemented or validated result;
- relevant deviation from the approved plan.

Automated validation
- relevant commands, scenarios, and results.

Manual validation
- relevant scenarios and owner result.

Related changes
- ADRs, architecture documents, or reference material updated.
```

Do not reproduce the code diff or conversation transcript.

## Session handoff

When a session ends with unfinished work, persistent project context must be sufficient for another session to continue.

Update the relevant Specification, Work Package, Spike, Issue, or other authoritative source with:

* current status;
* completed work;
* remaining work;
* relevant evidence;
* validation results;
* unresolved blockers;
* unresolved architectural questions.

The next session reconstructs context from the repository and GitHub rather than assuming previous conversation history.

## Reduced SDD

Small, isolated, low-risk changes may use a reduced process when they do not require meaningful Discovery, decomposition, architectural decisions, or safety analysis.

Examples may include:

* typo corrections;
* broken documentation links;
* minor documentation wording corrections;
* trivial configuration maintenance;
* small reproducible fixes with obvious scope and no architectural impact.

Reduced SDD may use:

```text
scope confirmation
→ execution
→ proportional automated validation
→ owner manual validation when relevant
→ Done
```

Reduced SDD must not be used to bypass:

* required safety analysis;
* architectural decisions;
* significant requirements;
* meaningful scope ambiguity;
* destructive-operation validation;
* repository inspection;
* owner-controlled Git or publication rules.

## Guiding rule

Use enough process to preserve:

* intent;
* evidence;
* scope;
* architectural reasoning;
* safety;
* validation;
* continuity.

Do not add hierarchy, documentation, or ceremony that provides no useful project context.

The purpose of SDD is not to produce documents.

The purpose of SDD is to ensure that important BMProv behavior and decisions are explicit before they become difficult to change.
