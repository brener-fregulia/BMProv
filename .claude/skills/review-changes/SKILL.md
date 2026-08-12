---
name: review-changes
description: Perform a read-only BMProv review of executed work against approved scope, architecture, safety, and validation evidence.
argument-hint: "[Work Package, branch, diff, files, or change set to review]"
disable-model-invocation: true
---

# Review changes

Review:

$ARGUMENTS

## Purpose

Use this skill to perform a focused technical review after execution and before owner acceptance.

This skill is read-only.

It identifies actionable defects, risks, validation gaps, architectural violations, and unintended scope expansion.

It does not implement corrections.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/sdd.md`;
   - `docs/development/workflow.md`;
   - `docs/development/testing.md`;
   - `docs/development/documentation-policy.md`;
   - relevant Specifications, ADRs, architecture, and persistent GitHub context.

2. Reconstruct the approved scope and acceptance criteria.

3. Identify the actual change set.

4. Use the `review` subagent for non-trivial review work.

5. Inspect:
   - changed files;
   - affected implementation;
   - nearby tests;
   - relevant contracts and configuration;
   - actual validation evidence.

6. Compare the work against:
   - approved scope;
   - Specifications;
   - accepted ADRs;
   - implemented architecture;
   - safety invariants;
   - validation expectations.

7. Report actionable findings only, ordered by impact.

8. Report validation gaps separately from implementation defects.

9. Report useful out-of-scope observations separately unless they represent a concrete defect or risk in the reviewed work.

## Review focus

Follow the review priorities and severity model defined by the `review` subagent.

Pay particular attention, when relevant, to:

- destructive-operation safety and data loss;
- identity, authentication, authorization, and privilege;
- stale state, retry, replay, cancellation, and recovery;
- persistence and concurrency;
- shared protocol and contract compatibility;
- artifact integrity and transfer behavior;
- architectural boundaries;
- regressions and missing error handling;
- missing or misleading validation;
- unintended scope expansion.

Do not spend review effort on personal style preferences or speculative future requirements.

## Evidence

Review against persistent project evidence.

Do not:

- infer requirements from implementation alone when approved scope exists;
- assume tests passed merely because test code exists;
- accept simulation as physical compatibility evidence;
- treat coverage percentage alone as proof of correctness;
- compare BMProv against FORGE or Pascoal unless explicitly relevant to the approved work.

If authoritative sources conflict, report the conflict.

## Restrictions

This skill is read-only.

Do not:

- edit files;
- implement fixes;
- create or modify tests;
- reformat code;
- create ADRs;
- modify Git or GitHub state;
- publish anything.

A request to review does not authorize corrections.

Corrections require a separate execution step.

## Output

Start with findings ordered from highest to lowest severity.

For each finding include, when useful:

- severity;
- location;
- problem;
- impact;
- relevant requirement, invariant, ADR, or evidence;
- correction direction.

If no actionable issue is identified, state:

`No actionable findings identified in the reviewed scope.`

Then report, when relevant:

- scope reviewed;
- validation evidence inspected;
- remaining validation gaps;
- unresolved questions;
- relevant out-of-scope observations;
- remaining owner manual validation.

Do not claim the work is `Done`.

Do not claim owner acceptance.