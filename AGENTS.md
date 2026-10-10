# Instructions for AI Agents and Developers

Rocky is a minimal deployment tool and CLI for managing NixOS fleets without cluster daemons or runtime control planes.

## Core Rules

1. **Keep it Simple**: Do not invent new abstractions, configuration formats, or wrapper layers. No YAML, no TOML, no intermediary config files. The project is strictly Rust and Nix.
2. **Plain Language**: All documentation, code comments, and commit messages must use plain, direct English. Avoid AI-generated filler words and corporate jargon. Do not use em dashes.
3. **No Dynamic Dispatch**: Avoid `dyn Trait`, `Arc<dyn Trait>`, and unnecessary trait abstractions. Use concrete structs, modules, and static functions directly, following the patterns in `gaur`.
4. **Typed Error Channels**: Never use `unwrap()` or `expect()` in production or library code. All domain and system errors must be explicitly typed using `snafu`.
5. **Pure Domain vs System I/O**:
   - `src/fleet/`: Pure data calculations and validation. Zero I/O, zero network, zero filesystem access. Synchronous only.
   - `src/nixos/` and `src/network/`: System runners executing external tools (`nix`, `ssh`, `agenix`, `nixos-anywhere`, `tailscale`).
   - `src/commands/`: CLI command execution handlers (one file per action: `new.rs`, `up.rs`, `status.rs`).

## Verification Before Claims

Before marking any task complete or committing changes:
- Run Rust checks: `cargo check` and `cargo test`.
- Run strict Nix checks: `nix flake check` or `nix-instantiate --eval --strict`.
- Never claim a command passed without running it and inspecting the output.
