# OurioPicker: agent instructions

`ouro-terminal-picker`: pick a Zellij session, start a new one or open a plain shell when a terminal opens. A module of Ourio, the Ouro terminal (a fork of the Rio terminal).

Full rules: OuroHub [AGENTS.md](https://github.com/Moins-Technologies-Inc/OuroHub/blob/main/AGENTS.md) (complete standards, workflow, hooks, build host, MCP servers, privacy). When this file and the hub differ, the stricter one wins. Owner decisions cited by ID (E250 to E256, D2 ...) are in the hub's `plans/OWNER-QUESTIONS.md` and `plans/REQUIREMENTS.md`.

**Start here:** read `CURRENT_WORK.md`, then the governing plan in the hub: `plans/ouro-terminal-picker/PICK-001-session-picker.md`. State: v1 merged and public (PICK-001). Next: the Arch package and the private workstation manager's switch to the binary (hub issue #159).

## Standards

- Public repository: no machine names, user names, home paths, hostnames or addresses in any committed file; MIT licence.
- Rust toolchain pinned exactly: `rust-toolchain.toml` with `channel = "1.100.0"`; `rust-version = "1.100"`, edition 2024.
- Clippy `-D warnings` plus `unwrap_used`, `expect_used`, `panic`, `todo`, `dbg_macro`, `indexing_slicing` in non-test code; `missing_docs` denied; `unsafe_code = "forbid"` (no exceptions).
- `[lints.cargo] unused_dependencies = "deny"`; `.cargo/config.toml` keeps `[registry] global-min-publish-age = "7 days"`; never override the cooldown. `cargo-deny` and `cargo-machete` stay clean.
- Hard cap of 800 lines per new or changed file; split first, then edit.
- Typed error enums (`thiserror`) in the library, `anyhow` in the binary; `tracing` with structured fields, never session names or other user content.
- Synchronous code; no async runtime. Session names reach Zellij only as validated argv, never through a shell.
- Configuration writes stay preview-first, atomic, mode-preserving and refuse symlinked files or parents.
- TDD: failing test first, RED run kept. A fresh reviewer for every change.

## Workflow

- Issue first (labelled: one type label, one priority label), then a reviewed plan in the hub (`plans/ouro-terminal-picker/<ID>-<slug>.md`) and a row in `FEATURES.md`.
- Work in a separate worktree on `<type>/<issue>-<slug>`; never commit feature work on `main`. Checkpoint commits and pushes at every sub-step, with `CURRENT_WORK.md` updated.
- Builds and test suites run on the build workstation, never on the laptop. Tests use a fake `zellij` on `PATH` or a private `ZELLIJ_SOCKET_DIR`; never the user's real sessions or terminal configuration.
- Merge into `main` only after review PASS. Commit as `Fakhri Daniel Hammami <daniel@moins.tech>`; `git pull --no-rebase` before pushing.

## Hooks

- Hub tier: inside the hub, `git config core.hooksPath "$(git rev-parse --show-superproject-working-tree)/scripts/hooks"`; cloned alone, clone the hub next to it and `git config core.hooksPath <hub checkout>/scripts/hooks`. Never `--no-verify`.
