<!-- GSD:project-start source:PROJECT.md -->

## Project

**Ben**

Ben is a declarative NixOS deployment framework and Rust-powered CLI that enables Kubernetes-style service placement and cryptographically-scoped secrets for small-to-mid NixOS fleets — evaluated purely at build time with zero runtime control plane overhead. It formalizes and generalizes the production-proven architecture developed across `gaur-deployments` and `fincher/deploy` into an open-source, reusable framework.

**Core Value:** Declarative, compile-time service placement (resolving co-located services to loopback and distributed services to private mesh) combined with topology-derived cryptographic secret scoping, executed through an atomic, resumable bootstrap-reconciliation loop.

### Constraints

- **Tech Stack**: Rust for the CLI tool (packaged via Cargo / Crane in Nix), pure Nix for `lib/` and `modules/`.
- **Pure Evaluation**: The core `mkFleet` engine must evaluate cleanly without undeclared impure path reads, avoiding infinite recursion risks by keeping placement logic data-driven over `fleet.nix`.
- **Decoupled Architecture**: Engine must not hardcode opinions on specific services (Postgres, NATS, etc.) or mesh providers (Tailscale is a pluggable resolver, not a hardcoded mandate).
- **Hermetic CLI**: The Rust CLI must build and run as a standalone binary without needing external Node/Bun or ad-hoc scripting environments.

<!-- GSD:project-end -->

<!-- GSD:stack-start source:STACK.md -->

## Technology Stack

Technology stack not yet documented. Will populate after codebase mapping or first phase.
<!-- GSD:stack-end -->

<!-- GSD:conventions-start source:CONVENTIONS.md -->

## Conventions

Conventions not yet established. Will populate as patterns emerge during development.
<!-- GSD:conventions-end -->

<!-- GSD:architecture-start source:ARCHITECTURE.md -->

## Architecture

Architecture not yet mapped. Follow existing patterns found in the codebase.
<!-- GSD:architecture-end -->

<!-- GSD:skills-start source:skills/ -->

## Project Skills

No project skills found. Add skills to any of: `.agents/skills/`, `.agents/skills/`, `.cursor/skills/`, `.github/skills/`, or `.codex/skills/` with a `SKILL.md` index file.
<!-- GSD:skills-end -->

<!-- GSD:workflow-start source:GSD defaults -->

## GSD Workflow Enforcement

Before using Edit, Write, or other file-changing tools, start work through a GSD command so planning artifacts and execution context stay in sync.

Use these entry points:

- `/gsd-quick` for small fixes, doc updates, and ad-hoc tasks
- `/gsd-debug` for investigation and bug fixing
- `/gsd-execute-phase` for planned phase work

Do not make direct repo edits outside a GSD workflow unless the user explicitly asks to bypass it.
<!-- GSD:workflow-end -->

<!-- GSD:profile-start -->

## Developer Profile

> Profile not yet configured. Run `/gsd-profile-user` to generate your developer profile.
> This section is managed by `generate-claude-profile` -- do not edit manually.
<!-- GSD:profile-end -->
