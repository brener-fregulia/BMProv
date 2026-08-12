# Specification-Driven Development

## Purpose

BMProv uses Specification-Driven Development (SDD) to preserve requirements, decisions, scope, and validation across disposable AI sessions.

## Sources of truth

Repository:

- implementation and tests;
- `AGENTS.md`;
- `docs/architecture/`;
- `docs/decisions/`;
- `docs/development/`;
- `docs/reference/`.

GitHub, after approved work is materialized:

- Milestones: milestones or releases when applicable;
- Issues: materialized Specifications, ADR work, technical spikes, and Work Packages;
- Projects: workflow state and progress.

## Lifecycle

```text
Idea
→ Discovery
→ Specification
→ Owner approval
→ GitHub materialization
→ Work Packages
→ Implementation + automated tests
→ Automated validation
→ Owner manual Validation
→ Done
```

## Discovery

Discovery is analysis, not implementation.

It should identify the requested outcome, evidence, constraints, non-goals, risks, required decisions, validation expectations, and whether decomposition is necessary.

Do not invent requirements to fill gaps.

## Specification

Use only sections that add useful information:

- Context;
- Goal;
- Scope / Out of scope;
- Functional Requirements (`RF-###`), Non-Functional Requirements (`RNF-###`), and Business Rules (`RN-###`) when they improve precision;
- Acceptance Criteria;
- Architecture impact;
- Related ADRs;
- Work Package decomposition.

Before owner approval, a Specification is a proposal.

## Work Package

A Work Package is the smallest planned execution unit that another session can implement and validate without depending on the conversation that created it.

Recommended structure:

```text
Objective
Scope / Out of scope
Related requirements
Relevant architecture / ADRs
Acceptance criteria
Implementation notes
Automated validation
Manual validation
Outcome
```

## Status model

- **Backlog** — identified, but not yet ready for execution;
- **Ready** — approved context is sufficient for another session to begin;
- **In Progress** — implementation, tests, and automated validation are active;
- **Validation** — relevant automated validation is complete; owner manual validation is pending;
- **Done** — the owner accepted the work.

An automated or manual validation failure returns the item to `In Progress`.

## Architectural decisions

Agents must not establish architectural policy silently through code.

If a durable decision with meaningful alternatives emerges, follow the ADR process before choosing the affected approach.

## Session handoff

Unfinished work must persist its current status, completed and remaining work, known validation results, and blockers in the repository or GitHub.

The next session reconstructs context from those sources rather than relying on conversation history.

## Guiding rule

Use enough process to preserve decisions, safety, scope, validation, and continuity. Do not create hierarchy or documentation without a real purpose.
