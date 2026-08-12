# BMProv

> **BMProv is an open-source bare-metal provisioning platform.**

BMProv (Bare-Metal Provisioning) é uma plataforma open source para provisionamento e recuperação bare-metal em redes locais controladas.

O projeto está em fase inicial de **Specification-Driven Development (SDD)**. O repositório começa pela definição de requisitos, contratos, limites arquiteturais, decisões e estratégia de testes. Não há implementação de produção neste bootstrap.

## Direção inicial

- V1 focada em provisionamento de Windows 11 em UEFI x86-64;
- administração browser-first;
- Server Linux, inicialmente direcionado a Debian;
- desenvolvimento local sem dependência do laboratório físico;
- Simulator como requisito de primeira classe;
- modular monolith no Server, com isolamento de cargas pesadas quando necessário;
- Server, Web e Agent versionados independentemente;
- API e protocolos versionados separadamente das versões dos artefatos;
- armazenamento modelado por papéis lógicos `SYSTEM`, `CACHE` e `ARCHIVE`;
- segurança e validação explícitas para operações destrutivas.

## Componentes esperados

- **BMProv Server** — control plane, domínio, orchestration, scheduling e adapters;
- **BMProv Web** — interface administrativa browser-first;
- **BMProv Agent** — runtime efêmero no ambiente de manutenção;
- **BMProv Simulator** — simulação determinística de endpoints e infraestrutura;
- **BMProv Extensions** — futuras extensões desacopladas por contratos públicos.

Os nomes técnicos esperados incluem `bmprov-server`, `bmprov-web`, `bmprov-agent` e `bmprov-simulator`.

## Status

O primeiro marco é **M0 — Architecture Baseline & Simulated Provisioning Contract**.

M0 não contém provisionamento de produção. Seu objetivo é transformar Discovery em uma baseline arquitetural e de especificação aprovada antes do primeiro vertical slice de implementação.

Consulte:

- [`docs/discovery/`](docs/discovery/) para análise e questões ainda em decisão;
- [`docs/specifications/`](docs/specifications/) para trabalho futuro aprovado;
- [`docs/architecture/`](docs/architecture/) somente para arquitetura efetivamente implementada;
- [`docs/decisions/`](docs/decisions/) para ADRs;
- [`docs/development/`](docs/development/) para SDD, workflow e testes;
- [`docs/reference/`](docs/reference/) para conhecimento validado do PoC e compatibilidade de hardware.

## Licença

BMProv é distribuído sob a [Apache License 2.0](LICENSE). Consulte também [`NOTICE`](NOTICE).
