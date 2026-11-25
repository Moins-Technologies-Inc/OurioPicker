# Desktop Launcher, Icons and AUR Packaging (`ouro-terminal-picker`)

This page documents the Freedesktop desktop launcher, bundled application icons, shell completion packaging, and manual AUR submission workflow for `ouro-terminal-picker` (OurioPicker).

Authoritative references:

- [Arch Wiki — AUR submission guidelines](https://wiki.archlinux.org/title/AUR_submission_guidelines)
- [Arch Wiki — `.SRCINFO`](https://wiki.archlinux.org/title/.SRCINFO)
- [Freedesktop Desktop Entry Specification 1.5](https://specifications.freedesktop.org/desktop-entry/latest-single/)
- [Freedesktop Menu Category Registry](https://specifications.freedesktop.org/menu-spec/latest/category-registry.html)

## Packaged files (`0.1.1-1`)

`packaging/arch/PKGBUILD` builds from the single immutable GitHub release tarball `v0.1.1.tar.gz`, generates shell completions from the freshly compiled `./target/release/ouro-terminal-picker` binary (never from a host-installed binary), and installs:

| Artifact | Installed path |
|---|---|
| Binary | `/usr/bin/ouro-terminal-picker` |
| Desktop entry | `/usr/share/applications/ouro-terminal-picker.desktop` |
| Light icon (default) | `/usr/share/pixmaps/ouro-terminal-picker.png` |
| Dark icon variant | `/usr/share/pixmaps/ouro-terminal-picker-dark.png` |
| Bash completion | `/usr/share/bash-completion/completions/ouro-terminal-picker` |
| Fish completion | `/usr/share/fish/vendor_completions.d/ouro-terminal-picker.fish` |
| Zsh completion | `/usr/share/zsh/site-functions/_ouro-terminal-picker` |
| License | `/usr/share/licenses/ouro-terminal-picker/LICENSE` |
| Documentation | `/usr/share/doc/ouro-terminal-picker/{README,terminal-wiring,shell-completions,zellij-session-list,aur-packaging}.md` |

## Desktop launcher (`packaging/desktop/ouro-terminal-picker.desktop`)

- Standards-based `Terminal=true`, `Exec=ouro-terminal-picker`, `TryExec=ouro-terminal-picker`, `Name=OurioPicker`, `Icon=ouro-terminal-picker`, `StartupNotify=false`.
- `Categories=System;ConsoleOnly;`: uses the Freedesktop registered main category `System` and additional category `ConsoleOnly` ("Application that only works inside a terminal"). `TerminalEmulator` is intentionally not used because `ouro-terminal-picker` is a terminal TUI (`Terminal=true`), not a terminal emulator itself, avoiding default-terminal pickers misclassifying it.
- No hard dependency on Kitty, Rio, or any hardcoded terminal emulator or shell command; desktop environments launch it in the user's configured default terminal emulator.
- Validation command:

```sh
desktop-file-validate --warn-kde packaging/desktop/ouro-terminal-picker.desktop
```

## Source pinning and checksum rules

- Never rewrite or move a published tag (`v0.1.0`, `v0.1.1`, ...).
- Every entry in `source=()` must be an immutable, publicly fetchable HTTPS URL with a real `sha512sum` (never `SKIP`).
- Before pushing `v0.1.1` to AUR, tag `v0.1.1` on `main`, compute the exact `sha512sum` of the published GitHub `v0.1.1.tar.gz` archive (replacing the pre-tag zero placeholder), and regenerate `.SRCINFO` with `makepkg --printsrcinfo > .SRCINFO`.
- Prefix downloaded filenames with `${pkgname}-${pkgver}...` so shared `$SRCDEST` directories never collide across packages.
- Never use `replaces=` in an AUR `PKGBUILD`.

## Local verification (before uploading)

Run the hermetic packaging verification script:

```sh
packaging/tests/verify-desktop-and-aur.sh
```

After tagging `v0.1.1` and updating `sha512sums`, verify source fetch, checksum, and `.SRCINFO`:

```sh
cd packaging/arch
makepkg --verifysource
makepkg --printsrcinfo > .SRCINFO
```

Test a clean local build (without `-i` / without `sudo` when only testing the build):

```sh
cd packaging/arch
makepkg --cleanbuild --syncdeps --rmdeps --clean
```

## Manual AUR submission workflow (owner-run only)

AUR publishing is strictly manual under the owner's own AUR SSH identity. Never commit SSH keys, tokens, or AUR credentials to this repository, and never commit `*.pkg.tar*` build outputs to the AUR repository.

1. Configure your AUR SSH key in `~/.ssh/config` per the Arch Wiki AUR submission guidelines (`Host aur.archlinux.org`, `User aur`, dedicated `IdentityFile`).
2. Clone the AUR package repository (first-time creation prints `warning: You appear to have cloned an empty repository` — expected):

```sh
git -c init.defaultBranch=master clone ssh://aur@aur.archlinux.org/ouro-terminal-picker.git
cd ouro-terminal-picker
```

3. Copy **only** the tagged `PKGBUILD` (with real `v0.1.1.tar.gz` `sha512sums`) and freshly generated `.SRCINFO` into the cloned AUR directory (never copy `*.pkg.tar*`, `src/`, `pkg/`, or duplicate asset files):

```sh
cp <path-to-OurioPicker>/packaging/arch/PKGBUILD .
makepkg --printsrcinfo > .SRCINFO
```

4. Confirm git commit identity for the AUR repository (`git config user.name` / `git config user.email`), verify only `PKGBUILD` and `.SRCINFO` are staged, commit on `master` (AUR rejects other branch names), and push:

```sh
git status --short
git add PKGBUILD .SRCINFO
git commit -m "Initial upload: ouro-terminal-picker 0.1.1-1"
git push origin master
```
