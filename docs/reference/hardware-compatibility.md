# Hardware Compatibility — Knowledge from the Previous PoC

This document records hardware and boot compatibility observations from the previous
FORGE laboratory.

These observations are **not BMProv hardware requirements**.

Exact software, firmware, and bootloader versions were not preserved in the current
BMProv reference material. Results that may depend on version or environment must
therefore be revalidated before being treated as current compatibility guarantees.

## MikroTik CRS326-24G-2S+RM / tested family

The previous laboratory observed PXE/unicast problems with hardware bridge
offloading enabled.

The tested workaround used:

- `hw=no` on relevant ports;
- `protocol-mode=none` to avoid STP delay during PXE boot;
- `fast-forward=no`.

This is compatibility evidence from that environment, not a required BMProv network
configuration.

## Intel X520-DA2

In the previous setup, TX checksum offload on the bond produced invalid checksums
when traffic crossed the tested switch.

Disabling `tx-checksumming` on the bond after networking came up resolved the
observed problem.

This is compatibility evidence from the tested environment, not a mandatory global
configuration.

## Bonding

`active-backup` was preferred over LACP in the previous laboratory because
redundancy was the goal and LACP negotiation introduced noticeable PXE boot delay.

The experiment does not establish a general BMProv bonding requirement.

## Bootloader experiments

The tested sequence was:

1. pxelinux — rejected because it did not satisfy the UEFI target;
2. `ipxe.efi` — showed incompatibility in the tested setup;
3. `snponly.efi` — booted Alpine but became unstable while loading larger kernel and
   initramfs payloads;
4. `grubx64.efi` — provided stable Alpine boot behavior in the tested laboratory.

The experiment validates GRUB as a working solution for that environment. It does
not establish GRUB as the permanent BMProv boot implementation.

Current BMProv boot architecture and future decisions belong in Discovery,
Specifications, and ADRs rather than this reference document.

## Dynamic boot by endpoint

The previous PoC demonstrated the operational need to select the next boot
environment independently for each endpoint.

Its implementation used MAC-specific GRUB configuration.

The reusable evidence is the per-endpoint boot-selection requirement observed by the
PoC. The MAC-specific configuration mechanism is an implementation detail of the
previous system and is not a BMProv constraint.

See `../discovery/architecture-redesign.md` for the current BMProv architectural
direction derived from this and other evidence.