# M0 ADR Triage

Status: **Operational proposal based on the approved Discovery baseline**

The goal is to identify likely durable architectural decisions while avoiding ADR
inflation.

An item listed here is an ADR candidate, not an automatic requirement.

Create an ADR only after Discovery identifies a durable decision with meaningful
alternatives, non-trivial trade-offs, or constraints that should be preserved for
future work.

## M0 ADR candidates

1. Runtime topology: modular monolith and worker/process isolation.
2. Endpoint identity and enrollment/trust bootstrap.
3. Agent control protocol.
4. Agent typed-action model, idempotency, retry, and cancellation.
5. Backend/Agent language strategy.
6. Persistence strategy for standalone deployments.
7. Durable Job/JobStep state model.
8. Data-plane and transfer protocol.
9. Transfer resumability and snapshot-format strategy.
10. Storage roles and capability model.
11. Scheduler and resource-lease model.
12. Security trust model and destructive-operation boundary.
13. Boot orchestration boundary between the domain and PXE/GRUB.

Some candidates may be resolved primarily through Specifications rather than
separate ADRs if Discovery reveals no meaningful architectural alternative.

Related candidates may also be combined into one ADR when they represent one
coherent architectural decision.

## May become ADRs after a Technical Spike

- definitive WinPE mechanism;
- Secure Boot or hardened boot chain;
- driver-provider integration;
- switch discovery adapter contract, if alternatives create durable constraints;
- packaging or service isolation, if implementation reveals a non-trivial long-term choice.

A Technical Spike provides evidence. Its result does not require an ADR unless a
durable architectural decision remains to be recorded.

## Do not require ADRs at this stage

These are product, scope, repository-convention, or operational decisions already
established by the owner:

- Bamep name and expected component names;
- Apache-2.0 licensing;
- canonical repository and engineering documentation in English;
- source/API/protocol identifiers in English;
- initial UI locale `pt-BR`, with `en-US` planned;
- Windows 11 as the primary V1 provisioning target;
- UEFI x86-64 for V1;
- Legacy BIOS outside V1 scope;
- HA outside V1 scope;
- a dedicated provisioning network/interface/VLAN as the initial deployment assumption;
- Internet access is not guaranteed;
- a single Server node initially;
- Debian as the first production Server target;
- GitHub Project statuses `Backlog`, `Ready`, `In Progress`, `Validation`, and `Done`;
- SemVer per independently deployable artifact;
- `SYSTEM`, `CACHE`, and `ARCHIVE` as already approved vocabulary.

If an item in this section later develops meaningful architectural alternatives or
new constraints, it may be reconsidered through normal SDD rather than because of
this triage document.