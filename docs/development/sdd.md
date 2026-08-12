# Specification-Driven Development

## Purpose

BMProv usa SDD para preservar requirements, decisions, scope e validation entre sessões descartáveis de IA.

## Sources of truth

Repository:

- implementation e tests;
- `AGENTS.md`;
- `docs/architecture/`;
- `docs/decisions/`;
- `docs/development/`;
- `docs/reference/`.

GitHub, após materialização:

- Milestones: marcos/releases quando aplicável;
- Issues: Specifications, ADR work, spikes e Work Packages materializados;
- Projects: workflow state e progress.

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

Discovery é análise, não implementação.

Deve identificar outcome, evidence, constraints, non-goals, riscos, decisões necessárias, validation expectations e necessidade de decomposição.
Não invente requisitos para preencher lacunas.

## Specification

Use somente seções úteis:

- Contexto;
- Objetivo;
- Escopo / Fora de escopo;
- RF / RNF / RN quando agregarem precisão;
- Acceptance Criteria;
- Architecture impact;
- Related ADRs;
- Work Package decomposition.

Antes da aprovação do owner, é proposta.

## Work Package

É a menor unidade de execução planejada que outra sessão consegue implementar e validar sem depender da conversa que a criou.

Estrutura recomendada:

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

## Status

- **Backlog** — identificado, ainda não pronto para execução;
- **Ready** — contexto aprovado é suficiente para outra sessão começar;
- **In Progress** — implementação, testes e validação automatizada em andamento;
- **Validation** — automação relevante concluída; validação manual do owner pendente;
- **Done** — owner aceitou o trabalho.

Falha automática ou manual retorna o item a In Progress.

## Architectural decisions

Agentes não estabelecem policy arquitetural silenciosamente por código.
Se surgir decisão durável com alternativas relevantes, siga o processo de ADR antes de escolher.

## Session handoff

Trabalho incompleto deve persistir status, realizado/restante, validação conhecida e blockers no repositório/GitHub.
A próxima sessão reconstrói o contexto a partir dessas fontes.

## Guiding rule

Use processo suficiente para preservar decisões, segurança, escopo, validação e continuidade. Não crie hierarquia ou documentação sem função real.
