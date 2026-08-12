---
name: implement-feature
description: Implement one approved BMProv Work Package or reduced-SDD task with focused validation and strict scope control.
argument-hint: "[approved Work Package or reduced-SDD task]"
disable-model-invocation: true
---

# Implement feature

Implement:

$ARGUMENTS

## Purpose

Use this skill to execute one approved BMProv Work Package or one valid reduced-SDD task.

This skill assumes the relevant planning and approval already exist.

It must not invent scope, bypass unresolved architecture, or absorb unrelated work.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/sdd.md`;
   - `docs/development/workflow.md`;
   - `docs/development/testing.md`;
   - `docs/development/documentation-policy.md`;
   - relevant Specifications, ADRs, architecture, and persistent GitHub context.

2. Reconstruct the approved execution scope.

3. Inspect:
   - affected implementation;
   - nearby tests;
   - relevant contracts and configuration;
   - established local patterns.

4. Verify before editing that:
   - the task is approved and executable;
   - blocking architectural decisions are resolved or isolated;
   - required Technical Spikes are complete;
   - relevant safety constraints are explicit.

5. Implement only the approved responsibility.

6. Add or update focused automated tests required by the changed behavior.

7. Run the narrowest meaningful validation first and broaden it according to risk and `docs/development/testing.md`.

8. Review the resulting changes for:
   - correctness;
   - scope expansion;
   - safety violations;
   - architectural violations;
   - regressions;
   - missing failure handling;
   - missing validation.

9. Update durable documentation only when the completed work changes information owned by that documentation source.

10. Report the work as ready for owner validation only when required automated validation is not known to be failing.

## Scope and decisions

Do not silently add:

- unrelated refactoring or cleanup;
- speculative abstractions;
- dependency or release changes;
- future features;
- unrelated documentation;
- architectural changes.

If execution reveals incomplete scope, unresolved requirements, or a new durable architectural decision, stop the affected work and return it to the appropriate SDD, ADR, or Technical Spike step.

Do not establish architecture implicitly through implementation.

## Safety

Follow the safety rules in `AGENTS.md` and the approved work.

Do not invent missing destructive-operation policy during implementation.

Use safe local boundaries such as Simulator, fakes, fixtures, temporary storage, and controlled integrations where appropriate.

Physical or destructive Integration Environment work requires the explicit authorization defined by repository policy.

Never use unrestricted remote shell execution as a shortcut around approved typed Agent actions.

## Validation

Follow `docs/development/testing.md`.

Do not:

- weaken tests or safety checks;
- hide failures;
- add blind retries;
- increase timeouts merely to mask instability;
- claim validation that was not executed.

When physical validation remains necessary, complete the safe local work that is possible and report the remaining Integration Environment procedure.

## Git and GitHub

Follow `AGENTS.md`.

This skill does not implicitly authorize Git or GitHub writes.

Do not stage, commit, branch, merge, push, publish, or modify Issues or Project state unless separately and explicitly authorized.

## Completion

The work may be reported as ready for `Validation` when:

- approved scope is complete;
- relevant automated tests exist;
- required automated validation was executed;
- required validation is not known to be failing;
- remaining Integration Environment or owner-manual validation is explicit.

Do not claim the work is `Done`.

Owner manual validation remains the acceptance gate.

## Output

Report:

- implemented scope;
- files changed;
- tests added or updated;
- commands executed;
- actual validation results;
- environment limitations;
- remaining Integration Environment validation;
- remaining owner manual validation;
- relevant out-of-scope findings;
- one suggested Conventional Commit message.

Do not claim owner acceptance.