# CURRENT_WORK: OurioPicker

Updated: 2026-11-25. Standard and main index live in OuroHub (`CURRENT_WORK.md`, `work/README.md`).

## State
- **PICK-001** (`ouro-terminal-picker` v1) merged into `main` (OurioPicker#2, issue #1 closed, tag `v0.1.0`) and base Arch `PKGBUILD` merged (OurioPicker#4, issue #3 closed, `deeb657`); repository public (E252). Governing plan: OuroHub `plans/ouro-terminal-picker/PICK-001-session-picker.md`.
- **PICK-003** (`ouro-terminal-picker completions <shell>`, issue #6, PR #7): merged into `main` (`e889cbb`), using `clap_complete 4.6.12` (MIT OR Apache-2.0, zero extra transitive deps).
- **Binary allowlist:** `quality/binary-allow.txt` merged on `main` (`2b8d20e`) allowing the exact blob SHAs of both owner-accepted PNG icons (`ouro-terminal-picker.png` and `ouro-terminal-picker-dark.png`).
- **PICK-002** (issue `#5`, branch `feature/5-desktop-aur`, plan `docs/plans/PICK-002-desktop-and-aur.md`): **Ready for review (v0.1.1).** Merged `origin/main`, bundled both owner-accepted PNG icons verbatim, bumped crate version to `0.1.1` (syncing `Cargo.lock` offline on the workstation), unblocked `packaging/arch/PKGBUILD` (`pkgver=0.1.1`, `pkgrel=1`) to build from the single `v0.1.1` release tarball, generate Bash/Fish/Zsh completions from `./target/release/ouro-terminal-picker`, and install binary, `.desktop`, both pixmap icons, all three shell completions, LICENSE, README, and wiki docs. Regenerated `packaging/arch/.SRCINFO` via `makepkg --printsrcinfo`. Remaining post-merge step: tag `v0.1.1`, refresh the `sha512sums` placeholder and `.SRCINFO` to the published GitHub `v0.1.1.tar.gz` digest, and manually push `PKGBUILD` + `.SRCINFO` to AUR `master`.

## What exists
- Library crate `ouro_terminal_picker` (`src/lib.rs`, `unsafe_code = "forbid"`, edition 2024, `rust-version = "1.100"`, `rust-toolchain.toml` pinned to `1.100.0`, version `0.1.1`):
  - `src/name.rs`, `src/sessions.rs`, `src/launch.rs`, `src/picker.rs`, `src/config/{mod,kitty,rio}.rs`, `src/term.rs`.
- Binary `ouro-terminal-picker` (`src/main.rs`): picker mode, `install`, `uninstall`, `print-config`, and `completions <shell>` (`bash`, `fish`, `zsh`, plus standard Clap `elvish`/`powershell`).
- Integration & PTY suites (`tests/cli_and_launch.rs`, `tests/completions.rs`, `tests/pty_smoke.rs`) and wiki docs (`docs/wiki/zellij-session-list.md`, `docs/wiki/terminal-wiring.md`, `docs/wiki/shell-completions.md`, `docs/wiki/aur-packaging.md`).
- Packaging (`feature/5-desktop-aur`): `packaging/desktop/ouro-terminal-picker.desktop`, `packaging/icons/ouro-terminal-picker.png`, `packaging/icons/ouro-terminal-picker-dark.png`, `packaging/arch/PKGBUILD` (`0.1.1-1`), `packaging/arch/.SRCINFO`, `packaging/tests/verify-desktop-and-aur.sh`.

## Verification (PICK-002 v0.1.1 on `feature/5-desktop-aur`)
- Laptop (light checks only):
  - `desktop-file-validate --warn-kde packaging/desktop/ouro-terminal-picker.desktop`: PASS (exit 0, 0 warnings).
  - `packaging/tests/verify-desktop-and-aur.sh`: PASS (all hermetic checks green, incl. `.SRCINFO` byte-sync and icon blob SHA verification against `quality/binary-allow.txt`).
  - Hub scans (`leakguard`, `secrets`, `privacy`, `binary-files`): PASS on every commit/push.
- Workstation (1.100.0, `nice -n 10`, `CARGO_BUILD_JOBS=8`, isolated target dir):
  - `cargo +1.100.0 fmt --all --check`, `cargo +1.100.0 clippy --locked --all-targets --all-features -- -D warnings`, `cargo +1.100.0 test --locked --all-features`, `RUSTDOCFLAGS="-D warnings" cargo +1.100.0 doc --locked --no-deps`, `cargo-deny`, `cargo-machete`, release completion generation (`bash`, `fish`, `zsh`), and full hub `scripts/check.sh push` gate: verified on the workstation before push.

## Next steps
1. Fresh independent review of `feature/5-desktop-aur` (issue `#5`).
2. Merge into `main` (`Closes #5`) after review PASS.
3. Tag `v0.1.1`, update `packaging/arch/PKGBUILD` `sha512sums` from the published GitHub `v0.1.1.tar.gz` tarball (via `aur-update-sha.py`), regenerate `.SRCINFO`, run a clean local `makepkg` build, and manually upload `PKGBUILD` + `.SRCINFO` to AUR `master`.
