# CLAUDE.md

@AGENTS.md

## Role

Claude Code is a primary tool for technical analysis, Discovery, specification,
implementation, testing, investigation, and review in BMProv.

Repository rules and engineering process are defined by `AGENTS.md` and
`docs/development/`.

## Context management

- Treat the current repository as authoritative over prior conversation context.
- Start with the task and the closest relevant files.
- Prefer targeted searches using symbols, component names, protocol fields, tests,
  configuration, or observable behavior.
- Follow dependencies only as far as needed to understand the task.
- Expand context when architecture, safety, dependencies, or failing validation
  require it.
- Avoid loading entire directories or large unrelated documents without a concrete
  reason.
- Reconstruct persistent context from repository and GitHub sources rather than
  relying on previous sessions.

For substantial planning or cross-cutting work, inspect relevant Discovery,
Specifications, architecture, ADRs, and GitHub context before proposing changes.

## Skills

Use project skills under `.claude/skills/` when their procedure matches the task.

- Follow the skill's scope, restrictions, validation requirements, and output
  expectations.
- Do not invent or claim to have executed a missing skill.
- Skills cannot override `AGENTS.md`.
- Do not use an implementation skill to bypass Discovery, Specification, owner
  approval, or a required Technical Spike.

## Subagents

Use specialized subagents when they provide relevant expertise or isolate focused
investigation.

Give each subagent:

- a bounded objective;
- relevant starting points;
- explicit scope restrictions;
- required evidence or validation;
- expected output.

The main agent remains responsible for:

- approved scope;
- coordination;
- resolving conflicting findings;
- validating the combined result;
- the final response.

Do not delegate the same work repeatedly without a concrete reason.

## Claude Code behavior

- Inspect the closest existing pattern before creating files or abstractions.
- Keep edits focused and preserve surrounding conventions.
- Verify repository commands before executing them.
- Do not install system dependencies or change global configuration without explicit
  permission.
- Do not infer BMProv architecture or technology choices from FORGE, Pascoal, or
  another project.
- Use `docs/development/sdd.md` for Discovery, Specification, Technical Spikes, and
  approval boundaries.
- Use `docs/development/workflow.md` for execution and operational state.
- Use `docs/development/testing.md` for validation strategy.
- Use `docs/development/documentation-policy.md` when deciding where durable
  information belongs.
- Implement one approved Work Package or reduced-SDD responsibility at a time.
- When execution reveals a new architectural decision, stop the affected choice and
  follow the ADR process instead of establishing policy through code.