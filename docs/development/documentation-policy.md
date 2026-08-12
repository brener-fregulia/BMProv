# Documentation Policy

## Purpose

This document defines where BMProv information belongs and when documentation should be created or updated.

The goals are to:

* keep one primary source for each kind of information;
* avoid duplicated maintenance;
* separate planned work from implemented reality;
* preserve durable technical knowledge;
* keep transient execution history out of permanent documentation;
* ensure relevant project context does not exist only in conversation history.

Related responsibilities:

* `AGENTS.md`: mandatory repository-wide rules;
* `docs/development/sdd.md`: Discovery, Specification, approval, and work decomposition;
* `docs/development/workflow.md`: execution workflow;
* `docs/development/testing.md`: testing and validation policy.

## Principles

* Every relevant piece of information should have one primary source.
* Link to authoritative detail instead of copying it.
* Document confirmed facts as facts.
* Preserve uncertainty when evidence is incomplete.
* Do not describe planned architecture as implemented architecture.
* Keep public documentation concise.
* Keep technical documentation maintainable.
* Preserve consistent terminology across documentation, code, APIs, protocols, and GitHub work.
* Do not leave required project context only in an AI session or conversation.
* Create permanent documentation only when the information remains useful beyond the work item that produced it.

## Sources of truth

| Source                   | Primary responsibility                                                                     |
| ------------------------ | ------------------------------------------------------------------------------------------ |
| Implementation and tests | Currently implemented behavior                                                             |
| `README.md`              | Public project overview                                                                    |
| `AGENTS.md`              | Mandatory repository-wide agent rules                                                      |
| `docs/discovery/`        | Investigation, evidence, uncertainty, and early technical analysis                         |
| `docs/specifications/`   | Persistent product or system specifications when repository-level persistence is justified |
| `docs/architecture/`     | Current implemented architecture and boundaries                                            |
| `docs/decisions/`        | Durable architectural decisions and reasoning                                              |
| `docs/development/`      | Engineering process and development policy                                                 |
| `docs/reference/`        | Validated technical facts, compatibility findings, and reusable external knowledge         |
| GitHub Issues            | Approved actionable work and Work Packages                                                 |
| GitHub Projects          | Operational workflow state                                                                 |
| GitHub Milestones        | Milestone or release scope                                                                 |

The repository is the permanent technical source of truth.

GitHub becomes the operational source of truth for approved and materialized work.

Conversation history is supplemental context only.

## README

`README.md` is public product documentation.

It should remain focused on information useful to someone discovering BMProv, such as:

* project purpose;
* project status;
* major capabilities when implemented;
* supported environments;
* installation or usage when available;
* basic development entry points when stable;
* license;
* links to relevant public documentation.

Do not use the README as:

* agent instructions;
* backlog;
* detailed roadmap;
* architecture specification;
* ADR index;
* Discovery log;
* implementation history;
* Work Package tracker.

Update it when public project positioning, supported behavior, requirements, or stable entry points change.

## Discovery

Use `docs/discovery/` for investigations whose evidence or reasoning is useful beyond one conversation.

Discovery may contain:

* problem analysis;
* current-state investigation;
* constraints;
* validated PoC lessons;
* alternative approaches;
* unresolved questions;
* evidence gathered during architecture exploration;
* results that may later produce Specifications, ADRs, or Technical Spikes.

Discovery is allowed to describe proposed possibilities and uncertainty.

It must clearly distinguish:

* known facts;
* observed evidence;
* assumptions;
* proposals;
* unresolved questions.

Discovery is not implemented architecture.

Once an important conclusion receives a more authoritative permanent home, link to that source instead of maintaining competing copies.

## Specifications

Specifications define intended approved behavior.

Use `docs/specifications/` when a Specification has long-term system value beyond one operational GitHub Issue.

Examples may include:

* core domain contracts;
* protocol-level behavior;
* safety-critical lifecycle definitions;
* cross-cutting system behavior expected to remain stable;
* milestone-level specifications useful as persistent design context.

Routine Feature, Fix, Refactor, or Work Package specifications may live only in GitHub after approval when repository duplication would provide no additional value.

Do not maintain identical Specifications in both GitHub and repository files.

When both are necessary:

* the repository document owns durable system-level specification;
* the GitHub Issue owns actionable scope, status, and execution context;
* each should link to the other.

## Architecture

`docs/architecture/` describes what BMProv currently implements.

It may document:

* runtime components;
* module responsibilities;
* boundaries;
* data flows;
* deployment topology;
* persistence boundaries;
* control and data planes;
* adapter boundaries;
* implemented security boundaries.

Architecture documents must reflect repository reality.

Do not write planned architecture into `docs/architecture/` merely because it has been proposed or approved.

Before implementation, planned structure belongs in:

* Discovery;
* Specifications;
* ADRs.

After implementation, update architecture documentation when the resulting structure is important enough to remain useful.

## Architectural Decision Records

Use `docs/decisions/` for durable architectural decisions with meaningful alternatives.

Create an ADR when a decision:

* establishes or changes an important boundary;
* constrains future implementation;
* selects between meaningful architectural alternatives;
* affects security, persistence, protocols, deployment, or system topology;
* is costly to reverse;
* is likely to be questioned again.

Do not create ADRs for:

* routine implementation details;
* obvious naming choices;
* owner-defined project conventions with no meaningful architectural alternative;
* small reversible refactors;
* temporary experimental choices.

ADR format and lifecycle belong in `docs/decisions/README.md`.

Accepted ADRs are historical records.

When a decision changes, supersede or amend it according to the ADR policy rather than rewriting history as if the previous decision never existed.

## Reference documentation

Use `docs/reference/` for reusable factual knowledge.

Examples include:

* hardware compatibility;
* firmware behavior;
* network equipment findings;
* validated PXE quirks;
* tool behavior;
* storage characteristics;
* external protocol constraints;
* licensing or distribution constraints;
* experimentally confirmed system limitations.

Reference documentation should distinguish:

* observed environment;
* tested versions;
* validated behavior;
* known limitations.

A reference fact is not automatically an architectural requirement.

Architecture and Specifications may depend on reference findings, but should link to them rather than copy their full evidence.

## Technical Spikes

Technical Spike execution belongs to the relevant GitHub work item or focused Discovery material while active.

Preserve permanent results according to what the Spike discovered:

```text
empirical fact
→ docs/reference/

architectural decision
→ docs/decisions/

approved intended behavior
→ Specification

implemented structure
→ docs/architecture/
```

Do not maintain permanent Spike reports when all useful conclusions have been transferred to their authoritative sources.

Experimental code is not documentation of accepted architecture.

## GitHub Issues

Use GitHub Issues for approved actionable work.

Issues may represent:

* Epics;
* Features;
* Fixes;
* Refactors;
* Technical Spikes;
* Work Packages.

Issues should contain enough approved context to execute and validate the work without depending on conversation history.

They may include:

* scope;
* out of scope;
* acceptance criteria;
* relevant requirements;
* safety constraints;
* related ADRs;
* validation expectations;
* execution outcome;
* unresolved blockers.

Do not copy durable architecture or ADR reasoning into Issues when an authoritative repository document already exists.

Link instead.

## GitHub Projects and Milestones

GitHub Projects stores operational workflow state:

```text
Backlog
Ready
In Progress
Validation
Done
```

It is not technical documentation.

GitHub Milestones group milestone or release scope.

Do not recreate Project or Milestone state in repository documents.

## Documentation during implementation

Update documentation only when the work changes information that should remain useful afterward.

Typical mapping:

| Change                                         | Typical permanent source                       |
| ---------------------------------------------- | ---------------------------------------------- |
| Internal implementation with no durable effect | No permanent documentation required            |
| Approved actionable work                       | GitHub Issue                                   |
| New durable requirement                        | Specification                                  |
| New architectural decision                     | ADR                                            |
| Implemented architecture change                | `docs/architecture/`                           |
| Hardware or compatibility finding              | `docs/reference/`                              |
| Development process change                     | `docs/development/`                            |
| Public capability or requirement               | `README.md` when appropriate                   |
| Experimental investigation                     | Spike/Discovery until conclusions are promoted |

Do not update every documentation category for every change.

## Avoiding duplication

Before adding information, ask:

1. Does this information already have a primary source?
2. Is this document the correct owner?
3. Can this document link to the authoritative source instead?
4. Will this information remain useful after the current task?
5. Am I copying execution history into permanent documentation?

Prefer:

```text
short context
+ link to authority
```

over duplicated explanations.

Do not maintain the same requirement independently in:

* Discovery;
* Specification;
* Issue;
* ADR;
* architecture documentation;
* reference documentation.

Each source should preserve only the responsibility it owns.

## Language

Canonical repository documentation is written in English.

This includes:

* Discovery;
* Specifications;
* architecture;
* ADRs;
* development documentation;
* reference documentation;
* GitHub Issues and Work Packages.

BMProv user-facing localization is independent from repository documentation language.

Academic or TCC material may be written separately in Brazilian Portuguese.

Academic material must not become a competing authoritative copy of engineering documentation.

## Validation

For documentation changes, verify as applicable:

* referenced paths exist;
* commands reflect actual repository configuration;
* terminology is consistent;
* links are valid;
* claims match implementation or cited evidence;
* planned behavior is not presented as implemented;
* ADR status is respected;
* GitHub operational state is not unnecessarily duplicated;
* required context is not left only in conversation history.

Documentation-only changes normally do not require product test suites unless they modify generated documentation, executable examples, configuration, schemas, or other testable artifacts.

## Guiding rule

Store information where its future reader will expect its authority to live.

Discovery preserves evidence.

Specifications preserve intended behavior.

ADRs preserve why durable choices were made.

Architecture preserves implemented structure.

Reference documentation preserves reusable facts.

GitHub preserves approved actionable work and operational state.

The README presents the product.

If the same information appears fully in several of those places, the documentation model is probably wrong.
