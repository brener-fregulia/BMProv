# Compatibilidade de hardware — conhecimento herdado do PoC

Este documento registra quirks observados no laboratório do FORGE. Eles **não são requisitos de hardware do BMProv**.

## MikroTik CRS326-24G-2S+RM / família testada

No laboratório anterior foram observados problemas de PXE/unicast que exigiram configuração específica de bridge:

- `hw=no` nas portas relevantes;
- `protocol-mode=none` para evitar atraso de STP no PXE;
- `fast-forward=no` para não bypassar o forwarding esperado.

Esses ajustes pertencem a um futuro profile/adapter de hardware quando aplicável, não ao domínio BMProv.

## Intel X520-DA2

No setup anterior, TX checksum offload no bond produziu checksums inválidos ao atravessar o switch testado. O workaround foi desabilitar `tx-checksumming` no bond após a rede subir.

Isso deve ser tratado como compatibility knowledge, não como configuração global obrigatória.

## Bonding

`active-backup` foi preferido a LACP no PoC porque o objetivo era redundância e a negociação de LACP introduzia atraso perceptível no boot PXE. BMProv não exige bonding específico.

## Bootloader experiments

A sequência experimentada foi:

1. pxelinux — descartado por não atender UEFI;
2. `ipxe.efi` — apresentou incompatibilidade no setup testado;
3. `snponly.efi` — conseguiu boot Alpine, mas apresentou instabilidade ao carregar kernel + initramfs maiores;
4. `grubx64.efi` — tornou o boot Alpine estável no laboratório.

GRUB permanece uma solução validada para aquele setup, mas a arquitetura BMProv deve manter PXE/boot atrás de boundary própria.

## Dynamic boot by endpoint

O PoC validou que o servidor precisa conseguir alterar o próximo ambiente de boot individualmente. A implementação anterior fazia isso por configuração GRUB específica por MAC.

BMProv deve preservar a capability de **desired next boot environment**, não o detalhe de path/configuração por MAC.
