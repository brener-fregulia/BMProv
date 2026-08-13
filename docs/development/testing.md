# Testing

## Purpose

This document defines the Bamep testing and validation strategy.

It describes:

* testing principles;
* test layers;
* isolation requirements;
* simulator usage;
* Integration Environment boundaries;
* safety-sensitive validation;
* test selection;
* coverage policy;
* failure handling;
* result reporting.

Current commands and test tooling must be verified from the repository once they exist.

Related responsibilities:

* `AGENTS.md`: mandatory safety and validation rules;
* `docs/development/sdd.md`: validation as part of approved work;
* `docs/development/workflow.md`: operational execution and handoff to owner validation.

## Principles

* Test observable Bamep behavior, not dependency internals.
* Relevant automated tests are part of implementation completeness.
* Run the narrowest meaningful validation first.
* Expand validation according to scope and risk.
* Prefer deterministic and isolated tests.
* Use simulators and fakes at hardware, network, storage, and destructive boundaries.
* Add regression tests for reproducible defects.
* Test failure behavior as deliberately as success behavior.
* Never use destructive real-world operations when a safe automated representation is sufficient.
* Use the Integration Environment when behavior cannot be represented reliably in local automation.
* Treat coverage as a diagnostic signal, not proof of correctness.
* Owner manual validation remains the acceptance gate before `Done`.

## Test layers

Bamep should use the smallest layer capable of validating the intended behavior.

The expected strategy contains:

```text
Unit / Domain
      ↓
Contract
      ↓
Component / Integration
      ↓
Simulator
      ↓
Integration Environment
      ↓
Owner manual validation
```

Not every change requires every layer.

## Unit and domain tests

Use unit and domain tests for deterministic behavior that does not require external infrastructure.

Important candidates include:

* state transitions;
* validation rules;
* identity matching logic;
* inventory reconciliation;
* Job and JobStep behavior;
* retry and cancellation rules;
* idempotency;
* scheduler decisions;
* resource lease acquisition and release;
* storage capability selection;
* safety invariants;
* protocol message validation;
* artifact metadata and integrity logic.

State machines should test both valid and rejected transitions.

Safety-sensitive domain behavior should include negative cases that demonstrate prohibited operations remain prohibited.

## Contract tests

Use contract tests at boundaries shared between independently evolving components.

Relevant contracts may include:

* Administrative API;
* Agent Protocol;
* Extension Protocol;
* Server ↔ Web interactions;
* Server ↔ Agent messages;
* storage adapter interfaces;
* infrastructure adapter interfaces;
* artifact metadata;
* domain events.

Contract tests should verify externally relevant behavior such as:

* serialization;
* required and optional fields;
* version handling;
* validation;
* error representation;
* duplicate messages;
* unknown actions;
* incompatible requests.

Do not couple contract tests to private implementation details.

## Component and integration tests

Use component or integration tests when correctness depends on multiple real internal responsibilities working together.

Examples include:

* persistence and domain state;
* scheduler and resource leases;
* Agent session management;
* API and application services;
* artifact lifecycle;
* transfer metadata and integrity;
* adapter behavior against controlled local dependencies.

Prefer local disposable dependencies when practical.

Integration tests must remain deterministic enough to run repeatedly without requiring the physical Bamep laboratory.

## Simulator

Bamep Simulator is a first-class validation tool.

It should eventually support many concurrent simulated endpoints with configurable characteristics such as:

* connection and reconnection;
* latency;
* throughput;
* CPU constraints;
* storage characteristics;
* operation duration;
* failures;
* retries;
* interruptions;
* inventory changes;
* storage pressure;
* concurrent Jobs.

Use the Simulator for behaviors that require realistic orchestration without physical hardware.

Important scenarios include:

* 20–24 or more concurrent endpoints;
* reconnect during active work;
* duplicate or delayed messages;
* stale inventory;
* endpoint disappearance;
* Agent restart;
* Server restart where supported by the scenario;
* scheduler contention;
* resource exhaustion;
* partial failure;
* cancellation;
* recovery after interruption.

Simulator tests must remain reproducible. Randomized scenarios should use reproducible seeds when failures need to be diagnosed.

The Simulator does not replace physical integration testing for hardware-specific behavior.

## Fakes and test boundaries

Use fakes where the external system is not the behavior under test.

Appropriate fake boundaries may include:

* Agent connections;
* storage devices;
* network infrastructure;
* DHCP/PXE infrastructure;
* switch integrations;
* filesystem operations;
* process execution;
* clocks and timers;
* artifact stores;
* external download sources.

A fake must preserve the contract relevant to the test.

Do not create mocks that merely reproduce the internal implementation line by line.

When the external integration itself is being validated, use the appropriate integration layer instead of mocking it.

## Destructive-operation safety

Automated tests must not operate on real user or production data.

Tests involving disk or storage mutation must use appropriate safe targets such as:

* temporary files;
* virtual disk images;
* disposable loop devices when explicitly appropriate;
* temporary filesystems;
* isolated containers or virtual machines;
* controlled Integration Environment devices.

Destructive workflows should test, when applicable:

* identity validation;
* stale inventory rejection;
* target mismatch;
* missing authorization;
* invalid preconditions;
* interruption;
* safe retry behavior;
* non-replayable destructive steps;
* recovery-required states;
* verification before destructive execution.

A test must never weaken a safety invariant merely to reach later workflow stages.

## Data transfer and artifact tests

Backup and recovery behavior requires explicit failure testing.

Depending on the chosen implementation, relevant scenarios include:

* interrupted transfer;
* incomplete `.part` artifact;
* digest mismatch;
* size mismatch;
* duplicate transfer request;
* storage exhaustion;
* producer or consumer disconnect;
* restart behavior;
* atomic commit;
* verified artifact promotion;
* failed verification before destructive provisioning.

Resumability must only be tested according to capabilities actually supported by the selected transfer and artifact design.

Do not simulate resume semantics that the real producer cannot provide.

## Persistence and recovery tests

Durable workflow state must be tested independently from ephemeral runtime connections.

Relevant scenarios include:

* process restart;
* Agent reconnect;
* incomplete JobStep;
* persisted state with no active connection;
* duplicate result delivery;
* stale inventory revision;
* interrupted destructive workflow;
* recovery reconciliation;
* invalid state recovery.

Tests should demonstrate that restart or reconnect does not blindly replay destructive work.

## Frontend tests

Use frontend tests for behavior owned by Bamep Web.

Relevant areas include:

* components;
* state and stores;
* forms and validation;
* loading and error states;
* localization boundaries;
* API interaction through controlled boundaries;
* Job and endpoint presentation;
* destructive-action confirmations;
* accessibility-relevant behavior.

Prefer observable assertions over private component structure.

Frontend tests must not depend on a live production Bamep Server unless explicitly running as an integration scenario.

## Regression tests

A reproducible defect should receive a regression test when an active test layer can represent it reliably.

A regression test should:

1. reproduce the relevant failure condition;
2. assert the expected behavior;
3. avoid unrelated implementation details;
4. fail against the defective behavior when practical.

If no automated layer can represent the defect reliably, document why and define the required manual or Integration Environment validation.

Do not introduce an inappropriate test layer solely to cover one unrelated defect.

## Test isolation

Automated tests must not depend on:

* personal files;
* real endpoint data;
* production storage;
* mutable external infrastructure;
* developer-specific configuration;
* real credentials;
* public Internet availability;
* physical Bamep hardware unless the test is explicitly an Integration Environment test.

Use:

* temporary directories;
* deterministic fixtures;
* isolated databases;
* fake adapters;
* local test servers;
* disposable artifacts;
* explicit setup and teardown.

Created resources should be cleaned up after success and failure when practical.

## Integration Environment

The physical Bamep laboratory exists for behavior that cannot be validated faithfully through local tests or simulation.

Examples include:

* PXE;
* DHCP behavior;
* UEFI firmware;
* GRUB;
* Alpine boot;
* physical NIC behavior;
* MikroTik integration;
* real disk tooling;
* Windows deployment;
* WinPE;
* hardware-specific compatibility;
* destructive end-to-end provisioning.

Integration Environment tests must identify:

* required hardware or topology;
* preparation;
* exact target;
* safety precautions;
* expected result;
* cleanup or recovery procedure.

Destructive execution requires explicit owner authorization as defined in `AGENTS.md`.

Results that establish reusable hardware or compatibility facts should be persisted in `docs/reference/`.

## Selecting validation

Choose validation according to affected responsibility.

| Change                          | Minimum expected validation                                         |
| ------------------------------- | ------------------------------------------------------------------- |
| Pure domain rule                | Focused unit/domain tests                                           |
| State machine                   | Transition and rejection tests                                      |
| Shared protocol or API contract | Relevant contract tests                                             |
| Persistence behavior            | Domain + persistence integration                                    |
| Adapter                         | Adapter contract + controlled integration when relevant             |
| Scheduler/resource model        | Domain tests + concurrent simulator scenarios                       |
| Agent lifecycle                 | Contract + reconnect/failure scenarios                              |
| Transfer behavior               | Integration + interruption/integrity scenarios                      |
| Frontend behavior               | Relevant frontend tests                                             |
| Hardware-specific behavior      | Automated layers where useful + Integration Environment             |
| Destructive workflow            | Safety tests + explicit Integration Environment/manual validation   |
| Documentation only              | Verify terminology, links, paths, examples, and referenced behavior |

Shared or cross-cutting changes require broader validation than isolated changes.

## Coverage

Coverage is useful for discovering weakly tested:

* branches;
* state transitions;
* failure paths;
* safety logic;
* shared domain behavior;
* protocol handling.

Coverage percentage alone is not a quality target.

Do not introduce arbitrary thresholds during bootstrap.

Before enforcing thresholds:

1. establish stable test tooling;
2. establish a meaningful baseline;
3. verify which files and generated code are included;
4. inspect important uncovered behavior;
5. choose conservative values that prevent meaningful regression.

Do not lower an established threshold merely to make a change pass.

Safety-critical behavior may require strong targeted coverage even when repository-wide coverage remains lower.

## Handling failures

When validation fails:

1. reproduce the failure with the narrowest useful test or scenario;
2. determine whether the cause is the current change, environment, missing prerequisite, flaky behavior, or pre-existing state;
3. fix failures caused by the current work;
4. report unrelated or unresolved failures explicitly.

Do not:

* delete a failing test;
* skip it;
* weaken its assertion;
* increase timeouts;
* add retries;
* disable safety checks;

without understanding and documenting the underlying reason.

## Manual validation

Owner manual validation follows relevant automated validation.

It is especially important for:

* complete provisioning workflows;
* hardware behavior;
* destructive operations;
* operator-facing behavior that automation cannot represent reliably;
* real Windows deployment and recovery.

Before handing work to `Validation`, report:

* automated checks actually performed;
* actual results;
* environment limitations;
* exact manual scenarios remaining.

Agents must not claim owner validation has been completed.

A problem discovered during manual validation returns the affected work to `In Progress`.

## Reporting results

Report only validation that actually occurred.

Include, when relevant:

* scenarios covered;
* commands or test targets executed;
* actual pass/fail results;
* relevant coverage results;
* missing prerequisites;
* environment limitations;
* Integration Environment work performed;
* owner manual validation still required.

Do not describe intended tests as executed tests.

## Current bootstrap state

Bamep does not yet have an established production test stack or authoritative repository-wide test commands.

Do not invent them.

As Server, Web, Agent, Simulator, and supporting tooling are implemented, this document should record only stable testing responsibilities and policies.

Concrete commands should remain authoritative in the build, package, workspace, or test configuration that defines them.

## Guiding rule

Bamep testing should make unsafe, invalid, interrupted, and unexpected behavior as deliberate to validate as the successful path.

Simulation should cover what can be represented deterministically.

The Integration Environment should validate what depends on real hardware.

Owner manual validation should accept what automation cannot prove.
