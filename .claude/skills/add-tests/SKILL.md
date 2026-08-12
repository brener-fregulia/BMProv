---
name: add-tests
description: Add or improve focused BMProv automated tests for approved behavior without expanding product scope or weakening safety and validation rules.
argument-hint: "[behavior, Work Package, files, defect, or test gap]"
disable-model-invocation: true
---

# Add tests

Add or improve tests for:

$ARGUMENTS

## Purpose

Use this skill to add focused automated validation for existing or approved BMProv behavior.

This skill may:

- add missing tests;
- strengthen weak tests;
- add regression coverage;
- improve test isolation;
- make small behavior-preserving changes required for testability.

It must not invent product requirements, expand implementation scope, or introduce architecture solely to make testing easier.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/testing.md`;
   - `docs/development/workflow.md`;
   - `docs/development/sdd.md`;
   - relevant Specifications;
   - relevant ADRs;
   - relevant architecture documentation.

2. Inspect:
   - the behavior under test;
   - nearby tests;
   - existing fixtures and helpers;
   - affected contracts;
   - relevant configuration.

3. Use the `testing` subagent for non-trivial testing work.

4. Determine:
   - the intended behavior;
   - the risk being validated;
   - the narrowest appropriate test layer;
   - whether a regression condition exists;
   - whether physical Integration Environment validation is still required.

5. Add only tests that represent actual specified or implemented behavior.

6. Prefer deterministic and isolated validation.

7. Make only small production changes required to create an appropriate test boundary.

8. Run the narrowest relevant tests first.

9. Broaden validation when shared behavior, contracts, persistence, concurrency, or safety justify it.

10. Report actual results and remaining gaps.

## Test layers

Choose the smallest layer that can represent the behavior reliably.

Possible layers include:

- unit/domain tests;
- state-machine tests;
- protocol or API contract tests;
- persistence integration tests;
- adapter contract tests;
- Simulator scenarios;
- frontend tests;
- controlled integration tests.

Do not add every layer by default.

Use broader layers only when they validate behavior that cannot be represented meaningfully in a narrower one.

## Domain and state-machine tests

Use domain tests for deterministic behavior such as:

- Job and JobStep transitions;
- scheduler decisions;
- resource leases;
- endpoint identity matching;
- inventory reconciliation;
- retry and cancellation;
- idempotency;
- artifact lifecycle;
- storage capability selection;
- safety invariants.

Test both allowed and rejected behavior.

For state machines, explicitly cover invalid transitions when they are part of the contract.

## Contract tests

Use contract tests for externally meaningful boundaries such as:

- Administrative API;
- Agent Protocol;
- Extension Protocol;
- Server and Web interactions;
- Server and Agent messages;
- adapters;
- domain events;
- artifact metadata.

Relevant cases may include:

- serialization;
- required fields;
- optional fields;
- validation;
- unknown values;
- duplicate messages;
- incompatible versions;
- error representation;
- idempotency.

Do not couple tests to private implementation details when the contract is the behavior under test.

## Safety-sensitive tests

Prioritize negative tests for destructive or privileged behavior.

Relevant scenarios may include:

- stale inventory;
- endpoint mismatch;
- wrong disk target;
- missing authorization;
- invalid preconditions;
- failed artifact verification;
- duplicate destructive request;
- unsafe retry;
- interrupted operation;
- recovery-required state;
- replay after reconnect or restart.

A safety test must demonstrate that prohibited behavior remains prohibited.

Never weaken a safety invariant to make a test scenario reachable.

## Regression tests

For a reproducible defect:

1. reproduce the exact relevant failure condition;
2. assert expected behavior;
3. avoid unrelated implementation details;
4. ensure the test would fail against the defective behavior when practical.

Do not add broad test suites when one focused regression test is sufficient.

If the defect cannot be represented reliably in an automated layer, document the required Integration Environment or owner-manual validation instead.

## Simulator tests

Use BMProv Simulator when behavior depends on orchestration across multiple endpoints without requiring physical hardware.

Relevant scenarios may include:

- connect and reconnect;
- duplicate messages;
- delayed messages;
- stale inventory;
- concurrent Jobs;
- scheduler contention;
- resource exhaustion;
- cancellation;
- retries;
- partial failure;
- endpoint disappearance;
- Server or Agent restart where supported.

Keep scenarios reproducible.

Use deterministic seeds when randomized behavior is useful.

Do not use simulation to claim hardware compatibility.

## Test isolation

Automated tests must not depend on:

- personal files;
- production data;
- production storage;
- mutable external infrastructure;
- public Internet access;
- real credentials;
- developer-specific state;
- physical endpoints unless explicitly running as Integration Environment validation.

Prefer:

- temporary directories;
- isolated databases;
- deterministic fixtures;
- fake adapters;
- local test servers;
- virtual disk images;
- disposable artifacts;
- controlled clocks and timers.

Clean up created resources when practical.

## Production changes for testability

A small behavior-preserving production change is acceptable when required to create a reliable test boundary.

Examples include:

- extracting deterministic logic;
- separating parsing from process execution;
- introducing dependency injection at an existing external boundary;
- introducing an adapter where an external dependency already exists;
- making time controllable through an appropriate abstraction.

Do not perform substantial redesign under the label of testability.

If meaningful architecture change is required, stop and return it to the SDD/ADR process.

## Existing tests

Preserve useful existing coverage.

Do not:

- delete tests merely because they fail after a change;
- weaken assertions without understanding why;
- duplicate an existing scenario without additional value;
- rewrite unrelated tests;
- replace integration behavior with mocks when the integration itself is under test.

Follow established local testing patterns when they are appropriate.

## Failure handling

When validation fails:

1. reproduce the failure narrowly;
2. determine whether the cause is:
   - the current test;
   - production behavior;
   - environment;
   - missing prerequisite;
   - flaky behavior;
   - pre-existing repository state;
3. correct work within approved scope;
4. report unrelated failures separately.

Do not:

- skip failures;
- increase timeouts arbitrarily;
- add blind retries;
- disable warnings;
- disable safety checks.

## Coverage

Coverage may help identify weak paths, but percentage is not the goal of this skill.

Prioritize meaningful validation of:

- branches;
- state transitions;
- failure paths;
- safety rules;
- protocol handling;
- shared domain behavior.

Do not add superficial tests only to increase a metric.

Do not introduce or modify coverage thresholds unless explicitly requested and supported by project policy.

## Scope control

This skill adds validation, not new product behavior.

Do not silently add:

- new features;
- unrelated bug fixes;
- broad refactors;
- new dependencies without justification;
- architecture changes;
- release work;
- GitHub state changes.

Report out-of-scope findings separately.

## Git and GitHub

Follow `AGENTS.md`.

This skill does not implicitly authorize:

- staging;
- commits;
- branch creation;
- merges;
- pushes;
- Issue updates;
- Project state changes;
- publication.

## Completion criteria

Testing work is complete when:

- the relevant behavior is represented by appropriate tests;
- tests are isolated and deterministic enough for their layer;
- relevant safety-negative cases are covered when applicable;
- the narrowest required validation was actually executed;
- broader validation was run when justified;
- remaining Integration Environment or owner-manual validation is explicit.

Do not claim validation that was not executed.

## Output

Report:

- behavior covered;
- tests added or changed;
- production files changed for testability, if any;
- test layer used;
- commands executed;
- actual results;
- coverage results when collected;
- environment limitations;
- remaining Integration Environment validation;
- remaining owner manual validation;
- out-of-scope findings;
- one suggested Conventional Commit message.

Do not claim the work is `Done`.

Do not claim owner acceptance.
