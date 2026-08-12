---
name: implement-feature
description: Implement one approved BMProv Work Package or reduced-SDD task with focused automated validation and strict scope control.
argument-hint: "[approved Work Package or reduced-SDD task]"
disable-model-invocation: true
---

# Implement feature

Implement:

$ARGUMENTS

## Purpose

Use this skill to execute one approved BMProv Work Package or one reduced-SDD responsibility.

This skill assumes planning and approval already exist.

It must not invent scope, bypass unresolved architecture, or absorb unrelated work.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/sdd.md`;
   - `docs/development/workflow.md`;
   - `docs/development/testing.md`;
   - relevant Specifications;
   - relevant ADRs;
   - relevant architecture documentation.

2. Reconstruct the task from persistent repository and GitHub context.

3. Confirm that the requested execution corresponds to:
   - one approved Work Package; or
   - one valid reduced-SDD task.

4. Inspect:
   - affected implementation;
   - nearby tests;
   - relevant configuration;
   - related contracts;
   - existing patterns.

5. Verify before editing that:
   - scope is sufficiently defined;
   - relevant blocking ADRs are accepted;
   - required Technical Spikes are complete or explicitly isolated;
   - safety-sensitive behavior has explicit constraints;
   - no unresolved question blocks the implementation.

6. Implement only the approved scope.

7. Add or update the focused automated tests required by the changed behavior.

8. Use appropriate safe boundaries:
   - fakes;
   - fixtures;
   - temporary storage;
   - Simulator;
   - controlled local integrations.

9. Run the narrowest relevant automated validation first.

10. Broaden validation when:
    - shared behavior changed;
    - contracts changed;
    - persistence changed;
    - concurrency changed;
    - destructive behavior changed;
    - risk justifies broader coverage.

11. Review the resulting changes for:
    - correctness;
    - scope expansion;
    - safety violations;
    - architectural violations;
    - regressions;
    - missing tests;
    - missing failure handling.

12. Report the work as ready for owner validation only when required automated validation is not known to be failing.

## Scope control

Implement exactly the approved responsibility.

Do not silently add:

- unrelated refactoring;
- cleanup;
- dependency upgrades;
- formatting sweeps;
- speculative abstractions;
- future features;
- release changes;
- unrelated documentation;
- architecture changes.

Report useful out-of-scope findings separately.

If implementation reveals that the approved scope is incomplete, stop and surface the issue instead of expanding it implicitly.

## Architectural decisions

If execution reveals a durable architectural decision with meaningful alternatives:

1. record the unresolved decision;
2. inspect existing ADRs;
3. stop implementation that depends on the choice;
4. gather relevant evidence;
5. recommend a Technical Spike when empirical evidence is required;
6. return the decision for owner approval;
7. resume only after the appropriate ADR process is complete.

Do not establish architecture silently through code.

Do not reinterpret an accepted ADR merely because another implementation appears easier.

## Safety-sensitive implementation

Apply additional caution when the work involves:

- endpoint identity;
- enrollment or authentication;
- privileged Agent behavior;
- disk selection;
- partitioning;
- formatting;
- operating system deployment;
- backup or recovery;
- artifact deletion;
- destructive retries;
- recovery state.

Before implementing destructive behavior, verify the approved specification defines relevant:

- preconditions;
- safety invariants;
- identity assumptions;
- authorization requirements;
- stale-state handling;
- retry semantics;
- cancellation behavior;
- interruption behavior;
- recovery behavior.

Do not invent missing destructive-operation policy while implementing.

Never use unrestricted remote shell execution as a shortcut around typed Agent actions.

## Testing

Follow `docs/development/testing.md`.

Relevant automated tests are part of implementation completeness.

Prefer the smallest appropriate layer:

- domain;
- state machine;
- contract;
- persistence integration;
- adapter contract;
- Simulator;
- frontend;
- controlled integration.

Add regression coverage for reproducible defects when an active test layer can represent them reliably.

Do not weaken tests to make implementation pass.

Do not:

- skip failing tests without understanding them;
- weaken assertions;
- increase timeouts arbitrarily;
- add retries to hide instability;
- disable safety checks.

## Simulator

Use BMProv Simulator when behavior involves orchestration that does not require physical hardware.

Examples may include:

- Agent connect and reconnect;
- concurrent Jobs;
- scheduler contention;
- delayed messages;
- duplicate results;
- stale inventory;
- storage pressure;
- cancellation;
- failure and retry;
- endpoint disappearance.

Do not use simulation to claim physical hardware compatibility.

## Integration Environment

Physical BMProv hardware belongs to explicit Integration Environment validation.

Do not execute real destructive hardware operations unless the task includes explicit owner authorization for the exact environment and target.

When physical validation is required but not authorized or available:

- complete the safe local implementation and automated validation that is possible;
- report the exact remaining Integration Environment procedure;
- do not claim that validation was completed.

## Documentation

Update documentation only when the implementation changes durable information that belongs there.

Follow `docs/development/documentation-policy.md`.

Examples:

- accepted architectural change → relevant ADR and, after implementation, architecture documentation;
- reusable hardware fact → reference documentation;
- durable contract behavior → Specification when appropriate;
- public capability → README only when appropriate.

Do not duplicate the Work Package execution history into permanent documentation.

## Git and GitHub

Follow `AGENTS.md`.

Do not implicitly:

- stage changes;
- commit;
- create branches;
- merge;
- push;
- modify Issues;
- modify Project status;
- publish anything.

A request to implement does not automatically authorize Git or GitHub writes.

## Completion criteria

Implementation may be reported as ready for `Validation` when:

- approved scope is complete;
- required tests were added or updated;
- relevant automated validation was actually executed;
- required validation is not known to be failing;
- architecture and safety constraints remain satisfied;
- remaining manual or Integration Environment validation is explicit.

Do not claim the Work Package is `Done`.

Owner manual validation is the gate from `Validation` to `Done`.

## Output

Report:

- implemented scope;
- files changed;
- relevant design or implementation notes;
- automated tests added or updated;
- commands executed;
- actual validation results;
- environment limitations;
- Integration Environment validation still required;
- owner manual validation still required;
- relevant out-of-scope findings;
- one concise Conventional Commit suggestion.

Do not claim validation that was not actually performed.

Do not claim owner acceptance.

Do not modify Git or GitHub state unless separately and explicitly authorized.