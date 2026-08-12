# Hardware Compatibility — Knowledge from the Previous PoC

This document records quirks observed in the FORGE laboratory. They are **not BMProv hardware requirements**.

## MikroTik CRS326-24G-2S+RM / tested family

The previous laboratory observed PXE/unicast problems that required specific bridge configuration:

- `hw=no` on relevant ports;
- `protocol-mode=none` to avoid STP delay during PXE boot;
- `fast-forward=no` so forwarding did not bypass the expected software path.

These settings belong to a future hardware profile or adapter when applicable, not to the BMProv domain.

## Intel X520-DA2

In the previous setup, TX checksum offload on the bond produced invalid checksums when traffic crossed the tested switch. The workaround was to disable `tx-checksumming` on the bond after networking came up.

This must be treated as compatibility knowledge, not a mandatory global configuration.

## Bonding

`active-backup` was preferred over LACP in the PoC because redundancy was the goal and LACP negotiation introduced noticeable PXE boot delay. BMProv does not require a specific bonding mode.

## Bootloader experiments

The tested sequence was:

1. pxelinux — rejected because it did not satisfy the UEFI target;
2. `ipxe.efi` — showed incompatibility in the tested setup;
3. `snponly.efi` — booted Alpine but became unstable while loading larger kernel + initramfs payloads;
4. `grubx64.efi` — made Alpine boot stable in the laboratory.

GRUB remains a validated solution for that setup, but BMProv architecture must keep PXE/boot behind its own boundary.

## Dynamic boot by endpoint

The PoC validated that the Server needs to control the next boot environment independently for each endpoint. The previous implementation did this through MAC-specific GRUB configuration.

BMProv should preserve the **desired next boot environment** capability, not the MAC-specific file/path implementation detail.
