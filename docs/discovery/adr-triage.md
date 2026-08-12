# Triagem de ADRs para M0

Status: **Proposta operacional baseada no Discovery aprovado**

O objetivo é registrar somente decisões arquiteturais duráveis com alternativas relevantes, evitando ADR inflation.

## Requerem ADR

1. Runtime topology: modular monolith e isolamento de workers/processos.
2. Endpoint identity e enrollment/trust bootstrap.
3. Agent control protocol.
4. Agent typed-action model, idempotency, retry e cancellation.
5. Backend/Agent language strategy.
6. Persistence strategy para standalone.
7. Durable Job/JobStep state model.
8. Data-plane/transfer protocol.
9. Transfer resumability/snapshot-format strategy.
10. Storage roles/capability model.
11. Scheduler/resource lease model.
12. Security trust model e destructive-operation boundary.
13. Boot orchestration boundary entre domínio e PXE/GRUB.

## Podem virar ADR quando o spike produzir decisão

- mecanismo WinPE;
- Secure Boot / hardened boot chain;
- driver provider integration;
- switch discovery adapter contract, se as alternativas criarem constraint durável;
- packaging/service isolation se a implementação exigir escolha não trivial.

## Não precisam de ADR neste momento

Estas são decisões de produto, escopo, convenção ou operação já estabelecidas pelo owner:

- nome BMProv e nomes esperados dos componentes;
- Apache-2.0;
- documentação em pt-BR inicialmente;
- source/API/protocol identifiers em inglês;
- UI `pt-BR` inicialmente com `en-US` planejado;
- Windows 11 como alvo primário de V1;
- UEFI x86-64 em V1;
- Legacy BIOS fora do escopo V1;
- HA fora do escopo V1;
- dedicated provisioning network/interface/VLAN como hipótese inicial;
- Internet não garantida;
- Server único inicialmente;
- Debian como primeiro target de produção;
- GitHub Project com Backlog, Ready, In Progress, Validation e Done;
- SemVer por artifact independently deployable;
- `SYSTEM`, `CACHE`, `ARCHIVE` como vocabulário já aprovado — o ADR de storage deve decidir semântica/capabilities, não rediscutir esses nomes sem nova evidência.
