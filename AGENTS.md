# Instructions for AI Agents and Developers

Rocky is a minimal deployment tool and CLI for managing NixOS fleets without cluster daemons or runtime control planes.

## Core Rules

1. **Keep it Simple**: Do not invent new abstractions, configuration formats, or wrapper layers. No YAML, no TOML, no intermediary config files. The project is strictly Rust and Nix.
2. **Plain Language**: All documentation, code comments, and commit messages must use plain, direct English. Avoid AI-generated filler words and corporate jargon. Do not use em dashes.
3. **No Dynamic Dispatch**: Avoid `dyn Trait`, `Arc<dyn Trait>`, and unnecessary trait abstractions. Use concrete structs, modules, and static functions directly, following the patterns in `gaur`.
4. **Typed Error Channels**: Never use `unwrap()` or `expect()` in production or library code. All domain and system errors must be explicitly typed using `snafu`.
5. **Action-Based Entity Layout**:
   - Every file contains only code related to its specific action (`<entity>/<action>.rs`).
   - No generic `utils/` folder. Filesystem logic lives in `src/fs/`, process execution lives in `src/process/`, and Nix tooling lives in `src/nix/`.
   - Domain scaffolding engine lives in `src/templates/`:
     - `src/templates/fetch.rs`
     - `src/templates/env/create.rs`
     - `src/templates/host/create.rs`
     - `src/templates/service/create.rs`
   - CLI command files live in `src/commands/` and are thin adapters:
     - `src/commands/init.rs`
     - `src/commands/new/env.rs`
     - `src/commands/new/host.rs`
     - `src/commands/new/service.rs`

## Codebase Searches and Lookups with Graphify

To save context tokens and avoid scanning raw files across the codebase:
- Use `graphify query "<question>"` for architecture lookups, cross-module relationships, and call chains.
- Use `graphify path "<source>" "<target>"` to find connections between two concepts or modules.
- Use `graphify explain "<node>"` for node summaries and connections.
- After adding or modifying files, update the knowledge graph with `graphify extract . --code-only`.
- Only read raw source files when modifying them or inspecting specific implementation details.

## Verification Before Claims

Before marking any task complete or committing changes:
- Run Rust checks: `cargo check` and `cargo test`.
- Run strict Nix checks: `nix flake check` or `nix-instantiate --eval --strict`.
- Never claim a command passed without running it and inspecting the output.

