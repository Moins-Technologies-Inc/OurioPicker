# Shell completions (`ouro-terminal-picker completions`)

`ouro-terminal-picker completions <shell>` generates shell completion scripts directly from the live Clap CLI definition using `clap_complete` (`clap_complete::aot::Generator::try_generate`).

## Supported shells

- Primary Linux shells: `bash`, `fish`, `zsh`
- Additional `clap_complete::aot::Shell` variants exposed by Clap: `elvish`, `powershell` (no non-Linux runtime support is claimed)

Completions include all subcommands (`install`, `uninstall`, `print-config`, `completions`), their flags (`--terminal`, `--dry-run`, `--force`), and the `--terminal` values (`kitty`, `rio`) automatically.

## Runtime guarantees

- **Pure stdout:** stdout contains only the generated completion script; stderr stays empty on success.
- **Early execution:** the subcommand runs before tracing initialization, shell resolution, TTY/nested `ZELLIJ` picker guards, and any user-configuration I/O. It works cleanly in non-TTY and nested `ZELLIJ` environments without running a shell or `zellij`, and never modifies files.
- **Clean errors:** unknown or missing shell names exit non-zero without side effects; stdout write errors (such as a broken pipe) exit non-zero cleanly without panicking.

## Generating completions during packaging

From a freshly built binary (for example `target/release/ouro-terminal-picker`):

```sh
./target/release/ouro-terminal-picker completions bash > ouro-terminal-picker.bash
./target/release/ouro-terminal-picker completions fish > ouro-terminal-picker.fish
./target/release/ouro-terminal-picker completions zsh  > _ouro-terminal-picker
```

Standard Linux / Arch package install paths:

| Shell | Standard install path |
|---|---|
| Bash | `/usr/share/bash-completion/completions/ouro-terminal-picker` |
| Fish | `/usr/share/fish/vendor_completions.d/ouro-terminal-picker.fish` |
| Zsh | `/usr/share/zsh/site-functions/_ouro-terminal-picker` |

## Per-user installation

```sh
# Bash
mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion/completions"
ouro-terminal-picker completions bash > "${XDG_DATA_HOME:-$HOME/.local/share}/bash-completion/completions/ouro-terminal-picker"

# Fish
mkdir -p "${XDG_CONFIG_HOME:-$HOME/.config}/fish/completions"
ouro-terminal-picker completions fish > "${XDG_CONFIG_HOME:-$HOME/.config}/fish/completions/ouro-terminal-picker.fish"

# Zsh (ensure the directory is on $fpath before compinit)
mkdir -p "${XDG_DATA_HOME:-$HOME/.local/share}/zsh/site-functions"
ouro-terminal-picker completions zsh > "${XDG_DATA_HOME:-$HOME/.local/share}/zsh/site-functions/_ouro-terminal-picker"
```
