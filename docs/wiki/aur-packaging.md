# Desktop Launcher and AUR Packaging (`ouro-terminal-picker`)

This page documents the Freedesktop desktop launcher, Arch Linux / AUR packaging rules, current release blockers, and manual AUR submission workflow for `ouro-terminal-picker` (OurioPicker).

Authoritative references:

- [Arch Wiki — AUR submission guidelines](https://wiki.archlinux.org/title/AUR_submission_guidelines)
- [Arch Wiki — `.SRCINFO`](https://wiki.archlinux.org/title/.SRCINFO)
- [Freedesktop Desktop Entry Specification 1.5](https://specifications.freedesktop.org/desktop-entry/latest-single/)
- [Freedesktop Menu Category Registry](https://specifications.freedesktop.org/menu-spec/latest/category-registry.html)

## Current status (BLOCKED — do not upload to AUR yet)

As of OurioPicker issue `#5`, the AUR RPC check confirms no `ouro-terminal-picker` package exists on AUR, and the desktop launcher plus staged `PKGBUILD` / `.SRCINFO` are prepared on branch `feature/5-desktop-aur`.

**Do not build or push this recipe to AUR until both pending items are resolved:**

1. **Icon asset (blocked):** `packaging/desktop/ouro-terminal-picker.desktop` sets `Icon=ouro-terminal-picker`, pending an owner-approved transparent PNG icon (or explicit owner authorization to switch to a generic icon-theme fallback). No placeholder, synthetic SVG, opaque PNG, or nonexistent icon URL is shipped. `prepare()` in `packaging/arch/PKGBUILD` fails closed while this blocker is open.
2. **Shell completions (`clap_complete`, pending parallel work):** Bash, Fish, and Zsh completion generation is being implemented in a separate branch. Once merged, `PKGBUILD` must bundle completions generated from the newly built binary or Cargo build output (never from a host-installed `ouro-terminal-picker`) and install them to:
   - Bash: `/usr/share/bash-completion/completions/ouro-terminal-picker`
   - Fish: `/usr/share/fish/vendor_completions.d/ouro-terminal-picker.fish`
   - Zsh: `/usr/share/zsh/site-functions/_ouro-terminal-picker`

Because `clap_complete` changes Rust application code, the recommended release path once both the icon and completions land on `main` is to cut a new release tag (for example `v0.1.1` or `v0.2.0`) so `PKGBUILD` fetches a single immutable GitHub release tarball containing the Rust source, `packaging/desktop/ouro-terminal-picker.desktop`, and the approved transparent PNG icon.

## Desktop launcher (`packaging/desktop/ouro-terminal-picker.desktop`)

- Installed path: `/usr/share/applications/ouro-terminal-picker.desktop`.
- Standards-based `Terminal=true`, `Exec=ouro-terminal-picker`, `TryExec=ouro-terminal-picker`, `Name=OurioPicker`, `Icon=ouro-terminal-picker`, `StartupNotify=false`.
- `Categories=System;ConsoleOnly;`: uses the Freedesktop registered main category `System` and additional category `ConsoleOnly` ("Application that only works inside a terminal"). `TerminalEmulator` is intentionally not used because `ouro-terminal-picker` is a terminal TUI (`Terminal=true`), not a terminal emulator itself, avoiding default-terminal pickers misclassifying it.
- No hard dependency on Kitty, Rio, or any hardcoded terminal emulator or shell command; desktop environments launch it in the user's configured default terminal emulator.
- Validation command:

```sh
desktop-file-validate --warn-kde packaging/desktop/ouro-terminal-picker.desktop
```

## Source pinning and checksum rules

- Never rewrite or move the published `v0.1.0` tag.
- Never reference files inside `OurioPicker-0.1.0` that do not exist in `v0.1.0.tar.gz` (such as `packaging/desktop/ouro-terminal-picker.desktop`).
- Every entry in `source=()` must be an immutable, publicly fetchable HTTPS URL with a real `sha512sum` (never `SKIP`).
- Prefix downloaded filenames with `${pkgname}-${pkgver}...` so shared `$SRCDEST` directories never collide across packages.
- While staged on `v0.1.0`, the desktop file is pinned to an immutable raw GitHub commit URL. Before any AUR upload, either switch to the new release tag tarball (recommended once `clap_complete` + icon merge) or re-pin any raw URL to a commit permanently reachable on `main`.
- Never use `replaces=` in an AUR `PKGBUILD`.

## Coordinating `clap_complete` completions in `PKGBUILD`

Once the completions worker lands `clap_complete` support and a new release tag is cut:

1. Generate completion files during `build()` using the freshly compiled `./target/release/ouro-terminal-picker` binary (or consume files emitted by `build.rs` / `OUT_DIR` during `cargo build --frozen --release`). Never run a host-installed `ouro-terminal-picker` from `$PATH`.
2. Install the generated files in `package()`:
   - `install -Dm644 <generated-bash> "${pkgdir}/usr/share/bash-completion/completions/ouro-terminal-picker"`
   - `install -Dm644 <generated-fish> "${pkgdir}/usr/share/fish/vendor_completions.d/ouro-terminal-picker.fish"`
   - `install -Dm644 <generated-zsh> "${pkgdir}/usr/share/zsh/site-functions/_ouro-terminal-picker"`
3. When bundling the approved transparent PNG icon, add `'hicolor-icon-theme'` to `depends` and install the icon to `"${pkgdir}/usr/share/icons/hicolor/<size>x<size>/apps/ouro-terminal-picker.png"`.

## Local verification (before uploading)

Run the hermetic packaging verification script:

```sh
packaging/tests/verify-desktop-and-aur.sh
```

Verify source fetches and checksums without installing or building:

```sh
cd packaging/arch
makepkg --verifysource
```

Regenerate `.SRCINFO` after any metadata change in `PKGBUILD`:

```sh
cd packaging/arch
makepkg --printsrcinfo > .SRCINFO
```

Once the icon and completions blockers are removed, test a clean local build (without `-i` / without `sudo` when only testing the build):

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

3. Copy **only** the unblocked `PKGBUILD` and freshly generated `.SRCINFO` into the cloned AUR directory (optional: a `0BSD` `LICENSE` for the packaging scripts themselves if desired per Arch RFC 0040; never copy `*.pkg.tar*`, `src/`, `pkg/`, or `.desktop`/`.png` duplicates when fetched from upstream URLs):

```sh
cp <path-to-OurioPicker>/packaging/arch/PKGBUILD .
makepkg --printsrcinfo > .SRCINFO
```

4. Optionally set the maintainer contact line at the top of `PKGBUILD` and confirm git commit identity for the AUR repository (`git config user.name` / `git config user.email`).
5. Verify only `PKGBUILD` and `.SRCINFO` are staged, commit on `master` (AUR rejects other branch names), and push:

```sh
git status --short
git add PKGBUILD .SRCINFO
git commit -m "Initial upload: ouro-terminal-picker <pkgver>-<pkgrel>"
git push origin master
```
