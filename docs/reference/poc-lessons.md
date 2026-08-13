# Validated Lessons from the FORGE PoC

FORGE was a previous private PoC/TCC project.

BMProv does not copy its source code or Git history. This document preserves only
sanitized technical observations and lessons from that work.

These findings are historical evidence, not BMProv architectural requirements.
Current BMProv requirements and architectural direction belong in Specifications,
Discovery, and ADRs.

## Validated observations

The previous PoC demonstrated that:

- diskless Alpine boot into RAM was viable as a maintenance environment;
- multi-stage workflows required selecting the next boot environment independently
  for each endpoint;
- storage inventory could take significantly longer than basic inventory, creating
  problems when liveness depended on the same execution path;
- `/dev/sdX` was not reliable as persistent disk identity;
- workflows could operate without CDN or Internet access after required artifacts
  were available locally;
- control-plane traffic and large-data transfers had substantially different
  operational characteristics;
- CPU, endpoint disk, network, and Server storage became bottlenecks under different
  workloads;
- fixed concurrency based only on endpoint count did not represent actual resource
  pressure well;
- summary fleet payloads combined with detailed data on demand reduced unnecessary
  traffic compared with always returning complete endpoint state;
- updating the Agent runtime independently from the initramfs significantly improved
  development iteration speed.

These observations may inform BMProv design, but their architectural implications
must be established through the normal SDD process.

## PoC technology choices

The following were implementation choices of the previous PoC and are not inherited
BMProv constraints:

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

Their previous use is evidence that they existed in the PoC, not justification for
selecting or rejecting them in BMProv without current requirements and analysis.

## Problematic patterns observed in the PoC

The previous implementation exposed limitations associated with:

- transport connections mixed with domain/runtime state;
- orchestration implemented directly inside HTTP routes;
- global mutable state used as a primary architectural boundary;
- filesystem, subprocess, and networking concerns coupled directly to
  presentation/API code;
- arbitrary remote shell execution;
- MAC address treated as identity or a trust anchor;
- absence of an authentication boundary;
- destructive operations without explicit safety invariants;
- hardcoded storage layout;
- concurrency implicitly controlled by ports or process counts;
- CPU-heavy work without resource quotas;
- blocking or heavy work capable of starving the control plane;
- dependence on the physical Server for normal development;
- automated tests introduced late.

This section records historical engineering evidence. It does not independently
define BMProv architecture or requirements.

See `../discovery/architecture-redesign.md` for the current BMProv direction derived
from these and other inputs.