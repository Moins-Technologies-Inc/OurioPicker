# OurioPicker feature matrix

Status values: Planned, In progress, Review, Done.

## v1

| ID | Feature | Decisions | Release | Status | Acceptance (summary; full list in the plan) | Target tests |
|---|---|---|---|---|---|---|
| PICK-001 | In-terminal Zellij session picker (`ouro-terminal-picker`) with Kitty and Rio/Ourio wiring | E250–E256 | v1 | Done | Lists running and exited Zellij sessions, creates named/unnamed sessions, falls back to shell when nested/missing/non-TTY/non-zero exit; restores terminal on every exit including panic; idempotent atomic preview-first `install`/`uninstall`/`print-config` for Kitty and Rio/Ourio refusing symlinks and foreign `[shell]`; full 1.100.0 gate green | session parsing (0.46 fixture), session name validation, argv construction, picker state machine, shell resolution, fake-zellij integration, Kitty/Rio golden byte & parser tests, PTY smoke test |
| PICK-002 | Freedesktop desktop launcher (`.desktop`), icon integration and AUR packaging (`PKGBUILD` / `.SRCINFO`) | OurioPicker#5 | v1.x | In progress | Standards-based `Terminal=true` `.desktop` entry validated by `desktop-file-validate --warn-kde`; immutable-source AUR `PKGBUILD` + `.SRCINFO` with real `sha512sums` (no `SKIP`); manual AUR upload docs; **blocked** pending owner-approved transparent PNG icon and parallel `clap_complete` Bash/Fish/Zsh shell completions | `packaging/tests/verify-desktop-and-aur.sh` (11 hermetic checks), `desktop-file-validate --warn-kde`, `makepkg --printsrcinfo`, `makepkg --verifysource` |
