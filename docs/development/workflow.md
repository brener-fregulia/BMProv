# Development Workflow

## Purpose

Define como trabalho BMProv aprovado é executado mantendo controle do owner, rastreabilidade, segurança e handoff entre sessões.

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

1. identifique Specification/parent item aprovado;
2. identifique o Work Package atual;
3. confira scope e acceptance criteria;
4. confira arquitetura implementada e ADRs relevantes;
5. confira implementação e testes existentes;
6. confirme que não há decisão arquitetural bloqueadora ainda aberta.

## Branches

`main` é a stable integration branch.

Planejamento normalmente usa:

```text
feature/<name>
fix/<name>
refactor/<name>
docs/<name>
```

Work Packages normalmente compartilham a branch do parent item e não criam branch por padrão.

## In Progress

- implemente apenas o escopo aprovado;
- acrescente ou atualize testes focados junto do comportamento;
- use fakes/simulator nas boundaries apropriadas;
- preserve unrelated work;
- não introduza cleanup, dependencies, translations, release work ou refactors não necessários;
- reveja diff e falhas antes do handoff.

## Validation

Antes de `Validation`, automated validation relevante conhecida deve passar.

Manual Validation é responsabilidade do owner e é especialmente obrigatória para comportamento que envolve PXE, boot firmware, real disks, Windows/WinPE, hardware específico e destructive workflows.

## Done

`Done` significa aceite manual do owner.

Registre somente outcome útil para trabalho futuro:

```text
Outcome
Automated validation
Manual validation
Related changes
```

Não replique transcript ou diff inteiro.

## Commits

Use Conventional Commits concisos quando autorizado, por exemplo:

```text
feat(agent): add enrollment handshake
fix(scheduler): release storage lease after cancellation
test(protocol): cover duplicate action acknowledgement
docs(architecture): record storage capability boundary
```

Detalhes de execução pertencem ao Work Package, não ao commit message.
