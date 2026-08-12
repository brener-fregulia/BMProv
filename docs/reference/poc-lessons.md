# Validated Lessons from the FORGE PoC

FORGE was a previous private PoC/TCC project. BMProv does not copy its source code or Git history; it preserves only sanitized technical knowledge.

## Evidence that survives the redesign

- diskless Alpine boot into RAM is viable for the maintenance environment;
- selecting the next boot environment per endpoint is necessary for multi-stage workflows;
- storage inventory can be significantly slower than basic inventory and must not block liveness;
- `/dev/sdX` is not a persistent disk identity;
- provisioning must work without CDN or Internet dependency once required artifacts are local;
- control-plane traffic and large-data transfer have different requirements;
- CPU, endpoint disk, network, and server storage may each become the bottleneck under different workloads;
- concurrency must be managed through capacity rather than a magic fixed number;
- summary payloads for fleet views and detailed payloads on demand reduce traffic and coupling;
- updating the Agent runtime separately from the initramfs greatly improves development iteration speed.

## PoC choices that are not constraints

Do not automatically inherit:

- FastAPI/Python;
- PostgreSQL;
- Vanilla JavaScript;
- WebSocket;
- an Agent HTTP service on a fixed port;
- raw TCP over a fixed port range;
- shell as the permanent Agent implementation;
- remote `sh -c`;
- hardcoded IP addresses and paths;
- mandatory hot cache;
- RAID1;
- SNMP;
- zstd using all CPU cores;
- remote terminal as a normal production capability.

## Architectural mistakes to avoid

- transport connections mixed with domain/runtime state;
- orchestration implemented directly inside HTTP routes;
- global mutable state used as a primary architectural boundary;
- filesystem, subprocess, and networking concerns coupled directly to presentation/API code;
- arbitrary remote shell execution;
- MAC address treated as identity or a trust anchor;
- no authentication boundary;
- destructive operations without explicit safety invariants;
- hardcoded storage layout;
- concurrency implicitly controlled by ports or process counts;
- CPU-heavy work without quotas;
- blocking/heavy work capable of starving the control plane;
- dependence on the physical Server for normal development;
- automated tests introduced late.
