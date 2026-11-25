# OurioPicker (`ouro-terminal-picker`)

Small in-terminal Zellij session picker that runs as a terminal's shell program (Kitty, Rio, Ourio).

When a terminal opens, `ouro-terminal-picker` lists existing Zellij sessions (running ones first, exited ones marked as resurrectable), offers creating a new named or unnamed session, or starting a plain shell. It also includes `install`, `uninstall`, `print-config`, and `completions` subcommands to wire Kitty and Rio/Ourio configurations safely and generate shell completions.

## Documentation

- [Zellij session list format](docs/wiki/zellij-session-list.md)
- [Terminal wiring (Kitty and Rio/Ourio)](docs/wiki/terminal-wiring.md)
- [Shell completions](docs/wiki/shell-completions.md)
- [Desktop launcher and AUR packaging](docs/wiki/aur-packaging.md)
- [PICK-002 plan (desktop launcher & AUR)](docs/plans/PICK-002-desktop-and-aur.md)
- [Feature matrix](FEATURES.md)
- [Current work](CURRENT_WORK.md)

## License

MIT — see [LICENSE](LICENSE).
