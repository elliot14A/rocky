# rocky

Opinionated NixOS fleet deployment tool written in Rust.

Rocky manages NixOS servers on cloud VPS providers (Hetzner Cloud and Oracle Cloud Infrastructure). You write your services as NixOS modules, map them to hosts in an environment file, and use the CLI to bootstrap blank machines and deploy changes.

## What it does

- **Opinionated fleet layout**: Sets up environments (`environments/<env>/`), shared services (`services/<name>.nix`), and base system modules (firewall, SSH, Tailscale).
- **Disk wiping and install**: Uses Disko and `nixos-anywhere` to format blank VPS instances, inject host SSH keys, and install NixOS over SSH.
- **Service addressing**: Computes internal service addresses and Caddy reverse proxy upstreams from the fleet configuration so services can find each other over Tailscale.
- **Secret scoping**: Derives Agenix recipient keys so each host only gets decryption keys for the secrets its assigned services need.
- **Deployment and rollback**: Builds system closures and applies them to running hosts with deploy-rs, automatically rolling back if a deploy fails.

## Commands

```bash
# Scaffold
rocky env new <name>            # environments/<name>/
rocky host new <name> --env <env> # environments/<env>/hosts/<name>/
rocky service new <name>         # services/<name>.nix

# Deploy
rocky up <env>                  # installs uninstalled hosts, rekeys secrets, deploys the fleet
rocky status <env>              # checks host connectivity and running generations
```

## How an environment is structured

```
environments/<env>/
  fleet.nix          # maps hosts to the services they run
  env.nix            # per-service environment variables
  hosts/<name>/      # disk layout (disko.nix) and host settings
  secrets/           # agenix encrypted secrets and host public keys
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
1. It checks `secrets/host-keys.nix` against `fleet.nix` to find uninstalled hosts.
2. For uninstalled hosts, it asks for the public IP, generates an SSH host key, wipes the disk, and installs NixOS.
3. It runs `agenix --rekey` so secrets are encrypted for the new host keys.
4. It deploys system closures across the fleet using `deploy-rs`.

## Host lifecycle

1. **Declared**: You add a host entry in `fleet.nix` with its provider and services.
2. **Bootstrapped**: `rocky up` wipes the blank VPS disk via Disko, generates a dedicated SSH host key locally, injects it via `nixos-anywhere --extra-files`, and installs NixOS.
3. **Rekeyed**: The host public key is recorded in `secrets/host-keys.nix`. Agenix rekeys existing secrets to include the new host key.
4. **Active**: The machine joins the private Tailscale mesh and runs its assigned systemd units or containers.
5. **Updated**: Subsequent `rocky up` runs deploy changes over Tailscale via `deploy-rs`. If activation fails, `deploy-rs` automatically rolls back to the previous generation.

## Secrets and security

- **Encryption at rest**: Secrets are encrypted using Agenix (Age) and committed to git as `.age` files. Plaintext secrets never touch git.
- **Recipient scoping**: Each secret is encrypted only for the operator's key plus the host keys that run that service. If host `002` only runs frontend code, it does not get decryption keys for database secrets.
- **Firewall**: Inbound network traffic on public interfaces is blocked by default using `nftables`. Hosts communicate over private Tailscale addresses (`tailscale0`). Only public web ports (`80`, `443`) are exposed on hosts running Caddy.

## How services run

Services in `services/<name>.nix` are standard NixOS modules. They run in one of two ways:

- **Native systemd services**: Binaries or packages managed directly by systemd (for example PostgreSQL, Caddy, or pre-built binaries).
- **OCI containers**: Docker images run via Podman using NixOS's built-in `virtualisation.oci-containers`. Containers run as systemd units with automatic restart on failure and access to Agenix secret files.

## Development

```bash
cargo build
cargo test
nix flake check
```
