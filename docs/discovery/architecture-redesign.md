# BMProv — Architecture Discovery Baseline

Status: **Baseline de Discovery atualizada com decisões do owner**

## Contexto

BMProv é uma implementação nova, sem histórico Git ou código herdado do FORGE. O PoC anterior é evidência técnica: comportamento validado, limitações, workarounds e erros arquiteturais. Ele não é uma compatibility constraint.

## Boundary do produto

BMProv é uma plataforma standalone de bare-metal provisioning e recovery para redes locais controladas. Ela deve descobrir e identificar endpoints, coordenar boot e ambientes de manutenção, coletar inventário, executar workflows, transferir e gerenciar artifacts, agendar recursos concorrentes e oferecer operação segura, observável e auditável por API e interface web.

BMProv V1:

- provisiona Windows, com Windows 11 como alvo moderno primário;
- suporta UEFI x86-64;
- opera inicialmente em servidor único;
- assume interface/VLAN/rede dedicada de provisioning onde BMProv pode controlar DHCP/PXE;
- não depende de Internet quando os artifacts necessários já estão locais;
- não exige MikroTik, hot cache, cold storage, RAID, PostgreSQL ou WebSocket.

BMProv não é ERP, CRM, sistema financeiro, RMM genérico, NAS, gerenciador geral de switches ou plataforma multi-site V1. Um futuro ERP deve integrar por API pública/versionada e domain events, nunca pelo banco interno.

## Component boundaries propostos

Antes de tecnologia definitiva, as responsabilidades são separadas em:

- Presentation: Web Administration e Administrative API;
- Application: Endpoint Management, Provisioning/Recovery Orchestration, Boot Orchestration e Artifact Management;
- Domain: Endpoint, Job, JobStep, Attempt, Inventory, Artifact/Snapshot, Transfer, Storage Target e Domain Events;
- Runtime Services: Scheduler/Resource Arbiter, Agent Control Gateway, Transfer Coordinator e Runtime Presence Registry;
- Ports: repositories, Agent transport, boot, discovery, storage e infrastructure metrics;
- Adapters: persistence, PXE/GRUB, switch integration, filesystem/storage e protocol transports;
- Workers: transfer, compression, verification e artifact movement.

O domínio não deve conhecer GRUB, MikroTik, `/dev/sda`, `snmpwalk`, WebSocket, SQLite ou zstd.

## Runtime direction aceita

A direção inicial é **modular monolith first**, com boundaries internas explícitas e isolamento por processo/worker para cargas pesadas quando necessário.

Microservices, clustering, Redis, leader election e distributed scheduler não são requisitos V1.

Workers pertencem inicialmente à release do Server e não recebem versionamento independente.

## Development architecture

O laboratório físico é Integration Environment.

O desenvolvimento normal deve funcionar sem PXE real, MikroTik, clientes reais ou discos destrutivos por meio de:

- simulated agents;
- fake boot/discovery/storage adapters;
- temporary local storage;
- deterministic fixtures;
- simulação de 20–24+ endpoints;
- cenários de latency, throughput, disconnect, reconnect, retries, failure e storage pressure.

## Frontend

Direção aceita, salvo blocker concreto descoberto posteriormente:

- TypeScript;
- Svelte;
- Vite;
- Vitest;
- administração browser-first.

BMProv Web é independently deployable/updateable em relação ao Server. Um bugfix exclusivamente Web não deve exigir reiniciar jobs do Server.

## Backend e Agent

Ainda não decididos.

Candidatos principais: Rust, Go e Python. Antes de aceitar arquitetura polyglot, deve-se avaliar se uma única linguagem atende razoavelmente Server e Agent, considerando que o projeto possui um único mantenedor principal.

O Agent permanente não deve aceitar `sh -c` arbitrário vindo do Server. A direção é um supervisor com actions tipadas, autenticação, state machine, retries, cancellation e process supervision, podendo invocar ferramentas fixas do ambiente Alpine.

## Control plane

A escolha de protocolo continua aberta e deve ser feita por ADR.

Browser e Agent não precisam usar o mesmo mecanismo.

Candidatos relevantes:

- REST + polling;
- REST + long polling;
- WebSocket com protocolo tipado;
- SSE para eventos browser + HTTP para commands.

Qualquer Agent Protocol deve especificar correlation, acknowledgement, duplicate handling, timeout, reconnect, cancellation, progress, version e idempotency semantics.

## Data plane

Grandes transfers ficam separados do control plane.

HTTP streaming/chunk-oriented transfer é direção forte, mas resumability não pode ser fingida por byte offset quando a fonte não consegue reproduzir o stream a partir daquele ponto.

Production V1 deve oferecer resume/checkpoint para grandes transfers quando tecnicamente possível. Volume-image e selective/chunked backup podem exigir estratégias diferentes.

## Persistence

Durable domain state/history deve ser separado de runtime connection/presence state.

SQLite é candidato forte para o standalone single-node; PostgreSQL continua alternativa se requisitos concretos de concorrência, remote DB, HA ou multi-site aparecerem. A decisão permanece ADR de M0.

## Storage

Papéis lógicos aceitos:

- `SYSTEM`;
- `CACHE`;
- `ARCHIVE`.

CACHE e ARCHIVE dedicados são opcionais. Um único SSD/NVMe pode cumprir múltiplos papéis em instalação pequena. Hardware layout é configuração de instalação.

Storage providers devem expor capabilities em vez de assumptions sobre RAID ou device names.

## Capacity and scheduling

Perfis iniciais de instalação:

- Small: ~3–5 endpoints ativos;
- Medium: ~8–10;
- High-density: ~20–24.

8 GB é o baseline mínimo pretendido do host completo para Small, sujeito a medição antes de 1.0.

Concorrência não deve ser um número global fixo. JobSteps devem competir por resource leases/tokens representando endpoint exclusivity, network, storage read/write, CPU/worker capacity e outros recursos relevantes.

## Security invariants

A LAN de provisioning é controlada, mas não confiável por definição.

- MAC não é autenticação nem identidade permanente;
- Server e Agent precisam autenticar-se;
- operações destrutivas precisam validar Endpoint, inventory revision e disk identity/fingerprint;
- reconnect não pode causar replay cego de comando destrutivo;
- Agent actions devem ser tipadas;
- backups críticos precisam de integrity verification antes de provisioning destrutivo;
- PTY/shell remoto, se existir, é break-glass e disabled by default;
- boot-chain integrity é requisito, embora Secure Boot não seja requisito M0.

## Endpoint identity

A identidade deve sobreviver à troca de NIC/MAC.

Direção para ADR:

1. Boot Orchestrator cria contexto/credential de enrollment curto;
2. Agent autentica o Server;
3. Agent resgata credential curto;
4. runtime identity/session credential é estabelecida;
5. MAC e hardware fingerprints permanecem sinais de inventário, não trust anchors.

## Backup model

Não existe `backup=true` genérico.

Estratégias mínimas a especificar separadamente:

- Volume/Image backup;
- Selective backup.

Todo artifact completo precisa de metadata/expected size quando aplicável, cryptographic digest, estado incompleto explícito, commit atômico e verification state.

## Durable workflow

Cada estágio relevante de provisioning é um JobStep com preconditions, execution state, result, postconditions, retry semantics e cancellation semantics.

Após power loss ou reconnect, o Server deve reconciliar estado real e durable state. Operações destrutivas não recebem retry automático apenas por política genérica.

## Observability

Correlation deve permitir relacionar endpoint, job, step, attempt, action e transfer.

Eventos de domínio duráveis devem existir quando úteis ao próprio produto e às integrações futuras, por exemplo provisioning completed/failed, artifact created/verified e inventory updated.

Telemetria de alta frequência não precisa ser persistida indefinidamente.

## Open source / commercial boundary

O standalone BMProv permanece genuinamente útil no open source: Server, Agent, Web, orchestration, scheduler, backup/recovery, artifact handling, Simulator, API, adapters básicos e observability essencial.

Futuras diferenciações comerciais podem existir acima ou ao redor do engine: ERP, multi-site, centralized management, advanced reporting, hosted services, support e specialized integrations.

Não criar forks condicionais por cliente.

## Packaging and versioning

Direção aceita:

- Server Linux, Debian como target inicial;
- `.deb` e APT repository assinada como distribuição eventual;
- sem self-updater silencioso do Server;
- Server, Web e Agent com SemVer independente;
- contracts versionados separadamente, por exemplo Administrative API v1 e Agent Protocol v1;
- sem lockstep de releases.

## Spikes explicitamente isolados

Não podem ser decididos silenciosamente durante implementação:

- mecanismo definitivo de WinPE;
- transfer/snapshot resumability quando o produtor não suporta restart arbitrário;
- Secure Boot/hardened boot chain;
- driver provider integration.
