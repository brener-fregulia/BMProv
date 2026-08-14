---
name: materialize-work
description: Materialize approved Bamep work into GitHub Issues, Milestones, Project items, and relationships without changing the approved scope.
argument-hint: "[approved Specification, proposal, milestone, or work item]"
disable-model-invocation: true
---

# Materialize work

Materialize:

$ARGUMENTS

## Purpose

Use this skill to turn already-approved Bamep work into persistent GitHub operational
state.

This skill performs GitHub materialization only.

It does not perform Discovery, approve proposals, create architectural decisions, or
implement product behavior.

Materialization must preserve the approved scope rather than redesign it.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/sdd.md`;
   - `docs/development/workflow.md`;
   - relevant approved Specifications, Discovery conclusions, ADRs, and other
     persistent sources referenced by the work.

2. Reconstruct the approved materialization scope, including when applicable:
   - classification;
   - Issue decomposition;
   - Work Packages;
   - Technical Spikes;
   - Milestone;
   - labels;
   - hierarchy or sub-issue relationships;
   - dependencies;
   - intended initial Project status.

3. Verify that owner approval exists for the work being materialized.

4. Inspect current GitHub state read-only before proposing writes:
   - existing Issues;
   - Milestones;
   - labels;
   - Project configuration and fields;
   - existing Project items;
   - relevant parent/sub-issue relationships.

5. Detect existing or partially materialized work and avoid duplicates.

6. Prepare the exact GitHub change set required to represent the approved work.

7. Verify explicit authorization for the required GitHub writes.

8. Perform only the authorized materialization.

9. Re-read the resulting GitHub state and verify that created or modified items match
   the approved materialization.

10. Report the resulting operational state and any work that could not be
    materialized safely.

## Approval boundary

Materialize only approved work.

Do not infer approval because:

- Discovery was completed;
- a Specification exists;
- a proposal was discussed;
- a milestone exists;
- the work appears necessary;
- this skill was invoked.

The skill invocation authorizes this workflow, but does not by itself authorize
restricted GitHub modifications.

Explicit and specific authorization may be provided in the same user request that
invokes this skill. Do not require a redundant second confirmation when that request
already unambiguously authorizes the exact GitHub materialization.

If approval or write authorization is missing, stop before modification and report
the exact proposed materialization.

## Materialization rules

GitHub operational state must represent approved repository context.

Do not:

- invent new requirements;
- silently change approved scope;
- redesign an approved decomposition;
- create speculative implementation tasks;
- create one Issue per possible ADR merely because an ADR candidate exists;
- create unnecessary Epic or Feature hierarchy;
- invent labels, priorities, due dates, assignees, or Project fields;
- duplicate information already owned by an ADR or repository document;
- modify unrelated existing Issues or Project items.

Use the smallest hierarchy that preserves execution context and traceability.

An ADR remains authoritative under `docs/decisions/`.

A GitHub Issue may track work that creates or updates an ADR, but the Issue must not
become a competing architectural source of truth.

## Issue content

Each materialized Issue must contain enough persistent context for its responsibility
without depending on conversation history.

For a Work Package, include only useful sections such as:

- Objective
- Scope
- Out of scope
- Related requirements
- Relevant architecture or decisions
- Dependencies
- Safety constraints
- Acceptance criteria
- Automated validation
- Manual or Integration Environment validation

For a Technical Spike, include as applicable:

- Question
- Why existing evidence is insufficient
- Constraints and assumptions
- Investigation method
- Evaluation or success criteria
- Expected evidence
- Dependencies
- Required environment
- Expected durable output

Do not copy an entire Specification into every child Issue.

Reference the authoritative source where duplication would add no execution value.

## Classification and relationships

Use the repository's established classifications and labels.

Do not create a new taxonomy merely because an existing label is imperfect.

Use Milestones for milestone grouping when approved.

Use parent/sub-issue relationships only when the approved decomposition benefits from
them.

Do not create hierarchy merely for visual organization.

## Project status

Newly materialized work should normally enter `Backlog`.

Creation alone does not make work `Ready`.

Use `Ready` only when the persistent work item already satisfies the repository's
definition of executable work and the approved materialization explicitly supports
that state.

Do not move work to:

- `In Progress` unless execution has actually begun;
- `Validation` unless execution and required automated validation are complete;
- `Done` without owner manual acceptance.

Preserve existing Project status when materializing an already-existing item unless
the authorized task explicitly includes a status transition.

## Safety-sensitive work

When materializing work involving identity, enrollment, destructive operations,
storage mutation, backup, restore, provisioning, retry, cancellation, or privileged
execution, preserve the approved safety requirements explicitly enough for execution.

Do not invent missing safety policy.

If required safety context is absent, report the gap and do not materialize the
affected execution unit as `Ready`.

## Git and GitHub

Follow `AGENTS.md`.

This skill may modify GitHub only when explicitly and specifically authorized for the
current materialization.

Authorization must be limited to the identified change set.

Do not:

- modify repository files;
- stage or commit;
- create or switch branches;
- push or pull;
- create releases;
- change unrelated repository settings;
- modify unrelated GitHub work.

Before executing GitHub writes, verify identifiers and current state rather than
assuming Issue numbers, Milestone numbers, Project IDs, field IDs, option IDs, or
repository names.

Prefer supported `gh` commands or GitHub APIs appropriate to the current environment.

## Partial failure

GitHub materialization may involve multiple writes.

If one operation fails:

1. stop dependent writes when continuing could create inconsistent state;
2. inspect the state that actually exists;
3. do not blindly repeat successful operations;
4. identify what was created, what failed, and what remains;
5. propose the smallest safe recovery action.

Never report atomic success when only part of the materialization completed.

## Output

Before writes, when authorization is still required, report:

- approved source being materialized;
- exact Issues or other items to create or modify;
- labels;
- Milestone;
- Project;
- initial statuses;
- relationships;
- any ambiguity or blocker requiring owner resolution.

After authorized materialization, report:

- Issues created or reused, with numbers and titles;
- Milestone assignment;
- Project membership;
- resulting statuses;
- relationships created;
- existing items intentionally reused;
- skipped or blocked items;
- failures or partial state;
- remaining work before any item can enter `Ready`.

Do not claim implementation, validation, or owner acceptance merely because work was
materialized.