# Zellij session list format (`zellij list-sessions --no-formatting`)

This note documents the observed output of Zellij 0.46.0 (2026-11-25) and the parsing rules implemented in `ouro_terminal_picker::sessions`.

## Observed output (Zellij 0.46.0)

Single exited session (exit status `0`, stdout):

```text
work [Created 10days 7h 11m 22s ago] (EXITED - attach to resurrect)
```

Mixed running and exited sessions from an isolated test server (exit status `0`, stdout):

```text
work [Created 10days 7h 11m 4s ago] (EXITED - attach to resurrect)
d580883b86d4 [Created 8s ago]
```

Current active session (exit status `0`, stdout):

```text
dev [Created 2h 3m 1s ago] (current)
```

No sessions at all (verified under isolated `HOME`, `XDG_CACHE_HOME`, `XDG_CONFIG_HOME`, `XDG_RUNTIME_DIR`, and `ZELLIJ_SOCKET_DIR`):

- Exit status: `1`
- stdout: empty
- stderr: `No active zellij sessions found.`

Note: setting only `ZELLIJ_SOCKET_DIR` is not enough to isolate exited sessions because Zellij 0.46.0 also reads resurrection metadata from `$XDG_CACHE_HOME/zellij/`.

## Parsing rules

1. **Defensive ANSI stripping:** even though `--no-formatting` is passed, CSI (`ESC [ ...`) and OSC (`ESC ] ... BEL` / `ESC \`) sequences are stripped from stdout and stderr before parsing.
2. **Empty-list handling:** empty stdout on exit `0`, or `"No active zellij sessions found."` on stdout or stderr (including exit `1` with empty stdout), returns `Ok(Vec::new())` rather than an error.
3. **Trust boundary on names:** the first whitespace-separated token of each line is validated through `SessionName::new` (`1..=64` ASCII characters from `[A-Za-z0-9._-]`, not starting with `-` or `.`). Lines whose token fails validation are skipped safely; duplicate names keep the first occurrence.
4. **State classification:** inspecting the remainder of the line after the name:
   - contains `(EXITED` -> `SessionState::Exited`
   - contains `(current)` -> `SessionState::Current`
   - otherwise -> `SessionState::Running`
5. **Ordering:** active sessions (`Running` and `Current`) are placed before `Exited` sessions while preserving Zellij's relative order within each group.
6. **Launch argv:** session names are never passed through a shell:
   - attach / resurrect: `zellij attach <name> options --on-force-close detach`
   - create named: `zellij attach --create <name> options --on-force-close detach`
   - create unnamed: `zellij options --on-force-close detach`
