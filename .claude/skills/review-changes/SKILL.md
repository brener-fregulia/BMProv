---
name: review-changes
description: Perform a read-only BMProv review of executed work against approved scope, Specifications, ADRs, architecture, safety invariants, tests, and validation evidence.
argument-hint: "[Work Package, branch, diff, files, or change set to review]"
disable-model-invocation: true
---

# Review changes

Review:

$ARGUMENTS

## Purpose

Use this skill to perform a focused technical review after execution and before owner acceptance.

This skill is read-only.

It evaluates whether the reviewed work:

- matches approved scope;
- preserves accepted architecture;
- respects safety invariants;
- has adequate validation;
- avoids unintended regressions or scope expansion.

It does not implement corrections.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/sdd.md`;
   - `docs/development/workflow.md`;
   - `docs/development/testing.md`;
   - `docs/development/documentation-policy.md`;
   - relevant Specifications;
   - relevant ADRs;
   - relevant architecture documentation.

2. Reconstruct approved scope from persistent repository and GitHub context.

3. Identify the actual change set.

4. Use the `review` subagent for non-trivial review work.

5. Inspect:
   - changed files;
   - nearby implementation;
   - relevant tests;
   - affected contracts;
   - relevant configuration;
   - validation evidence.

6. Compare the implementation against:
   - approved scope;
   - acceptance criteria;
   - safety invariants;
   - accepted ADRs;
   - implemented architecture;
   - testing expectations.

7. Identify actionable findings only.

8. Prioritize findings by impact.

9. Report validation gaps separately from implementation defects.

10. Report useful out-of-scope observations without turning them into required corrections unless they represent a concrete risk.

## Review priorities

Prioritize findings involving:

1. destructive-operation safety;
2. data loss;
3. identity and authentication;
4. authorization;
5. incorrect recovery behavior;
6. stale-state handling;
7. unsafe retry or replay;
8. protocol incompatibility;
9. persistence inconsistency;
10. concurrency and resource ownership;
11. regressions;
12. architectural violations;
13. missing error handling;
14. missing validation;
15. unintended scope expansion.

Avoid spending review attention on stylistic preferences unless they create a concrete maintenance or correctness problem.

## Safety review

For destructive or privileged work, inspect relevant behavior such as:

- endpoint identity validation;
- inventory revision checks;
- disk selection;
- destructive preconditions;
- authorization;
- artifact verification;
- retry semantics;
- duplicate execution;
- interruption;
- recovery-required states;
- privilege boundaries.

Do not accept a successful happy path as sufficient evidence when failure could destroy or corrupt data.

MAC address must not be treated as trusted permanent identity.

Unrestricted remote shell execution must not replace approved typed Agent actions.

## Architecture review

Verify that changes preserve accepted responsibilities and boundaries.

Look for:

- domain logic in infrastructure layers;
- infrastructure behavior leaking into domain rules;
- adapters being bypassed;
- unexpected cross-component coupling;
- new protocols introduced without an ADR;
- persistence policy emerging implicitly;
- duplicated responsibilities;
- speculative abstractions;
- dependencies introduced without a current requirement.

Do not compare BMProv architecture against FORGE or Pascoal unless the approved work explicitly uses them as evidence.

## State and persistence review

When relevant, inspect:

- Job and JobStep transitions;
- durable state before and after side effects;
- restart behavior;
- Agent reconnect;
- duplicate results;
- stale inventory;
- incomplete operations;
- reconciliation;
- cancellation;
- retry behavior.

Destructive work must not be replayed blindly after reconnect or restart.

## Scheduler review

When scheduling or resource management changes, inspect:

- lease acquisition;
- exclusivity;
- lease release;
- cancellation cleanup;
- failure cleanup;
- resource exhaustion;
- stale leases;
- concurrency;
- restart behavior;
- starvation where relevant.

Prefer evidence against the resource model rather than assumptions based on a fixed number of endpoints.

## Protocol review

When a protocol or API changes, inspect:

- serialization;
- field validation;
- version handling;
- unknown messages;
- duplicate messages;
- correlation;
- errors;
- idempotency;
- reconnect behavior;
- compatibility impact.

Do not assume independently deployable components evolve in lockstep.

## Transfer and artifact review

When relevant, inspect:

- temporary/incomplete artifacts;
- expected size;
- integrity digest;
- atomic commit;
- verification;
- storage exhaustion;
- interruption;
- duplicate transfers;
- retry behavior;
- resumability claims.

Do not accept resume behavior that the actual producer or artifact format cannot support.

## Testing review

Evaluate whether tests match the changed behavior and risk.

Look for missing:

- domain tests;
- invalid transition tests;
- safety-negative cases;
- contract tests;
- persistence/restart scenarios;
- duplicate-message scenarios;
- Simulator scenarios;
- interruption tests;
- frontend behavior tests when relevant.

Do not require broader testing merely for ceremony.

Coverage percentage alone is not evidence that behavior is sufficiently validated.

## Documentation review

When documentation changed, verify:

- correct ownership location;
- terminology;
- paths and links;
- current vs planned behavior;
- ADR status;
- architecture claims;
- duplicated sources of truth.

Do not require permanent documentation for transient implementation detail.

## Severity

Use:

- **Critical** — credible risk of destructive data loss, security compromise, or fundamentally unsafe behavior.
- **High** — major correctness, recovery, identity, protocol, or architectural failure.
- **Medium** — meaningful defect or validation gap likely to cause incorrect behavior.
- **Low** — limited-impact issue with a concrete reason to correct.

Do not inflate severity.

## Restrictions

This skill is read-only.

Do not:

- edit files;
- implement fixes;
- create tests;
- reformat code;
- modify Git state;
- modify GitHub state;
- create ADRs;
- publish anything.

A request to review does not authorize corrections.

If the user wants findings fixed, that is a separate execution step.

## Output

Start with findings ordered from highest to lowest severity.

For each finding include:

- severity;
- location;
- problem;
- impact;
- relevant requirement, invariant, ADR, or evidence;
- recommended correction direction when useful.

Then report:

- scope reviewed;
- validation evidence inspected;
- remaining validation gaps;
- unresolved questions;
- relevant out-of-scope observations;
- whether owner manual validation is still required.

If no actionable issue is identified, state:

`No actionable findings identified in the reviewed scope.`

Do not claim the work is `Done`.

Do not claim owner acceptance.