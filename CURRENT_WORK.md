# CURRENT_WORK: OurioPicker

Updated: 2026-11-25. Standard and main index live in OuroHub (`CURRENT_WORK.md`, `work/README.md`).

## State
PICK-001 (`ouro-terminal-picker` v1) implemented test-first on branch `feature/1-session-picker` (issue #1, OuroHub issue #159). Governing plan: OuroHub `plans/ouro-terminal-picker/PICK-001-session-picker.md`. Status in `FEATURES.md`: `Review`.

## What exists
- Library crate `ouro_terminal_picker` (`src/lib.rs`, `unsafe_code = "forbid"`, edition 2024, `rust-version = "1.100"`, `rust-toolchain.toml` pinned to `1.100.0`):
  - `src/name.rs`: `SessionName`, `NameError` (`1..=64` ASCII `[A-Za-z0-9._-]`, no leading `-` or `.`).
  - `src/sessions.rs`: `Session`, `SessionState`, `SessionListError`, `strip_ansi`, `parse_sessions`, `list_sessions`, `list_sessions_with` (handles Zellij 0.46.0 empty-list exit `1` on stderr `"No active zellij sessions found."`, running/current ordered before exited, `list_sessions` tracing span).
  - `src/launch.rs`: `Choice`, `LaunchError`, `zellij_command`, `zellij_command_with`, `is_executable_file`, `find_in_path`, `should_bypass_picker`, `parse_passwd_shell`, `resolve_shell_with`, `resolve_shell`, `exec_shell`, `run_choice`.
  - `src/picker.rs`: pure state machine `Picker`, `Key`, `Mode`, `Step`, and `render_lines` (fits `20x5` and wider, text-only markers, scrolling viewport).
  - `src/config/{mod,kitty,rio}.rs`: `TerminalKind`, `ConfigError`, `FileOp`, `ConfigPlan`, atomic mode-preserving `apply_plan`, symlink-parent/target/read-only refusal, Kitty two-line marked include block, Rio/Ourio `toml_edit` format-preserving edits with `--force` `0600` backups.
  - `src/term.rs`: `TerminalError`, `map_key_event`, `is_interactive_tty`, `no_color_enabled`, `restore_terminal`, `install_panic_hook`, `run_tui` (`TerminalGuard` RAII restoration).
- Binary `ouro-terminal-picker` (`src/main.rs`): picker mode (with panic `catch_unwind` + shell fallback and deferred stderr logging gated by `OURO_TERMINAL_PICKER_LOG`) plus `install`, `uninstall`, and `print-config` subcommands.
- Integration & PTY suites (`tests/cli_and_launch.rs`, `tests/pty_smoke.rs`) and wiki docs (`docs/wiki/zellij-session-list.md`, `docs/wiki/terminal-wiring.md`).

## Deviations from the plan
1. **Kitty marker placement:** Kitty's parser (`kitty.config.load_config`) rejects inline `#` comments on `include` lines (treating `# ouro-terminal-picker` as part of the included filename and logging `Could not find included config file`). Implemented a two-line marked block (`# ouro-terminal-picker\ninclude ouro-terminal-picker.conf`) validated directly against Kitty's parser (`kitty +runpy`) with zero bad lines.
2. **`zellij_command` return type:** returns `Option<Command>` (`None` for `Choice::Shell`) instead of `Command`, keeping the function pure, total, and panic-free without hidden shell resolution.
3. **Zellij 0.46.0 empty-list exit status:** observed on the workstation under isolated `HOME`/`XDG_*`/`ZELLIJ_SOCKET_DIR` that `zellij list-sessions --no-formatting` exits `1` with `"No active zellij sessions found."` on stderr (empty stdout); `list_sessions_with` treats that exact case as `Ok(Vec::new())`.
4. **PTY smoke test driver:** uses `/usr/bin/python3 -I` stdlib `pty.openpty()` (zero `unsafe`, zero extra dev-dependencies) instead of `script -qec`, because `util-linux`'s `script` executes `$SHELL` directly (conflicting with fake `$SHELL` tests) and gives deterministic screen-readiness + alternate-screen exit assertions.
5. **Missing `AGENTS.md` on `main`:** writing `AGENTS.md` was blocked by the agent harness's protected-file guard; per hub rules (refused protected-file writes go back to the owner), it was not bypassed and should be added by the owner/orchestrator.

## Verification
- Workstation (1.100.0, `nice -n 10`, `CARGO_BUILD_JOBS=16`):
  - `cargo fmt --all --check`: PASS
  - `cargo clippy --all-targets --all-features -- -D warnings`: PASS (0 warnings)
  - `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps`: PASS (0 warnings)
  - `cargo test`: 46 lib unit + 0 bin unit + 5 `cli_and_launch` integration (incl. `kitty +runpy` validation) + 5 `pty_smoke` + 68 doctests = 124 passed, 0 failed, 0 ignored
  - Hub `scripts/check.sh push` gate on workstation clone: PASS
- Laptop (light checks):
  - `cargo-deny check`: PASS (0 warnings; MIT / Apache-2.0 / Unicode-3.0 only)
  - `cargo-machete`: PASS (0 unused dependencies)
  - Hub hooks on every commit/push: `leakguard`, `secrets`, `privacy`, `binary-files` PASS

## Next step
1. Owner/orchestrator adds `AGENTS.md` to `main` (blocked from subagent write).
2. Fresh independent reviewer reviews `feature/1-session-picker` against `PICK-001-session-picker.md` and `REVIEW.md`.
3. Manual smoke check on the laptop (acceptance criterion 1) and merge into `main` (`Closes #1`) after review PASS.
