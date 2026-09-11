# Ben

## What This Is

Ben is a declarative NixOS deployment framework and Rust-powered CLI that enables Kubernetes-style service placement and cryptographically-scoped secrets for small-to-mid NixOS fleets — evaluated purely at build time with zero runtime control plane overhead. It formalizes and generalizes the production-proven architecture developed across `gaur-deployments` and `fincher/deploy` into an open-source, reusable framework.

## Core Value

Declarative, compile-time service placement (resolving co-located services to loopback and distributed services to private mesh) combined with topology-derived cryptographic secret scoping, executed through an atomic, resumable bootstrap-reconciliation loop.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] **Pure Nix Evaluation Engine (`ben.lib`)**:
  - [ ] `mkFleet`: Pure attrset-driven fleet generator producing `nixosConfigurations` and `deploy.nodes` without mandatory directory side-effects.
  - [ ] `mkFleetFromDir`: Opt-in directory convention sugar (`environments/<env>/`, `services/`, `providers/`).
  - [ ] Dynamic compile-time topology graph (`serviceHost` computing `127.0.0.1` when co-located vs mesh hostname when remote).
  - [ ] Multi-replica upstream calculator (`serviceUpstreams`) generating backend tables for reverse proxies.
  - [ ] Cryptographic secret recipient scoping (`recipients.nix` deriving public keys directly from the fleet service placement graph).
  - [ ] Pluggable address resolution engine (`tailscale`, `wireguard`, `dns`, `local`).

- [ ] **Standalone NixOS Modules (`ben.nixosModules`)**:
  - [ ] Standalone systemd sandboxing module (`hardening.nix`) with fine-grained toggles and escape hatches.
  - [ ] Baseline host hygiene (`base.nix`, journal limits, deploy user, baseline tools).
  - [ ] Hardened SSH daemon (`ssh.nix`) and default-deny firewall (`firewall.nix`).
  - [ ] Provider hardware profiles (Hetzner, GCP, AWS, Baremetal).
  - [ ] Disko disk partitioning presets (GPT+Ext4, NVMe+Ext4, Btrfs subvolumes).

- [ ] **Native Rust CLI (`cli/` -> `ben`)**:
  - [ ] `ben init`: Scaffold fresh deployments from built-in templates.
  - [ ] `ben up <env>`: Full reconciliation state machine (detect uninstalled hosts, generate Ed25519 keys locally, inject via `nixos-anywhere --extra-files`, capture public keys in `host-keys.nix`, rekey agenix secrets, deploy via `deploy-rs`).
  - [ ] `ben deploy <env> [<host>]`: Fast CI/CD-friendly deploy-rs wrapper over mesh/IP without secret rekeying.
  - [ ] `ben status [<env>]`: Matrix view of environments, declared hosts, installed host keys, and running services.
  - [ ] `ben doctor [<env>]`: Structural and cryptographic invariant validator.
  - [ ] `ben logs <env>-<host> [<unit>]`: Stream systemd journal over SSH/mesh.
  - [ ] `ben new <env|host|service>`: Scaffold environments, hosts, and services.
  - [ ] `ben rm host <env> <name>`: Guided decommission with immediate secret rekeying.
  - [ ] `ben session --identity <key>`: Subshell scoped with `ssh-agent`.
  - [ ] `ben secrets <edit|rekey>`: Agenix secret manager wrapper.

- [ ] **Flake Packaging & Templates**:
  - [ ] Hermetic `flake.nix` exposing `lib`, `nixosModules`, `packages.ben`, `apps.ben`, and templates (`minimal`, `full-stack`).
  - [ ] Developer shell with Rust toolchain, Nix tools (`deploy-rs`, `agenix`, `nixos-anywhere`, `disko`).

### Out of Scope

- **Runtime Control Plane / Agent Daemon**: Ben is strictly build-time evaluated; no centralized agent or Kubernetes-like daemon is installed on nodes.
- **Stateful Cloud Provisioning**: Cloud VM creation (Terraform/OpenTofu/Terranix) is out of scope; Ben manages VMs once an IP/SSH interface is reachable.
- **Imperative Container Shipping**: Core Ben does not include company-specific container build/save/ssh-load logic; it focuses on native NixOS closures.
- **High-Availability Distributed Consensus (HA Postgres/Raft)**: Stateful clustering is left to user application modules or external databases.

## Context

- Evolved from two real-world implementations:
  1. `fincher/deploy`: Single-host hackathon deployment with hardcoded literals, demonstrating the exact operational pain Ben solves.
  2. `gaur-deployments`: Multi-host, multi-environment production fleet that proved the viability of compile-time placement, host-key pre-injection, and agenix recipient scoping.
- Research confirmed a clear gap in the ecosystem between raw low-level tools (`deploy-rs`, `colmena`) and heavy P2P/runtime systems (`clan.lol`).
- Built for solo developers, startups, and agencies managing fleets on Hetzner, GCP, AWS, or bare-metal who desire clean co-location semantics without the operational burden of Kubernetes.

## Constraints

- **Tech Stack**: Rust for the CLI tool (packaged via Cargo / Crane in Nix), pure Nix for `lib/` and `modules/`.
- **Pure Evaluation**: The core `mkFleet` engine must evaluate cleanly without undeclared impure path reads, avoiding infinite recursion risks by keeping placement logic data-driven over `fleet.nix`.
- **Decoupled Architecture**: Engine must not hardcode opinions on specific services (Postgres, NATS, etc.) or mesh providers (Tailscale is a pluggable resolver, not a hardcoded mandate).
- **Hermetic CLI**: The Rust CLI must build and run as a standalone binary without needing external Node/Bun or ad-hoc scripting environments.

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Use Rust for CLI | Rust provides a single native binary, fast execution, strong type safety, and cultural fit for the Nix/systems community. | — Pending |
| Attrset-First `mkFleet` with Dir Sugar | Pure attrsets prevent flake evaluation purity/IFD issues; `mkFleetFromDir` provides familiar DX without forcing rigid repo layouts. | — Pending |
| Pluggable Address Resolvers | Decouples Ben from mandatory Tailscale usage, allowing WireGuard, DNS, or custom IP schemes. | — Pending |
| Separate `ben up` (Operator) vs `ben deploy` (CI) | Resolves the CI/CD secret blast-radius problem by isolating privileged host provisioning and rekeying from automated rollout activation. | — Pending |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd-transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd-complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-09-11 after initialization*
