# M0 — Architecture Baseline & Simulated Provisioning Contract

Status: **Approved milestone scope; internal decisions remain to be resolved through ADRs/specifications**

## Goal

Transformar o Discovery em uma baseline arquitetural e de contratos aprovada pelo owner antes do primeiro Work Package de implementação.

## Scope

M0 deve resolver ou isolar explicitamente:

- product boundary e domain vocabulary;
- Endpoint identity;
- Job/JobStep lifecycle;
- scheduler/resource model;
- Agent action model;
- control-plane contract;
- data-plane contract;
- persistence;
- backend/Agent stack;
- security/trust model;
- storage capabilities;
- Simulator contract;
- observability/domain events;
- packaging/versioning baseline;
- testing policy.

## Out of scope

- production provisioning implementation;
- real disk formatting;
- real Windows installation;
- WinPE implementation;
- MikroTik-specific production adapter;
- production backup format;
- ERP;
- licensing enforcement;
- multi-site;
- HA;
- Tauri.

## Required technical spikes

Questões que exigirem evidência empírica devem virar spikes explícitos, especialmente:

- WinPE mechanism;
- resumable volume/image transfer format;
- Secure Boot/hardened boot chain quando exigido;
- driver provider integrations.

## Acceptance criteria

M0 está concluído quando:

1. product boundary, vocabulary e non-goals estão persistidos;
2. ADRs bloqueadores estão Accepted ou a questão está isolada em spike explícito;
3. destructive operations possuem safety invariants especificados;
4. simulated vertical slice possui comportamento, contracts e failure scenarios definidos;
5. responsibilities e boundaries dos componentes estão claros;
6. requisitos relevantes possuem estratégia de validação;
7. nenhuma decisão arquitetural necessária está escondida em futuro Work Package;
8. o owner aprova explicitamente a baseline.

## First implementation slice after M0

O primeiro vertical slice posterior deve funcionar sem hardware real:

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

O slice deve admitir cenário de 20–24 simulated endpoints concorrentes.
