# Bamep

> Open-source bare-metal provisioning platform.

Bamep is a platform for provisioning, recovery, and orchestration of physical endpoints over local networks.

The project is designed around automated bare-metal workflows such as network boot, hardware and storage inventory, backup and recovery, operating system deployment, and post-install automation.

Bamep is being developed as a clean implementation informed by lessons learned from an earlier proof of concept.

## Project status

Bamep is currently in its architecture and specification phase.

Production provisioning is not implemented yet.

The initial production target is:

- Windows provisioning;
- UEFI x86-64 endpoints;
- Linux-based Bamep Server;
- browser-based administration;
- standalone, single-server deployments.

## Components

Bamep is expected to consist of independently evolvable components such as:

- **Bamep Server**
- **Bamep Web**
- **Bamep Agent**
- **Bamep Simulator**

The exact implementation architecture is still being defined through Specification-Driven Development and Architecture Decision Records.

## License

Bamep is licensed under the [Apache License 2.0](LICENSE).

Copyright 2026 Brener Fregulia.
