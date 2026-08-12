---
name: run-technical-spike
description: Execute one approved BMProv Technical Spike to gather focused empirical evidence without turning experimental work into production architecture implicitly.
argument-hint: "[approved Technical Spike question or Issue]"
disable-model-invocation: true
---

# Run technical spike

Investigate:

$ARGUMENTS

## Purpose

Use this skill to execute one approved BMProv Technical Spike.

A Technical Spike exists to gather evidence for a Specification or architectural decision when repository knowledge alone is insufficient.

This skill must not:

- silently select an architecture;
- expand into production implementation;
- turn experimental code into permanent product code;
- bypass owner approval for architectural decisions.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/sdd.md`;
   - `docs/development/testing.md`;
   - `docs/development/documentation-policy.md`;
   - relevant Specifications;
   - relevant ADRs;
   - relevant Discovery;
   - relevant reference material.

2. Reconstruct the approved Spike from persistent repository or GitHub context.

3. Confirm the exact question being investigated.

4. Inspect existing evidence before designing or running any experiment.

5. Use the `technical-spike` subagent for non-trivial investigation.

6. Establish:
   - why existing evidence is insufficient;
   - known constraints;
   - assumptions;
   - relevant accepted decisions;
   - expected decision impact;
   - safety boundaries.

7. Define the smallest experiment capable of answering the question.

8. Prefer:
   - simulation;
   - virtual machines;
   - local disposable environments;
   - disk images;
   - temporary storage;
   - controlled test services;

   before physical or destructive environments.

9. Execute only the approved experiment.

10. Record:
    - environment;
    - relevant versions;
    - configuration;
    - inputs;
    - procedure;
    - observations;
    - failures;
    - raw measurements when relevant.

11. Repeat the experiment when repeatability matters.

12. Distinguish:
    - observed facts;
    - interpretation;
    - limitations;
    - recommendation.

13. Determine whether the evidence is:
    - conclusive;
    - inconclusive;
    - blocked.

14. Identify which Specification, ADR, or future work is affected.

15. Persist only reusable conclusions in the appropriate authoritative location when the current task explicitly includes that documentation work.

## Scope control

Investigate one focused question.

Do not expand the Spike into:

- general architecture design;
- unrelated benchmarking;
- production implementation;
- broad refactoring;
- dependency modernization;
- speculative optimization;
- cleanup unrelated to the experiment.

If the original question is too broad to test meaningfully, stop and propose decomposition instead of improvising several unrelated experiments.

## Existing evidence

Before running an experiment, search for evidence already available in:

- `docs/discovery/`;
- `docs/reference/`;
- accepted ADRs;
- existing Specifications;
- tests;
- implementation;
- previous approved Spike results.

Do not rerun an experiment solely because the current session did not know its result.

Repeat prior experiments only when:

- environment changed;
- versions changed;
- assumptions changed;
- prior evidence is insufficient;
- reproduction is itself required.

## Experiment design

Define when applicable:

- question;
- independent variable;
- controlled conditions;
- environment;
- hardware or simulated hardware;
- software and firmware versions;
- inputs;
- procedure;
- evaluation criteria;
- expected observations;
- repeat count;
- safety precautions;
- cleanup.

Keep the experiment narrow enough to attribute results meaningfully.

## Safety

Experiments must follow `AGENTS.md`.

Prefer safe representations for destructive behavior.

Do not perform real destructive operations against physical targets without explicit owner authorization for the exact environment and target.

When working with network boot or infrastructure:

- avoid interfering with unrelated DHCP or PXE environments;
- isolate test networks when necessary;
- do not seize control of existing corporate network services;
- record topology when it materially affects behavior.

When working with storage:

- verify the exact test target;
- prefer virtual or disposable devices;
- never use personal or production data.

When safe execution is not possible, stop and report the required Integration Environment setup.

## Physical Integration Environment

Use physical hardware only when the Spike depends on behavior that simulation cannot represent faithfully.

Examples include:

- PXE firmware;
- UEFI behavior;
- Secure Boot;
- NIC firmware or driver behavior;
- switch behavior;
- real disk tooling;
- Windows or WinPE boot behavior.

For physical experiments, define before execution:

- topology;
- target hardware;
- exact device or endpoint;
- versions;
- preparation;
- safety precautions;
- expected result;
- recovery procedure.

If the required authorization is absent, do not execute the destructive portion.

## Performance Spikes

For performance or capacity experiments, record relevant context such as:

- endpoint count;
- workload;
- dataset size;
- CPU;
- memory;
- storage medium;
- network capacity;
- concurrency;
- latency;
- duration;
- cache state when relevant.

Do not generalize laboratory measurements into unsupported production guarantees.

Prefer statements such as:

`Observed X under environment Y`

over:

`BMProv supports X`

unless the latter is explicitly justified by a broader accepted requirement and validation strategy.

## Experimental code

Experimental code is disposable by default.

Keep it clearly separated from production implementation when practical.

Do not production-harden experimental code unless the approved Spike specifically requires evaluating production constraints.

A successful experiment does not authorize merging that implementation into the product.

If the approach should become production behavior:

1. preserve the evidence;
2. update or complete the relevant Specification;
3. complete any required ADR;
4. obtain owner approval;
5. implement it through separately approved work.

## Negative and inconclusive results

Preserve failures when they answer the question.

A failed approach may be more valuable than a successful prototype when it rules out an architectural alternative.

Do not hide:

- intermittent failure;
- unsupported behavior;
- version-specific limitations;
- unresolved causes;
- environmental dependency.

Do not force a conclusion when evidence remains insufficient.

## Relationship to ADRs

A Spike provides evidence.

An ADR records an accepted durable decision.

After the Spike:

- identify architectural implications;
- describe alternatives affected by the evidence;
- recommend the next decision step.

Do not mark an architectural decision accepted unless owner approval occurred separately.

## Relationship to Specifications

Report how the evidence affects intended behavior, such as:

- feasibility;
- constraints;
- safety requirements;
- performance assumptions;
- acceptance criteria;
- implementation boundaries.

Do not invent new product requirements solely from the experiment.

## Documentation promotion

Follow `docs/development/documentation-policy.md`.

Typical destinations are:

```text
validated reusable fact
→ docs/reference/

durable architectural decision
→ docs/decisions/

durable intended behavior
→ docs/specifications/

implemented structure
→ docs/architecture/
```

Do not preserve duplicate Spike reports after useful conclusions have an authoritative home unless the raw experiment itself remains valuable evidence.

## Git and GitHub

Follow `AGENTS.md`.

This skill does not implicitly authorize:

- branch creation;
- staging;
- commits;
- pushes;
- Issue updates;
- Project status changes;
- ADR creation;
- publication.

Experimental file changes may only be made when the approved task explicitly authorizes them.

## Output

Report:

- Question
- Existing evidence
- Assumptions
- Constraints
- Experiment
- Environment
- Procedure
- Observations
- Measurements when relevant
- Failures or negative results
- Limitations
- Conclusion
- Specification impact
- Architecture / ADR impact
- Recommended next step
- Remaining uncertainty

Clearly separate facts from interpretation.

End with exactly one:

`Status: Conclusive - evidence is sufficient for the stated question.`

`Status: Inconclusive - additional evidence is required.`

`Status: Blocked - the investigation cannot proceed safely or reliably with the available environment.`

Do not present the result as owner approval of an architectural decision.
