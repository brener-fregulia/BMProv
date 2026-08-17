# M0 — Architecture Baseline & Simulated Provisioning Contract

Status: **Architecture and contract baseline complete.** All M0 internal architectural decisions are resolved: ADR-0001 through ADR-0011 are `Accepted`, and every M0 Specification required to satisfy the "Acceptance criteria" below is `Approved`. This describes completion of the M0 architecture/contract phase only — it does not claim that implementation exists, that the first vertical slice has been empirically validated, or that physical Integration Environment work is complete; those remain the explicitly separate post-M0 phases described below and in `docs/development/testing.md`. Ready for owner final validation and milestone closure.

## Goal

Transform Discovery into an owner-approved architectural and contract baseline before the first implementation Work Package.

## Scope

M0 must resolve or explicitly isolate:

- product boundary and domain vocabulary;
- Endpoint identity;
- Job/JobStep lifecycle;
- scheduler and resource model;
- Agent action model;
- control-plane contract;
- data-plane contract;
- persistence strategy;
- Backend/Agent stack;
- security and trust model;
- storage capabilities;
- Simulator contract;
- observability and domain-event model;
- packaging and versioning baseline;
- testing policy.

## Out of scope

- production provisioning implementation;
- real disk formatting;
- real Windows installation;
- WinPE implementation;
- MikroTik-specific production adapter;
- final production backup format;
- ERP;
- licensing enforcement;
- multi-site management;
- HA;
- Tauri.

## Required technical spikes

Questions requiring empirical evidence must become explicit spikes, especially:

- WinPE mechanism;
- resumable volume/image transfer format;
- Secure Boot or hardened boot chain when required;
- driver-provider integrations.

## Acceptance criteria

M0 is complete when:

1. product boundary, vocabulary, and non-goals are persisted;
2. blocking ADRs are `Accepted` or the unresolved question is isolated in an explicit technical spike;
3. destructive operations have specified safety invariants;
4. the simulated vertical slice has defined behavior, contracts, and failure scenarios;
5. component responsibilities and boundaries are clear;
6. relevant requirements have a validation strategy;
7. no required architectural decision is hidden inside a future implementation Work Package;
8. the owner explicitly approves the baseline.

## First implementation slice after M0

The first implementation vertical slice must work without real hardware:

```text
Simulated endpoint connects
→ authenticated/enrolled
→ inventory reported
→ job created
→ scheduler evaluates resources
→ typed action dispatched
→ simulated transfer executed
→ progress/events persisted
→ disconnect/reconnect handled
→ job reaches terminal state
→ Web reflects result
```

The slice must support a scenario with 20–24 concurrent simulated endpoints.
