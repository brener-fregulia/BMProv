# Architectural Decision Records

ADRs preservam decisões técnicas significativas para que constraints duráveis não sejam redescobertas ou alteradas silenciosamente em sessões futuras.

## Quando criar

Crie ADR quando uma decisão:

- estabelece ou muda boundary arquitetural durável;
- possui alternativas relevantes ou trade-offs não triviais;
- condiciona futuras implementações;
- adota ou rejeita tecnologia/estratégia significativa;
- é provável que seja questionada novamente.

Não crie ADR para naming rotineiro, formatação, idioma de documentação, escopo de release ou detalhes reversíveis de implementação.

## Naming

```text
0001-example-decision.md
0002-another-decision.md
```

Números não são reutilizados.

## Status

- `Proposed`
- `Accepted`
- `Superseded`
- `Deprecated`
- `Rejected`

Decisão Accepted só deve ser reaberta com requisito, constraint ou evidência nova.

## Estrutura

```markdown
# ADR-NNNN: Título

Status: Proposed

## Contexto

## Decisão

## Alternativas consideradas

## Consequências

## Arquitetura relacionada

## Trabalho relacionado
```

## SDD

Se uma decisão arquitetural surgir durante implementação:

1. registre a questão no Work Package;
2. confira ADRs existentes;
3. interrompa somente a escolha afetada;
4. documente alternativas;
5. obtenha aprovação do owner;
6. crie/atualize o ADR;
7. continue a implementação.
