---
name: update-documentation
description: Update BMProv documentation in the correct authoritative location while preserving implemented reality, accepted decisions, terminology, and source-of-truth boundaries.
argument-hint: "[documentation task, files, behavior, Specification, ADR, or reference finding]"
disable-model-invocation: true
---

# Update documentation

Update documentation for:

$ARGUMENTS

## Purpose

Use this skill for explicit BMProv documentation work.

This skill may:

- update public project documentation;
- update Discovery;
- update durable Specifications;
- update implemented architecture documentation;
- create or update ADR-related documentation when already approved;
- promote validated Technical Spike findings into reference material;
- update development process documentation;
- correct inaccurate or duplicated documentation.

It must not use documentation work to invent product behavior, silently establish architecture, or duplicate authoritative sources.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/documentation-policy.md`;
   - `docs/development/sdd.md`;
   - relevant Specifications;
   - relevant ADRs;
   - relevant architecture documentation;
   - relevant reference material.

2. Inspect the documentation being changed and its closest authoritative sources.

3. Use the `documentation` subagent for non-trivial documentation work.

4. Determine:
   - what information is being documented;
   - whether it is current behavior, intended behavior, a durable decision, validated evidence, or operational work;
   - which source should own it;
   - whether another source already owns the same information.

5. Inspect implementation, tests, configuration, or GitHub context when required to verify technical claims.

6. Update only the appropriate documentation source.

7. Prefer links to authoritative detail instead of copying content.

8. Preserve established terminology.

9. Verify that:
   - planned behavior is not presented as implemented;
   - historical ADR reasoning is not rewritten;
   - GitHub operational state is not duplicated into repository documentation;
   - transient execution details are not promoted unnecessarily.

10. Validate paths, links, terminology, and technical claims that were changed.

## Source selection

Use the documentation model defined in `docs/development/documentation-policy.md`.

Typical ownership:

```text
README.md
→ public product overview

docs/discovery/
→ investigation, evidence, alternatives, and uncertainty

docs/specifications/
→ durable intended system behavior when repository persistence is justified

docs/architecture/
→ currently implemented architecture

docs/decisions/
→ durable architectural decisions and reasoning

docs/development/
→ engineering process and policy

docs/reference/
→ validated reusable technical facts

GitHub Issues
→ approved actionable work and execution context

GitHub Projects
→ workflow state

GitHub Milestones
→ milestone or release scope
```

Do not create competing authoritative copies.

## README

Use `README.md` only for public product information.

Appropriate content may include:

- project purpose;
- project status;
- implemented major capabilities;
- supported environments;
- stable installation or usage entry points;
- stable development entry points;
- license.

Do not add:

- agent rules;
- detailed SDD;
- architecture plans;
- backlog;
- Work Package state;
- implementation history;
- detailed ADR reasoning.

Do not advertise unimplemented capabilities as current behavior.

## Discovery

Use `docs/discovery/` when investigation or evidence remains useful before or across decisions.

Discovery may contain:

- alternatives;
- assumptions;
- uncertainty;
- observed evidence;
- unresolved questions;
- early technical analysis.

Clearly distinguish facts from proposals and uncertainty.

Do not rewrite Discovery as if every explored alternative became accepted.

## Specifications

Use `docs/specifications/` for durable intended behavior when repository-level persistence is justified.

When updating a Specification:

- preserve approved scope;
- keep requirements observable;
- preserve relevant safety invariants;
- distinguish approved requirements from proposals;
- link to ADRs instead of duplicating their reasoning;
- do not copy operational Work Package history into the Specification.

Routine actionable work may remain in GitHub when repository persistence adds no durable value.

## Architecture

`docs/architecture/` documents implemented reality only.

Before updating architecture documentation:

1. inspect the actual implementation;
2. inspect relevant configuration and tests when useful;
3. inspect accepted ADRs;
4. verify the documented boundary exists.

Do not document an approved but unimplemented design as current architecture.

Planned architecture belongs in Specifications, Discovery, or ADRs until implemented.

## ADR documentation

ADRs preserve durable decisions and why they were made.

When working with ADRs:

- preserve accepted historical reasoning;
- preserve decision status;
- do not rewrite older decisions merely to match current implementation;
- use the project's ADR lifecycle for changes or supersession;
- keep the decision focused on meaningful alternatives and trade-offs.

This skill does not authorize a new architectural decision.

If a documentation task reveals an unresolved architectural choice, stop that part and return it to the SDD/ADR approval process.

## Technical Spike findings

Promote Spike results according to what the evidence represents:

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

Do not copy the full experiment into every destination.

Preserve raw Spike material only when it remains useful evidence.

## Reference documentation

Use `docs/reference/` for validated reusable facts such as:

- hardware compatibility;
- firmware behavior;
- PXE quirks;
- tool behavior;
- storage characteristics;
- external technical constraints;
- experimentally confirmed limitations.

Record relevant versions, environment, or topology when they materially affect the finding.

Do not convert an empirical observation into an architectural requirement without an accepted decision.

## Development documentation

Use `docs/development/` for stable engineering process.

Examples include:

- SDD;
- workflow;
- testing policy;
- documentation policy;
- later release or contribution procedures when they exist.

Avoid duplicating detailed procedures already owned by another development document.

## Terminology

Use existing BMProv terminology consistently.

Do not create alternate names for established concepts merely for style.

Verify terminology against relevant:

- Specifications;
- ADRs;
- architecture documentation;
- protocols;
- schemas;
- implementation.

If terminology conflicts exist, report them instead of silently normalizing one source.

## Language

Canonical repository documentation is written in English.

Preserve commands, identifiers, protocol fields, code, and product names exactly where required.

The UI localization strategy is separate from repository documentation language.

Academic or TCC material in Brazilian Portuguese must not become a competing engineering source of truth.

## Duplication control

Before adding or repeating information, ask:

1. Which source owns this information?
2. Does the information already exist there?
3. Is a link sufficient?
4. Will this information remain useful beyond the current task?
5. Am I copying execution history instead of documenting durable knowledge?

Prefer:

```text
short local context
+ link to authoritative source
```

over duplicated sections.

## Evidence and accuracy

Do not document technical claims from memory when they can be verified.

When relevant, inspect:

- repository paths;
- implementation;
- tests;
- configuration;
- commands;
- schemas;
- protocol fields;
- supported environments;
- ADR status;
- GitHub context.

If evidence is insufficient, state the uncertainty or recommend Discovery or a Technical Spike.

Do not silently fill gaps.

## Scope control

This skill is for documentation work.

Do not silently:

- implement product behavior;
- refactor code;
- add dependencies;
- change architecture;
- create tests unrelated to documentation validation;
- change release state;
- modify GitHub workflow state.

Small non-documentation issues discovered during the task should be reported separately.

## Git and GitHub

Follow `AGENTS.md`.

This skill does not implicitly authorize:

- staging;
- commits;
- branch creation;
- merges;
- pushes;
- Issue modification;
- Project state changes;
- publication.

GitHub may be inspected read-only when necessary to verify approved work or operational context.

## Validation

After documentation changes, verify as applicable:

- Markdown structure;
- internal links;
- referenced paths;
- filenames;
- commands;
- terminology;
- implementation claims;
- ADR status;
- current vs planned behavior;
- duplicated authority;
- referenced versions or environment facts.

Do not claim verification that was not performed.

Product test suites are normally unnecessary for documentation-only changes unless the documentation task modifies executable examples, schemas, generated documentation, configuration, or other testable artifacts.

## Output

Report:

- documentation changed;
- files changed;
- authoritative sources consulted;
- ownership decisions made;
- terminology decisions made;
- validation performed;
- unresolved inconsistencies;
- missing evidence;
- relevant out-of-scope findings;
- one suggested Conventional Commit message.

Do not claim owner approval or manual validation unless explicitly provided.
