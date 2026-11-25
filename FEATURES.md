# OurioPicker feature matrix

Status values: Planned, In progress, Review, Done.

## v1

| ID | Feature | Decisions | Release | Status | Acceptance (summary; full list in the plan) | Target tests |
|---|---|---|---|---|---|---|
| PICK-001 | In-terminal Zellij session picker (`ouro-terminal-picker`) with Kitty and Rio/Ourio wiring | E250–E256 | v1 | Done | Lists running and exited Zellij sessions, creates named/unnamed sessions, falls back to shell when nested/missing/non-TTY/non-zero exit; restores terminal on every exit including panic; idempotent atomic preview-first `install`/`uninstall`/`print-config` for Kitty and Rio/Ourio refusing symlinks and foreign `[shell]`; full 1.100.0 gate green | session parsing (0.46 fixture), session name validation, argv construction, picker state machine, shell resolution, fake-zellij integration, Kitty/Rio golden byte & parser tests, PTY smoke test |
| PICK-002 | Shell completions subcommand (`ouro-terminal-picker completions <shell>`) via `clap_complete` | Owner request (#6) | v1 | Review | Generates completion scripts derived from live Clap CLI (`CommandFactory`) for `bash`, `fish`, `zsh` (plus standard `elvish`/`powershell`); pure stdout before tracing/shell/TTY/ZELLIJ/config I/O; non-zero clean exit on invalid shell or broken stdout without panicking; zero file or subprocess side effects | `completions` integration tests (help, bash/fish/zsh/extra generation, `bash -n`, `fish --no-execute`, pre-tracing isolation, invalid/missing shell, broken stdout, fake shell/zellij/config side-effect guards) + `src/main.rs` unit tests |
