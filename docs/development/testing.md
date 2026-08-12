# Testing

## Principles

- Teste comportamento BMProv observável, não internals de dependências.
- Testes automatizados relevantes fazem parte da implementação desde o primeiro Work Package.
- Use dados determinísticos e isolados.
- Run the narrowest relevant validation first.
- Bugs reproduzíveis recebem regression test quando a camada adequada consegue representá-los.
- Coverage é sinal diagnóstico, não prova de correção.
- Manual Validation do owner continua necessária onde automação não representa o risco real.

## Initial layers

### Domain/application tests

Devem cobrir desde o primeiro vertical slice:

- Job/JobStep state transitions;
- resource leases;
- retries e cancellation;
- idempotency;
- stale inventory;
- endpoint identity decisions;
- destructive-action safety invariants.

### Protocol contract tests

- schema/version compatibility;
- malformed messages;
- duplicate messages;
- correlation/acknowledgement;
- reconnect;
- replay rejection;
- timeout/cancellation.

### Adapter contract tests

Fakes e adapters reais devem compartilhar contracts verificáveis sempre que útil, por exemplo storage, boot e discovery providers.

### Data-plane tests

- slow producer/consumer;
- interruption;
- cancellation;
- corruption;
- digest mismatch;
- disk full;
- incomplete `.part` state;
- atomic completion;
- resume/checkpoint quando suportado.

### Simulator tests

CI/local development deve representar 20–24+ endpoints com latência, throughput, disconnect/reconnect, failure, retries e storage pressure sem hardware físico.

### Web tests

A direção aceita é Vitest para Svelte/TypeScript, com APIs/event boundaries simulados quando apropriado.

## Safety

Testes normais não devem executar operações destrutivas em discos reais.

Cenários de segurança mínimos incluem:

- wrong disk;
- changed disk identity;
- stale inventory revision;
- duplicate destructive action;
- cancelled action;
- action after reconnect;
- missing/invalid authorization;
- interrupted destructive JobStep.

## Physical Integration Environment

Validação em laboratório permanece explícita para:

- DHCP/PXE;
- GRUB/UEFI;
- Alpine diskless;
- real disks;
- Windows/WinPE;
- switch/NIC compatibility;
- destructive provisioning.

O laboratório não substitui testes locais e CI; ele cobre behavior impossível ou inseguro de simular completamente.

## Reporting

Nunca declare que uma validação passou sem executá-la.
Reporte comandos/checagens executados, resultados reais, limitações do ambiente e manual validation restante.
