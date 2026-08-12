---
name: plan-feature
description: Run read-only BMProv Discovery and produce an evidence-based SDD proposal for a Feature, Fix, Refactor, Technical Spike, or larger objective before GitHub materialization.
argument-hint: "[idea, problem, task, issue, or objective]"
disable-model-invocation: true
---

# Plan feature

Analyze and specify:

$ARGUMENTS

## Purpose

Use this skill to turn an informal BMProv idea, problem, or objective into an
evidence-based proposal before implementation or GitHub materialization.

This skill performs Discovery and Specification only.

It does not approve, implement, or materialize work.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/sdd.md`;
   - `docs/development/documentation-policy.md`;
   - relevant architecture documentation;
   - relevant ADRs.

2. Inspect the closest relevant repository state:
   - implementation when it exists;
   - nearby tests;
   - configuration;
   - Specifications;
   - Discovery;
   - reference material.

3. Inspect relevant GitHub Issues, Milestones, or Project context when existing
   approved work may affect the request.

   Keep GitHub operations read-only.

4. Use the `specification` subagent for non-trivial work.

5. Establish:
   - requested outcome;
   - current behavior or current project state;
   - relevant evidence;
   - constraints;
   - explicit non-goals;
   - affected responsibilities;
   - safety implications;
   - architectural questions;
   - validation expectations.

6. Distinguish clearly:
   - implemented behavior;
   - validated evidence;
   - accepted decisions;
   - proposals;
   - assumptions;
   - unresolved questions.

7. Classify the work as one of:
   - Feature;
   - Fix;
   - Refactor;
   - Technical Spike;
   - Epic containing related Features, only when shared context justifies it.

8. Determine whether existing evidence is sufficient.

   If an important question requires empirical validation before the Specification
   or architecture can be completed, propose a focused Technical Spike.

9. Define only useful requirements:
   - Functional Requirements (`RF-###`);
   - Non-Functional Requirements (`RNF-###`);
   - Business Rules (`RN-###`);
   - safety invariants.

   Do not create empty requirement categories merely to satisfy a template.

10. Define observable acceptance criteria.

11. Identify:
    - architecture impact;
    - relevant existing ADRs;
    - architectural decisions that still require approval;
    - Technical Spikes that block or inform the work.

12. Decompose the proposal into Work Packages only when decomposition improves:
    - execution continuity;
    - review;
    - validation;
    - safety;
    - session handoff.

13. Define proportional validation expectations:
    - automated tests;
    - Simulator scenarios;
    - Integration Environment validation;
    - owner manual validation.

14. Present the result as a proposal for owner review.

## Safety-sensitive work

When the request affects destructive or privileged behavior, explicitly address
relevant safety constraints.

Examples include:

- endpoint identity;
- enrollment or authentication;
- disk selection;
- partitioning;
- formatting;
- deployment;
- backup;
- restore;
- artifact deletion;
- privileged Agent actions;
- destructive retry behavior.

Do not leave destructive-operation preconditions or recovery behavior implicit.

## Technical Spikes

Recommend a Technical Spike when evidence is insufficient and the question can be
answered empirically.

A proposed Spike should include:

- exact question;
- reason current evidence is insufficient;
- constraints;
- proposed experiment;
- evaluation criteria;
- expected evidence;
- decision or Specification it informs.

Do not use a Spike as a substitute for making a product decision that does not
depend on empirical evidence.

## Architecture

Do not treat planned architecture as current architecture.

`docs/architecture/` describes implemented reality only.

When a durable architectural choice with meaningful alternatives is required:

- identify the decision;
- inspect existing ADRs;
- describe relevant alternatives and trade-offs;
- identify missing evidence;
- mark the decision as requiring owner approval.

Do not establish architecture implicitly through the proposal.

## Work Packages

A proposed Work Package should be independently understandable and executable.

Include only when useful:

- objective;
- scope;
- out of scope;
- related requirements;
- architecture constraints;
- safety constraints;
- acceptance criteria;
- automated validation;
- Integration Environment or manual validation.

Do not create more Work Packages than necessary.

## Restrictions

This skill is read-only.

Do not:

- edit files;
- implement behavior;
- create branches;
- stage or commit changes;
- create or modify Issues;
- create Milestones;
- modify GitHub Project state;
- create ADR files;
- materialize Work Packages;
- publish anything.

Do not infer owner approval.

Do not invent requirements or repository behavior.

Do not reopen accepted ADRs without new requirements, evidence, or constraints.

## Output

Use only sections that add value.

Possible sections:

- Classification
- Context
- Current behavior
- Evidence
- Goal
- Scope
- Out of scope
- Functional Requirements
- Non-Functional Requirements
- Business Rules
- Safety invariants
- Acceptance Criteria
- Architecture impact
- Related ADRs
- Proposed Technical Spikes
- Proposed Work Packages
- Automated validation
- Simulator validation
- Integration Environment validation
- Owner manual validation
- Milestone or release impact
- Open questions

End with:

`Status: Proposed - awaiting owner approval.`

After approval, GitHub materialization is a separate explicit step.