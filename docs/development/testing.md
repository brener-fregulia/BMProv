# Testing

## Principles

- Test observable BMProv behavior, not dependency internals.
- Relevant automated tests are part of implementation from the first Work Package.
- Use deterministic, isolated test data.
- Run the narrowest relevant validation first.
- Reproducible bugs should receive a regression test when an active test layer can represent them reliably.
- Coverage is a diagnostic signal, not proof of correctness.
- Owner manual Validation remains necessary where automation cannot represent the real risk.

## Initial test layers

### Domain and application tests

From the first vertical slice, cover:

- Job/JobStep state transitions;
- resource leases;
- retries and cancellation;
- idempotency;
- stale inventory;
- endpoint identity decisions;
- destructive-action safety invariants.

### Protocol contract tests

Cover:

- schema and protocol-version compatibility;
- malformed messages;
- duplicate messages;
- correlation and acknowledgement;
- reconnect behavior;
- replay rejection;
- timeout and cancellation.

### Adapter contract tests

Fakes and real adapters should share verifiable contracts when useful, including storage, boot, and discovery providers.

### Data-plane tests

Cover relevant cases such as:

- slow producer or consumer;
- interruption;
- cancellation;
- corruption;
- digest mismatch;
- disk full;
- incomplete `.part` state;
- atomic completion;
- resume or checkpoint behavior when supported.

### Simulator tests

Local development and CI must be able to represent 20–24+ endpoints with configurable latency, throughput, disconnect/reconnect, failures, retries, and storage pressure without physical hardware.

### Web tests

The accepted direction is Vitest for Svelte/TypeScript behavior, with API and event boundaries simulated where appropriate.

## Safety

Normal automated tests must not perform destructive operations on real disks.

Minimum safety scenarios include:

- wrong disk;
- changed disk identity;
- stale inventory revision;
- duplicate destructive action;
- cancelled action;
- action after reconnect;
- missing or invalid authorization;
- interrupted destructive JobStep.

## Physical Integration Environment

Explicit laboratory validation remains necessary for:

- DHCP/PXE;
- GRUB/UEFI;
- Alpine diskless boot;
- real disks;
- Windows/WinPE;
- switch and NIC compatibility;
- destructive provisioning.

The laboratory does not replace local tests and CI. It covers behavior that is impossible or unsafe to represent completely through simulation.

## Reporting

Never claim validation passed unless it was actually executed.

Report the commands or checks that ran, their actual results, environment limitations, and remaining manual validation.
