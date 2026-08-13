---
name: add-tests
description: Add or improve focused Bamep automated tests for approved behavior without expanding product scope or weakening validation rules.
argument-hint: "[behavior, Work Package, files, defect, or test gap]"
disable-model-invocation: true
---

# Add tests

Add or improve tests for:

$ARGUMENTS

## Purpose

Use this skill to add focused automated validation for existing or approved Bamep behavior.

This skill may add missing tests, strengthen weak coverage, add regressions, improve isolation, and make small behavior-preserving changes required for testability.

It must not invent product behavior or expand architecture merely to make testing easier.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/testing.md`;
   - `docs/development/workflow.md`;
   - `docs/development/sdd.md`;
   - relevant Specifications, ADRs, architecture, implementation, and existing tests.

2. Inspect:
   - the behavior under test;
   - nearby tests and helpers;
   - relevant contracts and configuration;
   - existing validation evidence.

3. Use the `testing` subagent for non-trivial testing work.

4. Establish:
   - the intended behavior;
   - the risk being validated;
   - the narrowest meaningful test layer;
   - relevant regression conditions;
   - remaining Integration Environment or owner-manual validation.

5. Add only tests that represent actual approved or implemented behavior.

6. Prefer deterministic and isolated validation.

7. Make only small behavior-preserving production changes required for a reliable test boundary.

8. Run the narrowest relevant tests first.

9. Broaden validation only when changed scope or risk justifies it according to `docs/development/testing.md`.

10. Report actual results and remaining gaps.

## Test design

Follow the test-selection and safety guidance defined by the `testing` subagent and `docs/development/testing.md`.

When relevant, prioritize:

- valid and rejected state transitions;
- safety-negative behavior;
- protocol and contract compatibility;
- persistence and recovery;
- duplicate, stale, retry, replay, and cancellation behavior;
- transfer and artifact integrity;
- concurrency and resource ownership;
- focused regression cases.

Do not add every possible test layer by default.

Simulation may validate orchestration behavior, but must not be used as evidence of physical hardware compatibility.

## Testability and scope

Small production changes are acceptable only when they preserve behavior and create a necessary test boundary.

Do not silently introduce:

- new product behavior;
- broad refactoring;
- speculative abstractions;
- architecture changes;
- unrelated fixes;
- dependency or release changes.

If meaningful architecture change is required for testability, return it to the SDD/ADR process.

## Failure handling

Do not hide failing validation.

When a test fails, identify whether the cause is the test, current production behavior, environment, prerequisites, flaky behavior, or pre-existing repository state.

Do not:

- weaken assertions without evidence;
- skip failures merely to pass;
- add blind retries;
- increase timeouts merely to mask instability;
- disable safety checks.

Report unrelated failures separately.

## Coverage

Coverage is diagnostic evidence, not the goal of this skill.

Do not add superficial tests only to increase a percentage or change coverage thresholds without explicit project policy.

## Git and GitHub

Follow `AGENTS.md`.

This skill does not implicitly authorize Git or GitHub writes.

Do not stage, commit, branch, merge, push, modify Issues or Project state, or publish unless separately and explicitly authorized.

## Output

Report:

- behavior covered;
- tests added or changed;
- production changes made only for testability, if any;
- test layer used;
- commands executed;
- actual validation results;
- coverage results when collected;
- environment limitations;
- remaining Integration Environment validation;
- remaining owner manual validation;
- relevant out-of-scope findings;
- one suggested Conventional Commit message.

Do not claim validation that was not executed.

Do not claim the work is `Done` or owner-accepted.