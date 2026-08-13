---
name: update-documentation
description: Update Bamep documentation in the correct authoritative location while preserving implemented reality, accepted decisions, terminology, and source-of-truth boundaries.
argument-hint: "[documentation task, files, behavior, Specification, ADR, or reference finding]"
disable-model-invocation: true
---

# Update documentation

Update documentation for:

$ARGUMENTS

## Purpose

Use this skill for explicit Bamep documentation work.

It updates durable project knowledge in the source that owns it.

It must not invent product behavior, silently establish architecture, or create competing authoritative copies.

## Procedure

1. Read:
   - `AGENTS.md`;
   - `docs/development/documentation-policy.md`;
   - `docs/development/sdd.md`;
   - relevant Specifications, ADRs, architecture, Discovery, reference material, and persistent GitHub context.

2. Inspect the requested documentation and its closest authoritative evidence.

3. Use the `documentation` subagent for non-trivial documentation work.

4. Determine:
   - what information is changing;
   - what kind of information it is;
   - which source owns it;
   - whether another source already contains authoritative detail.

5. Verify technical claims against implementation, tests, configuration, contracts, or GitHub context when required.

6. Update only the documentation sources that actually own the changed information.

7. Prefer links to authoritative detail over copied explanations.

8. Preserve established Bamep terminology and historical decision context.

9. Validate the changed documentation.

10. Report conflicts, uncertainty, or missing evidence instead of silently resolving them.

## Source-of-truth handling

Follow the ownership model in `docs/development/documentation-policy.md`.

In particular:

- `README.md` remains public project documentation;
- Discovery preserves useful investigation and uncertainty;
- Specifications describe intended behavior;
- architecture documents implemented reality;
- ADRs preserve durable decisions and reasoning;
- reference material preserves validated reusable facts;
- development docs own engineering process;
- GitHub owns operational work and workflow state.

Do not duplicate authoritative content merely for convenience.

## Accuracy and decisions

Do not:

- document planned behavior as implemented;
- rewrite accepted ADR history to match current implementation;
- turn experimental evidence directly into a requirement;
- establish a new architectural decision through documentation;
- promote transient execution history into permanent docs without durable value;
- silently normalize conflicting terminology or sources.

If documentation work exposes an unresolved requirement or architectural choice, return it to the appropriate SDD/ADR process.

## Scope

This skill is for documentation work.

Do not silently expand into:

- product implementation;
- unrelated refactoring;
- dependency changes;
- architecture changes;
- unrelated tests;
- release work;
- GitHub workflow changes.

Report useful out-of-scope findings separately.

## Language

Follow the language rules in `AGENTS.md`.

Canonical Bamep engineering documentation remains in English.

## Git and GitHub

Follow `AGENTS.md`.

This skill does not implicitly authorize Git or GitHub writes.

Do not stage, commit, branch, merge, push, modify Issues or Project state, or publish unless separately and explicitly authorized.

GitHub may be inspected read-only when necessary to verify persistent context.

## Validation

After editing, verify as applicable:

- paths and filenames;
- links;
- commands and examples;
- terminology;
- implementation claims;
- ADR status;
- current versus planned behavior;
- source-of-truth ownership;
- duplicated or conflicting information.

Do not claim verification that was not performed.

Product test suites are normally unnecessary for documentation-only work unless executable or generated artifacts are affected.

## Output

Report:

- documentation changed;
- files changed;
- authoritative sources consulted;
- relevant ownership or terminology decisions;
- validation performed;
- unresolved inconsistencies or missing evidence;
- relevant out-of-scope findings;
- one suggested Conventional Commit message.

Do not claim owner approval or manual validation unless explicitly provided.