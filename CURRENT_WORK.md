# CURRENT_WORK: OurioPicker

Updated: 2026-11-25. Standard and main index live in OuroHub (`CURRENT_WORK.md`, `work/README.md`).

## State
- PICK-001 (`ouro-terminal-picker` v1) merged into `main` (OurioPicker#2, issue #1 closed) after review round 1 FAIL and a delta review PASS bound to `a24fd4f`; repository public (E252). Arch package (`packaging/arch/PKGBUILD`) merged in OurioPicker#4 (issue #3 closed). Governing plan: OuroHub `plans/ouro-terminal-picker/PICK-001-session-picker.md`.
- PICK-002 (`ouro-terminal-picker completions <shell>`, issue #6, branch `feature/6-completions`): implemented with `clap_complete 4.6.12` (MIT OR Apache-2.0, resolved under the 7-day Cargo cooldown with zero extra transitive dependencies), ready for independent review.

## What exists
- Library crate `ouro_terminal_picker` (`src/lib.rs`, `unsafe_code = "forbid"`, edition 2024, `rust-version = "1.100"`, `rust-toolchain.toml` pinned to `1.100.0`):
  - `src/name.rs`: `MAX_SESSION_NAME_LEN`, `SessionName`, `NameError` (`1..=64` ASCII `[A-Za-z0-9._-]`, no leading `-` or `.`).
  - `src/sessions.rs`: `Session`, `SessionState`, `SessionListError`, `strip_ansi`, `parse_sessions`, `list_sessions`, `list_sessions_with` (handles Zellij 0.46.0 empty-list exit `1` on stderr `"No active zellij sessions found."`, running/current ordered before exited, `list_sessions` tracing span).
  - `src/launch.rs`: `FALLBACK_SHELL`, `Choice`, `LaunchError`, `zellij_command`, `zellij_command_with`, `is_executable_file`, `find_in_path`, `should_bypass_picker`, `parse_passwd_shell`, `resolve_shell_with`, `resolve_shell_with_exe`, `resolve_shell` (skips candidates canonicalizing to `current_exe()`), `exec_shell`, `run_choice` (falls back to shell with notice and warn log on both spawn failure and non-zero exit).
  - `src/picker.rs`: pure state machine `Picker`, `Key`, `Mode`, `Step`, and `render_lines` (fits `20x5` and wider, text-only markers, scrolling viewport).
  - `src/config/{mod,kitty,rio}.rs`: `TerminalKind`, `ConfigError` (incl. `ParentTraversal`, `MissingParent`, `InvalidUtf8`), `FileOp`, `ConfigPlan` (LCS line diff for `--dry-run`), atomic mode-preserving `apply_plan` (`file.set_permissions` on open fd, writability re-check), symlink-parent/target/traversal/read-only refusal, Kitty strict-UTF-8 two-line marked include block, Rio/Ourio `toml_edit` format-preserving edits with `--force` `0600` backups.
  - `src/term.rs`: `TerminalError`, `TerminalGuard` (public RAII guard), `map_key_event`, `is_interactive_tty`, `no_color_enabled`, `restore_terminal`, `install_panic_hook`, `run_tui`.
- Binary `ouro-terminal-picker` (`src/main.rs`): picker mode (with panic `catch_unwind` + shell fallback, `run_tui` error warn logging, and `LogSink` switching to direct stderr pass-through after terminal restore before any `exec`), `install`, `uninstall`, `print-config`, and `completions <shell>` (`clap_complete::aot::{Generator, Shell}` via `Cli::command()` + `try_generate`, executed before tracing initialization, shell resolution, TTY/nested `ZELLIJ` guards, and any config I/O).
- Integration & PTY suites (`tests/cli_and_launch.rs`, `tests/completions.rs`, `tests/pty_smoke.rs`) and wiki docs (`docs/wiki/zellij-session-list.md`, `docs/wiki/terminal-wiring.md`, `docs/wiki/shell-completions.md`).

## Round-2 review fixes applied (F1–F9)
- **F1:** `run_choice` catches Zellij spawn errors, logs `warn` (`"zellij fallback"`), prints `"Could not start Zellij (<reason>); opening your shell."`, and execs the shell.
- **F2:** `LogSink` buffers only while the TUI is active and switches to direct stderr pass-through immediately after terminal restoration (before `exec_shell`); `run_tui` errors are logged at `warn` instead of silently dropped.
- **F3:** Moved nested-`ZELLIJ` and missing-`zellij` bypass tests into the real PTY driver (asserting no `\x1b[?1049h` and shell execution); added `stty -a` cooked-mode (`icanon` + `echo`) checks in fake `zellij` and fake `SHELL`; added real-PTY `TerminalGuard` panic-unwind + panic-hook restoration test.
- **F4:** `ensure_no_symlink_ancestors` rejects `Component::ParentDir` (`ConfigError::ParentTraversal`) across the full path.
- **F5:** `atomic_write` uses `ConfigError::MissingParent`, re-checks target writability (`ConfigError::NotWritable`), and sets permissions on the open `File` handle (`file.set_permissions`).
- **F6:** Kitty planner rejects invalid UTF-8 in `kitty.conf` cleanly (`ConfigError::InvalidUtf8`) instead of `from_utf8_lossy`.
- **F7:** Added runnable `# Examples` doctests to `NameError` and all public constants (`MAX_SESSION_NAME_LEN`, `FALLBACK_SHELL`, `INCLUDE_FILE_NAME`, `MARKER_LINE`, `INCLUDE_LINE`).
- **F8:** `resolve_shell` skips `$SHELL` / `/etc/passwd` entries canonicalizing to `current_exe()`; dropped unused derives (`PartialOrd`/`Ord` on `SessionName`, `Clone` on `Picker`, `Hash` on `SessionState`/`Key`/`Mode`/`TerminalKind`).
- **F9:** `ConfigPlan::format_dry_run` computes a real LCS line diff so added duplicate lines are shown.

## Deviations from the plan
1. **Kitty marker placement:** Kitty's parser (`kitty.config.load_config`) rejects inline `#` comments on `include` lines. Implemented a two-line marked block (`# ouro-terminal-picker\ninclude ouro-terminal-picker.conf`) validated directly against Kitty's parser (`kitty +runpy`) with zero bad lines.
2. **`zellij_command` return type:** returns `Option<Command>` (`None` for `Choice::Shell`) instead of `Command`, keeping the function pure, total, and panic-free.
3. **Zellij 0.46.0 empty-list exit status:** `zellij list-sessions --no-formatting` exits `1` with `"No active zellij sessions found."` on stderr (empty stdout) when no sessions exist; `list_sessions_with` treats that exact case as `Ok(Vec::new())`.
4. **PTY smoke test driver:** uses `/usr/bin/python3 -I` stdlib `pty.openpty()` (zero `unsafe`, zero extra dev-dependencies) instead of `script -qec`.
5. **Missing `AGENTS.md` on `main`:** writing `AGENTS.md` is blocked by the harness's protected-file guard and left for the owner/orchestrator.

## Verification
- Workstation (1.100.0, `nice -n 10`, `CARGO_BUILD_JOBS=8`, dedicated `CARGO_TARGET_DIR`):
  - `cargo fmt --all --check`: PASS
  - `cargo check --locked --all-targets --all-features`: PASS
  - `cargo clippy --locked --all-targets --all-features -- -D warnings`: PASS (0 warnings)
  - `RUSTDOCFLAGS="-D warnings" cargo doc --locked --no-deps`: PASS (0 warnings)
  - `cargo test --locked`: 51 lib unit + 2 bin unit + 3 `cli_and_launch` + 8 `completions` + 10 `pty_smoke` + 77 doctests = 151 passed, 0 failed, 0 ignored
  - `cargo deny check` (`0.20.2`): PASS (`advisories ok, bans ok, licenses ok, sources ok`)
  - `cargo machete` (`0.9.2`): PASS (0 unused dependencies)
  - Completion syntax checks on built binary: `bash -n` PASS (228 lines), `fish --no-execute` PASS (51 lines), `zsh -n` SKIPPED (`zsh` not installed on workstation; generated 175 lines starting with `#compdef ouro-terminal-picker`)
  - Hub `scripts/check.sh push` gate on workstation clone: PASS

## Next step
1. Fresh independent review of `feature/6-completions` (issue #6).
2. Packaging stream updates `packaging/arch/PKGBUILD` / `.SRCINFO` to generate and install bash/fish/zsh completions from the freshly built binary.
3. Merge `feature/6-completions` into `main` (`Closes #6`) after review PASS.
