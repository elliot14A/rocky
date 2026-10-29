# rocky

> [!WARNING]
> **Work In Progress**: Rocky is under active development. CLI commands, system modules, and templates are evolving rapidly.

Opinionated NixOS fleet deployment tool written in Rust.

Rocky manages fleets of NixOS servers across bare-metal and cloud VPS providers without cluster daemons, etcd clusters, or runtime control planes. You write services as NixOS modules, map them to hosts in a declarative fleet file, and use the CLI to bootstrap blank machines, rekey secrets, and deploy changes.

## What it does

- **Compile-time placement**: Resolves internal service discovery and networking at Nix build time. Co-located services resolve to `127.0.0.1`, and remote services resolve over private Tailscale WireGuard addresses.
- **Kernel Layer 4 load balancing**: Service replicas are balanced directly in the Linux kernel on the destination host using `nftables` DNAT. Ingress proxies like Caddy connect directly to base ports with zero Layer 7 proxy overhead.
- **Zero-trust machine inception**: Uses Disko and `nixos-anywhere` to wipe blank VPS instances, inject local Ed25519 host SSH keys, and install NixOS over SSH.
- **Cryptographic secret scoping**: Derives Agenix recipient keys so each host only gets decryption keys for the secrets its assigned services need. Secrets remain encrypted at rest and in RAM on target hosts.
- **Dual workload support**: Runs native NixOS systemd services and sandboxed Podman OCI containers interchangeably.
- **Atomic deployment and rollback**: Builds host system closures and activates them with deploy-rs, automatically rolling back to the previous generation if activation fails.

## Commands

```bash
# Environment management
rocky env new <name>

# Host management
rocky host new <name> --env <env>

# Service scaffolding
rocky service new <name>

# Secret lifecycle
rocky secret new <env> <name>
rocky secret edit <env> <name>
rocky secret show <env> <name>
rocky secret rekey <env>

# Fleet lifecycle
rocky up <env>                  # bootstraps pending hosts, rekeys secrets, deploys fleet
rocky status <env>              # checks host connectivity and running generations
rocky ssh <env> <host>          # connects to a host over Tailscale
rocky log <env> <host> [svc]    # tails remote host or service systemd logs
```

## How an environment is structured

```text
environments/<env>/
  fleet.nix          # maps hosts to the services they run
  env.nix            # per-service non-sensitive environment variables
  keys.nix           # host SSH public keys registered during bootstrap
  hosts/<name>/      # disk layout (disko.nix) and host-specific settings
  secrets/           # agenix encrypted secret ciphertexts (.age)
services/*.nix       # one NixOS module per service (shared across environments)
```

Adding or moving a service is done in `environments/<env>/fleet.nix`:

```nix
hosts."001" = {
  provider = "hetzner";
  services = [ "postgres" "caddy" "api" ];
};
```

When you run `rocky up <env>`:
1. It inspects `fleet.nix` and `keys.nix` to identify uninstalled hosts.
2. For uninstalled hosts, it requests the rescue IP, generates host SSH keys locally, wipes the disk, and installs NixOS via `nixos-anywhere`.
3. It runs Agenix rekeying so secrets are encrypted for the new host keys.
4. It deploys system closures across the fleet using `deploy-rs`.

## Host lifecycle

1. **Declared**: You add a host entry in `fleet.nix` with its provider and services.
2. **Bootstrapped**: `rocky up` wipes the VPS disk via Disko, injects the host SSH key via `nixos-anywhere --extra-files`, and installs NixOS.
3. **Rekeyed**: The host public key is recorded in `environments/<env>/keys.nix`. Agenix rekeys existing secrets to include the new host key.
4. **Active**: The machine joins the private Tailscale mesh and runs its assigned systemd units or containers.
5. **Updated**: Subsequent `rocky up` runs deploy changes over Tailscale via `deploy-rs`. If activation fails, `deploy-rs` automatically rolls back to the previous generation.

## Secrets and security

- **Encryption at rest**: Secrets are encrypted using Agenix (Age) and decrypted into RAM at `/run/agenix/` upon host activation.
- **Recipient scoping**: Each secret is encrypted only for authorized operator keys plus the host keys that run that service. A frontend host never receives database credentials.
- **Firewall**: Inbound traffic on public interfaces is blocked by default. Hosts communicate over private Tailscale interfaces (`tailscale0`). Only public web ports (`80`, `443`) are exposed on hosts running Caddy.

## Development

```bash
cargo check
cargo test
nix flake check --no-build
```
