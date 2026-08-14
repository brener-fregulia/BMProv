# CLAUDE.md

@AGENTS.md

## Role

Claude Code is a primary tool for technical analysis, Discovery, specification,
implementation, testing, investigation, and review in Bamep.

Repository rules and engineering process are defined by `AGENTS.md` and
`docs/development/`.

## Interaction language

Use Brazilian Portuguese (`pt-BR`) when communicating with the repository owner by
default.

- Prefer natural Brazilian Portuguese for explanations, questions, summaries, and
  workflow output.
- Use another language only when the owner requests it or when preserving source text
  exactly is important.
- Keep repository content, source code, identifiers, documentation, GitHub Issues,
  ADRs, Specifications, and other persistent project artifacts in English as defined
  by `AGENTS.md`.
- Do not translate commands, paths, identifiers, API fields, or tool output merely for
  conversational consistency.

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

Project skills under `.claude/skills/` are user-invoked workflow commands.

When the user invokes a project skill:

- follow the skill's scope, procedure, restrictions, validation requirements, and output expectations;
- treat the invocation as authorization to run that workflow, not as authorization for otherwise restricted Git, GitHub, publication, infrastructure, or destructive operations;
- do not invent or claim to have executed a missing skill;
- do not use a skill to bypass Discovery, Specification, owner approval, or a required Technical Spike.

Project skills use `disable-model-invocation: true` intentionally.

Claude must not assume that a project skill can be loaded or invoked automatically.

Skills cannot override `AGENTS.md`.

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
- Do not infer Bamep architecture or technology choices from FORGE, Pascoal, or
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