---
name: run-technical-spike
description: Execute one approved Bamep Technical Spike to gather focused empirical evidence without turning experimental work into production architecture implicitly.
argument-hint: "[approved Technical Spike question or Issue]"
disable-model-invocation: true
---

# Run technical spike

Investigate:

$ARGUMENTS

## Purpose

Use this skill to execute one approved Bamep Technical Spike.

A Technical Spike gathers evidence for a Specification or architectural decision.

It must not silently select architecture, expand into production implementation, or bypass owner approval.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/sdd.md`;
   - `docs/development/testing.md`;
   - `docs/development/documentation-policy.md`;
   - relevant Specifications, ADRs, Discovery, reference material, and persistent GitHub context.

2. Reconstruct the approved Spike and preserve its exact question.

3. Inspect existing evidence before designing or repeating an experiment.

4. Use the `technical-spike` subagent for non-trivial investigation.

5. Establish:
   - why current evidence is insufficient;
   - relevant assumptions and constraints;
   - accepted decisions;
   - safety boundaries;
   - what decision or Specification the result informs.

6. Design and execute the smallest safe experiment capable of answering the question.

7. Prefer simulation, virtualized or disposable environments, temporary storage, and controlled integrations before physical or destructive testing.

8. Record enough environment, configuration, versions, procedure, observations, failures, and measurements to interpret and reproduce the result where practical.

9. Repeat the experiment when reliability or variability materially affects the question.

10. Separate:
    - observed facts;
    - interpretation;
    - limitations;
    - recommendations.

11. Determine whether the result is conclusive, inconclusive, or blocked.

12. Identify the impact on relevant Specifications, ADR candidates, or future work.

13. Persist reusable conclusions only when documentation changes are explicitly included in the approved task.

## Scope

Investigate only the approved question.

Do not silently expand into:

- general architecture design;
- production implementation;
- unrelated benchmarking;
- broad refactoring;
- dependency modernization;
- speculative optimization;
- unrelated cleanup.

If the question cannot be tested meaningfully as scoped, report the problem rather than improvising unrelated experiments.

## Evidence and safety

Follow the evidence and safety rules defined by the `technical-spike` subagent, `AGENTS.md`, and `docs/development/testing.md`.

Do not:

- rerun validated experiments without a concrete reason;
- generalize limited laboratory evidence into unsupported product guarantees;
- hide negative, intermittent, or inconclusive results;
- weaken safety controls to simplify an experiment;
- perform destructive physical operations without explicit authorization for the exact environment and target.

Use the Integration Environment only when the question depends on physical behavior that safer environments cannot represent faithfully.

Experimental code remains experimental unless separately approved as production work.

## Decisions

A successful Spike does not accept an architectural decision.

If the evidence supports a durable architectural choice, return that choice to the appropriate Specification and ADR process.

Do not invent new product requirements from experimental results.

## Git and GitHub

Follow `AGENTS.md`.

This skill does not implicitly authorize Git or GitHub writes.

Experimental file changes, repository writes, ADR creation, Issue updates, commits, pushes, or publication require separate explicit authorization unless already part of the approved Spike scope.

## Output

Use the report structure defined by the `technical-spike` subagent.

Clearly separate facts from interpretation.

End with exactly one:

`Status: Conclusive - evidence is sufficient for the stated question.`

`Status: Inconclusive - additional evidence is required.`

`Status: Blocked - the investigation cannot proceed safely or reliably with the available environment.`

Do not present the result as owner approval of an architectural decision.