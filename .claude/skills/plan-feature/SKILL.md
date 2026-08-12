---
name: plan-feature
description: Run read-only BMProv Discovery and produce an evidence-based SDD proposal before GitHub materialization.
argument-hint: "[idea, problem, task, issue, or objective]"
disable-model-invocation: true
---

# Plan feature

Analyze and specify:

$ARGUMENTS

## Purpose

Use this skill to turn an informal BMProv idea, problem, task, or objective into an evidence-based proposal before implementation or GitHub materialization.

This skill performs Discovery and Specification only.

It does not approve, implement, or materialize work.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/sdd.md`;
   - `docs/development/documentation-policy.md`;
   - relevant Specifications, ADRs, architecture, Discovery, and reference material.

2. Inspect the closest relevant repository evidence, including implementation, tests, configuration, and contracts when they exist.

3. Inspect relevant GitHub context read-only when existing approved or related work may affect scope.

4. Use the `specification` subagent for non-trivial planning.

5. Establish enough evidence to define:
   - current state;
   - goal;
   - scope and non-goals;
   - relevant requirements and safety constraints;
   - architectural impact;
   - unresolved questions;
   - validation expectations.

6. Separate empirical uncertainty into focused Technical Spikes when required.

7. Decompose work into Work Packages only when that improves execution, review, validation, safety, or continuity.

8. Present the result as a proposal for owner review according to `docs/development/sdd.md`.

## Evidence and scope

Do not:

- invent requirements or repository behavior;
- treat assumptions as facts;
- present planned architecture as implemented architecture;
- reopen accepted ADRs without new evidence, requirements, or constraints;
- use a Technical Spike for a decision that does not require empirical evidence;
- over-decompose work merely to create more Issues.

When evidence is insufficient, preserve the uncertainty explicitly.

## Safety

For safety-sensitive work, ensure the proposal exposes the relevant invariants, preconditions, failure behavior, recovery expectations, and required validation defined by repository policy.

Do not invent missing destructive-operation policy during planning.

## Restrictions

This skill is read-only.

Do not:

- edit repository files;
- implement behavior;
- create ADR files;
- materialize Work Packages;
- modify Git or GitHub state;
- publish anything.

Do not infer owner approval.

## Output

Use the Specification structure defined by `docs/development/sdd.md`.

Use only sections that add value.

Clearly distinguish:

- implemented behavior;
- validated evidence;
- accepted decisions;
- proposals;
- assumptions;
- unresolved questions.

End with:

`Status: Proposed - awaiting owner approval.`

After approval, GitHub materialization is a separate explicit step.