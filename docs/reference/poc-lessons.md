# Lições validadas pelo FORGE PoC

O FORGE foi um PoC/TCC anterior e privado. BMProv não copia seu código nem seu histórico; preserva somente conhecimento técnico sanitizado.

## Evidências que sobrevivem ao redesign

- boot diskless de ambiente Alpine em RAM é viável para manutenção;
- decisão do próximo boot por endpoint é necessária em workflows multi-stage;
- inventário de storage pode ser significativamente mais lento que inventário básico e não deve bloquear liveness;
- `/dev/sdX` não é identidade persistente de disco;
- provisioning deve funcionar sem dependência de CDN/Internet após artifacts locais estarem disponíveis;
- control plane e large-data transfer possuem requisitos diferentes;
- CPU, disk, network e storage server podem alternar como bottleneck;
- concurrency precisa ser gerenciada por capacidade, não por número mágico;
- enviar payload resumido para visão geral e detalhes sob demanda reduz tráfego e coupling;
- runtime do Agent atualizável separadamente do initramfs melhora muito o ciclo de desenvolvimento.

## Choices do PoC que não são constraints

Não herdar automaticamente:

- FastAPI/Python;
- PostgreSQL;
- Vanilla JavaScript;
- WebSocket;
- HTTP agent em porta específica;
- raw TCP em range fixo;
- shell como Agent permanente;
- `sh -c` remoto;
- IPs e paths hardcoded;
- hot cache obrigatório;
- RAID1;
- SNMP;
- zstd com todos os cores;
- terminal remoto como capability normal.

## Architectural mistakes a evitar

- transport connection misturada ao domain/runtime state;
- orchestration dentro de HTTP routes;
- global state como boundary principal;
- filesystem/subprocess/networking diretamente acoplados à apresentação;
- arbitrary remote shell;
- MAC tratado como identity/trust anchor;
- ausência de autenticação;
- destructive operations sem safety boundary explícita;
- storage layout hardcoded;
- concorrência implícita por portas/processos;
- CPU-heavy work sem quotas;
- blocking work capaz de afetar control plane;
- depender do servidor físico para desenvolvimento;
- testes introduzidos tarde.
