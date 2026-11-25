# CURRENT_WORK: OurioPicker

Updated: 2026-11-25. Standard and main index live in OuroHub (`CURRENT_WORK.md`, `work/README.md`).

## State
- **PICK-001** (`ouro-terminal-picker` v1) merged into `main` (OurioPicker#2, issue #1 closed, tag `v0.1.0`) and base Arch `PKGBUILD` merged (OurioPicker#4, issue #3 closed, `deeb657`); repository public (E252). Governing plan: OuroHub `plans/ouro-terminal-picker/PICK-001-session-picker.md`.
- **PICK-002** (issue `#5`, branch `feature/5-desktop-aur`, plan `docs/plans/PICK-002-desktop-and-aur.md`): **In progress — blocked on icon asset and pending `clap_complete` shell completions.**
  - Prepared and validated: `packaging/desktop/ouro-terminal-picker.desktop` (`desktop-file-validate --warn-kde` PASS), updated `packaging/arch/PKGBUILD` (`pkgrel=2`, pinned immutable sources with real `sha512sums`, fail-closed blocker guard in `prepare()`), `packaging/arch/.SRCINFO` (`makepkg --printsrcinfo`), `packaging/tests/verify-desktop-and-aur.sh` (11 hermetic checks, TDD RED -> GREEN), and `docs/wiki/aur-packaging.md`.
  - **Blocker 1 (icon):** awaiting an owner-approved transparent PNG icon (two opaque RGB 1254x1254 generation attempts rejected; no fake icon, SVG, nonexistent URL, or unauthorized generic fallback shipped).
  - **Blocker 2 (completions):** `clap_complete` Bash/Fish/Zsh completion generation is being implemented in parallel; `PKGBUILD` documents the exact Arch target paths and must generate completions from the freshly built release binary (never a host-installed binary).
  - Recommended next step once both blockers clear: cut a new release tag (e.g. `v0.1.1` or `v0.2.0`) bundling the desktop entry, approved transparent PNG icon, and `clap_complete` support, update `PKGBUILD` to that release tarball, remove the `prepare()` blocker, run a clean `makepkg` build, regenerate `.SRCINFO`, obtain fresh independent review PASS, and hand the final `PKGBUILD` + `.SRCINFO` to the owner for manual AUR upload.

## What exists
- Library crate `ouro_terminal_picker` (`src/lib.rs`, `unsafe_code = "forbid"`, edition 2024, `rust-version = "1.100"`, `rust-toolchain.toml` pinned to `1.100.0`):
  - `src/name.rs`: `MAX_SESSION_NAME_LEN`, `SessionName`, `NameError` (`1..=64` ASCII `[A-Za-z0-9._-]`, no leading `-` or `.`).
  - `src/sessions.rs`: `Session`, `SessionState`, `SessionListError`, `strip_ansi`, `parse_sessions`, `list_sessions`, `list_sessions_with` (handles Zellij 0.46.0 empty-list exit `1` on stderr `"No active zellij sessions found."`, running/current ordered before exited, `list_sessions` tracing span).
  - `src/launch.rs`: `FALLBACK_SHELL`, `Choice`, `LaunchError`, `zellij_command`, `zellij_command_with`, `is_executable_file`, `find_in_path`, `should_bypass_picker`, `parse_passwd_shell`, `resolve_shell_with`, `resolve_shell_with_exe`, `resolve_shell` (skips candidates canonicalizing to `current_exe()`), `exec_shell`, `run_choice` (falls back to shell with notice and warn log on both spawn failure and non-zero exit).
  - `src/picker.rs`: pure state machine `Picker`, `Key`, `Mode`, `Step`, and `render_lines` (fits `20x5` and wider, text-only markers, scrolling viewport).
  - `src/config/{mod,kitty,rio}.rs`: `TerminalKind`, `ConfigError` (incl. `ParentTraversal`, `MissingParent`, `InvalidUtf8`), `FileOp`, `ConfigPlan` (LCS line diff for `--dry-run`), atomic mode-preserving `apply_plan` (`file.set_permissions` on open fd, writability re-check), symlink-parent/target/traversal/read-only refusal, Kitty strict-UTF-8 two-line marked include block, Rio/Ourio `toml_edit` format-preserving edits with `--force` `0600` backups.
  - `src/term.rs`: `TerminalError`, `TerminalGuard` (public RAII guard), `map_key_event`, `is_interactive_tty`, `no_color_enabled`, `restore_terminal`, `install_panic_hook`, `run_tui`.
- Binary `ouro-terminal-picker` (`src/main.rs`): picker mode (with panic `catch_unwind` + shell fallback, `run_tui` error warn logging, and `LogSink` switching to direct stderr pass-through after terminal restore before any `exec`) plus `install`, `uninstall`, and `print-config` subcommands.
- Integration & PTY suites (`tests/cli_and_launch.rs`, `tests/pty_smoke.rs`) and wiki docs (`docs/wiki/zellij-session-list.md`, `docs/wiki/terminal-wiring.md`, `docs/wiki/aur-packaging.md`).
- Packaging (`feature/5-desktop-aur`): `packaging/desktop/ouro-terminal-picker.desktop`, `packaging/arch/PKGBUILD` (blocked on icon + completions), `packaging/arch/.SRCINFO`, `packaging/tests/verify-desktop-and-aur.sh`.

## Verification (PICK-002 on `feature/5-desktop-aur`)
- `desktop-file-validate --warn-kde packaging/desktop/ouro-terminal-picker.desktop`: PASS (exit 0, 0 warnings).
- `bash -n packaging/arch/PKGBUILD` and `bash -n packaging/tests/verify-desktop-and-aur.sh`: PASS.
- `makepkg --printsrcinfo > packaging/arch/.SRCINFO`: generated cleanly and verified byte-identical.
- `makepkg --verifysource` (scratch copy): verifies both `v0.1.0.tar.gz` and the pinned raw `.desktop` URL against their `sha512sums`.
- `packaging/tests/verify-desktop-and-aur.sh`: RED (3 passed, 12 failed) before implementation -> GREEN (11 passed, 0 failed) after implementation.
- Hub hooks (`pre-commit` fast tier + `pre-push` push tier): all steps PASS on `feature/5-desktop-aur`.
- AUR RPC check (`https://aur.archlinux.org/rpc/v5/info?arg[]=ouro-terminal-picker&arg[]=ourio-picker`): 0 results (no existing package conflict).

## Next steps
1. Owner supplies an approved transparent PNG icon (or authorizes a generic icon-theme fallback).
2. Parallel worker merges `clap_complete` Bash/Fish/Zsh completion support.
3. Cut a new release tag (or re-pin static assets if completions are deferred), remove the `prepare()` blocker in `packaging/arch/PKGBUILD`, add icon + completion install lines, regenerate `.SRCINFO`, run a clean local `makepkg` build, and pass a fresh independent review before merging to `main` (`Closes #5`).
4. Owner manually clones `ssh://aur@aur.archlinux.org/ouro-terminal-picker.git`, copies only `PKGBUILD` + `.SRCINFO`, commits on `master`, and pushes under their own AUR identity.
