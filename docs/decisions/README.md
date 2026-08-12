# Architectural Decision Records

ADRs preserve significant technical decisions so durable constraints are not repeatedly rediscovered or silently changed in future sessions.

## When to create an ADR

Create an ADR when a decision:

- establishes or changes a durable architectural boundary;
- has meaningful alternatives or non-trivial trade-offs;
- constrains future implementation choices;
- adopts or rejects a significant technology or strategy;
- is likely to be questioned again.

Do not create ADRs for routine naming, formatting, documentation language, release scope, or reversible implementation details.

## Naming

```text
0001-example-decision.md
0002-another-decision.md
```

Numbers are never reused.

## Status

- `Proposed`
- `Accepted`
- `Superseded`
- `Deprecated`
- `Rejected`

An `Accepted` decision should be reconsidered only when new requirements, constraints, or evidence justify doing so.

## Structure

```markdown
# ADR-NNNN: Decision title

Status: Proposed

## Context

## Decision

## Alternatives considered

## Consequences

## Related architecture

## Related work
```

## SDD relationship

If an architectural decision emerges during implementation:

1. record the question in the Work Package;
2. inspect existing ADRs;
3. stop only the affected architectural choice;
4. document the alternatives;
5. obtain owner approval;
6. create or update the ADR;
7. continue implementation.
