# Development Workflow

## Purpose

This document defines how approved BMProv work is executed while preserving owner control, traceability, safety, and handoff between sessions.

## Operational flow

```text
Approved specification
→ Work Package: Ready
→ branch from main
→ In Progress
  implementation + focused automated tests
→ automated validation
→ Validation
  owner manual checks
→ Done
```

## Before editing

1. identify the approved Specification or parent item;
2. identify the current Work Package;
3. inspect its scope and acceptance criteria;
4. inspect implemented architecture and relevant ADRs;
5. inspect existing implementation and tests;
6. confirm that no unresolved architectural decision blocks the work.

## Branches

`main` is the stable integration branch.

Planned work normally uses:

```text
feature/<name>
fix/<name>
refactor/<name>
docs/<name>
```

Work Packages normally share the branch of their parent item and do not create a branch by default.

## In Progress

- implement only the approved scope;
- add or update focused tests together with changed behavior;
- use fakes and the Simulator at appropriate boundaries;
- preserve unrelated work;
- do not introduce unnecessary cleanup, dependencies, translations, release work, or refactors;
- review the diff and validation failures before handoff.

## Validation

Before an item enters `Validation`, known relevant automated validation must pass.

Manual Validation is the owner's responsibility and is especially important for behavior involving PXE, boot firmware, real disks, Windows/WinPE, specific hardware, and destructive workflows.

## Done

`Done` means the owner manually accepted the work.

Record only outcome information useful to future work:

```text
Outcome
Automated validation
Manual validation
Related changes
```

Do not reproduce the conversation transcript or the complete code diff.

## Commits

Use concise Conventional Commits when authorized, for example:

```text
feat(agent): add enrollment handshake
fix(scheduler): release storage lease after cancellation
test(protocol): cover duplicate action acknowledgement
docs(architecture): record storage capability boundary
```

Detailed execution history belongs in the Work Package, not the commit message.
