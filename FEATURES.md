# OurioPicker feature matrix

Status values: Planned, In progress, Review, Done.

## v1

| ID | Feature | Decisions | Release | Status | Acceptance (summary; full list in the plan) | Target tests |
|---|---|---|---|---|---|---|
| PICK-001 | In-terminal Zellij session picker (`ouro-terminal-picker`) with Kitty and Rio/Ourio wiring | E246–E252 | v1 | Review | Lists running and exited Zellij sessions, creates named/unnamed sessions, falls back to shell when nested/missing/non-TTY/non-zero exit; restores terminal on every exit including panic; idempotent atomic preview-first `install`/`uninstall`/`print-config` for Kitty and Rio/Ourio refusing symlinks and foreign `[shell]`; full 1.100.0 gate green | session parsing (0.46 fixture), session name validation, argv construction, picker state machine, shell resolution, fake-zellij integration, Kitty/Rio golden byte & parser tests, PTY smoke test |
