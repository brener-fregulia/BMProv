# AGENTS.md

## Purpose

Este arquivo define regras obrigatórias para agentes de IA trabalhando no BMProv.
Procedimentos detalhados pertencem a `docs/development/`; decisões técnicas duráveis pertencem a `docs/decisions/`.

## Sources of truth

O repositório é a fonte permanente de contexto técnico.
Após materialização do trabalho aprovado, GitHub Issues, Projects e Milestones são a fonte operacional de escopo e estado.

Antes de propor ou alterar algo:

- leia somente a documentação relevante à tarefa;
- confira implementação e testes existentes quando houver;
- confira ADRs relacionados;
- reporte conflito entre pedido, especificação, ADR e estado real;
- não invente comportamento, requisitos, APIs, caminhos, comandos ou resultados de validação.

Uma sessão de IA nunca pode ser a única fonte de informação necessária para continuar o trabalho.

## SDD and scope

Siga `docs/development/sdd.md`.

- Discovery é análise, não implementação.
- Antes da implementação deve existir especificação suficiente e aprovação do owner.
- Implemente uma responsabilidade ou Work Package aprovado por vez.
- Não expanda silenciosamente o escopo.
- Decisões arquiteturais relevantes não podem surgir silenciosamente em código.
- Testes automatizados relevantes fazem parte da implementação.
- `Validation` é a etapa de validação manual do owner antes de `Done`.

## Architecture

`docs/architecture/` descreve somente arquitetura que realmente existe.
Não documente arquitetura planejada como se estivesse implementada.

Antes de introduzir uma abstração, módulo, serviço, adapter, dependency ou boundary:

1. identifique o requisito atual que a justifica;
2. identifique a responsabilidade arquitetural correta;
3. confira ADRs existentes;
4. preserve boundaries aceitos ou proponha explicitamente sua mudança.

BMProv não deve herdar stack, diretórios ou runtime boundaries do FORGE ou Pascoal sem justificativa própria.

## Safety

BMProv executará operações destrutivas em discos.

- Nunca enfraqueça validações de identidade, inventário, autorização ou segurança para fazer um fluxo passar.
- Operações destrutivas devem possuir preconditions e safety invariants explícitos.
- MAC address é sinal de inventário, não autenticação nem identidade permanente.
- Não introduza execução remota arbitrária como substituto de Agent actions tipadas.
- Não exponha, armazene ou imprima secrets, tokens, credenciais ou chaves privadas.
- Testes não devem tocar discos ou dados reais do usuário salvo integração destrutiva explicitamente autorizada em ambiente adequado.

## Development environment

O servidor físico e o laboratório são Integration Environment, não requisitos de desenvolvimento.

A maior parte do projeto deve ser executável e testável localmente, preferencialmente em Linux e, para partes portáveis, também em Windows 11.
Use fakes, simuladores, temporary storage e fixtures determinísticos nas boundaries apropriadas.

## Git and publication

O owner mantém controle sobre Git e publicação, salvo autorização explícita e específica para a tarefa atual.
Não faça commit, push, merge, tag, release ou alteração de Project implicitamente.

Ao concluir alterações locais, sugira Conventional Commit quando útil.

## Validation

Use a validação mais estreita capaz de demonstrar o comportamento alterado e amplie quando o risco justificar.

- Não declare teste, build ou validação como concluído se não foi executado.
- Não esconda falhas nem enfraqueça checks.
- Diferencie falha causada pela mudança, limitação do ambiente e falha preexistente quando houver evidência.
- Informe claramente a validação manual restante.

## Documentation ownership

- `README.md`: visão pública do projeto;
- `docs/discovery/`: análise, alternativas e questões ainda não aceitas;
- `docs/specifications/`: trabalho futuro aprovado;
- `docs/architecture/`: arquitetura atual implementada;
- `docs/decisions/`: ADRs e raciocínio histórico;
- `docs/development/`: processo de engenharia;
- `docs/reference/`: conhecimento factual de integração e compatibilidade;
- GitHub Issues: trabalho aprovado materializado;
- GitHub Projects: estado operacional;
- GitHub Milestones: agrupamento de marco/release quando aplicável.

Cada informação deve ter uma fonte primária. Evite duplicação.

## Language

- source code, schemas, APIs, protocol fields, internal events and logs: English;
- documentação de arquitetura, ADRs, SDD, especificações e referência: pt-BR inicialmente;
- UI: strings user-facing devem respeitar boundary de localização, começando por `pt-BR` e preparando `en-US`.
