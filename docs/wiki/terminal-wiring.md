# Terminal wiring (Kitty and Rio / Ourio)

`ouro-terminal-picker` runs as the terminal emulator's configured shell program. The `install`, `uninstall`, and `print-config` subcommands manage wiring for Kitty and Rio/Ourio with preview-first (`--dry-run`), atomic writes (same-directory temp file, `sync_all`, mode preservation, `rename`), and strict symlink refusal.

## Kitty

- **Config directory:** `$XDG_CONFIG_HOME/kitty`, else `$HOME/.config/kitty` (absolute paths only).
- **Managed include file:** `<kitty_dir>/ouro-terminal-picker.conf`:

```conf
# Managed by ouro-terminal-picker
shell /absolute/path/to/ouro-terminal-picker
```

- **Main config (`kitty.conf`):** appends a two-line marked block when absent:

```conf
# ouro-terminal-picker
include ouro-terminal-picker.conf
```

### Why the marker is on its own line (not inline)

Kitty's config parser (`kitty.config.load_config`) treats `#` as a comment only at the start of a line. An inline comment on `include` (such as `include ouro-terminal-picker.conf  # ouro-terminal-picker`) is parsed as part of the included filename, causing Kitty to log `Could not find included config file` and ignore the include. Placing `# ouro-terminal-picker` on the preceding line validates cleanly with zero bad lines under `kitty +runpy`.

### Why symlinked or read-only `kitty.conf` is refused

Workstation configuration managers (such as Nix/Home Manager or dotfile managers) frequently manage `kitty.conf` as a symlink or read-only file. Editing through a symlink could mutate a tracked source repository or break declarative ownership, and following a symlinked parent directory is a path-traversal hazard. When `kitty.conf`, `ouro-terminal-picker.conf`, or any ancestor directory is a symlink (checked via `lstat`), or when the file is not writable, `ouro-terminal-picker` refuses to write and prints the exact lines to add manually or in an overlay.

### Uninstall

`uninstall --terminal kitty` removes only the two-line `# ouro-terminal-picker` + `include ouro-terminal-picker.conf` block from `kitty.conf` (preserving all other lines and manual edits made after install) and deletes `ouro-terminal-picker.conf`.

## Rio / Ourio

- **Config path (per `rio.5`):** `$RIO_CONFIG_HOME/config.toml`, else `$XDG_CONFIG_HOME/rio/config.toml`, else `$HOME/.config/rio/config.toml` (absolute paths only). Ourio reads the same file.
- **Format-preserving edits:** edits use `toml_edit` so existing comments, whitespace, and unrelated tables remain intact.
- **Managed table:**

```toml
[shell]
program = "/absolute/path/to/ouro-terminal-picker"
args = []
```

- **Foreign `[shell]` protection:** if `[shell]` already exists with a different program or non-empty args, `install --terminal rio` refuses unless `--force` is passed. With `--force`, the previous file bytes are saved atomically next to `config.toml` as `config.toml.ouro-terminal-picker.bak.<unix-timestamp>` with mode `0600` before updating `[shell]` in place.
- **Uninstall:** `uninstall --terminal rio` removes `[shell]` only if `program` still equals the current binary path and `args` is empty (or absent); manually edited `[shell]` tables are left untouched.
