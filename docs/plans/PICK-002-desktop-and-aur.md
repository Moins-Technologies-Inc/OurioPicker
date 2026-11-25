# PICK-002: Desktop launcher (`.desktop`), icons and AUR packaging (`v0.1.1`)

Status: Review
Owner app/crate: OurioPicker (`ouro-terminal-picker`)
Matrix row: `FEATURES.md#PICK-002`
Issue: OurioPicker `#5`

## 1. Plan
- Purpose and user-visible behaviour: provide a standards-compliant Freedesktop `.desktop` launcher, bundle the owner-accepted light and dark application icons, and prepare the `0.1.1-1` Arch User Repository (`AUR`) packaging recipe (`PKGBUILD`, `.SRCINFO`, and manual upload documentation) that also builds and installs Bash, Fish, and Zsh shell completions (`PICK-003`).
- Non-goals:
  - No rewriting or moving the immutable `v0.1.0` tag.
  - No automated AUR publishing, no AUR credentials stored, no `sudo`, and no local package installation.
- Tenants: single-user local desktop packaging files installed system-wide by the user's package manager (`/usr/share/{applications,pixmaps,bash-completion,fish,zsh,licenses,doc}/...`); contains no tenant or user data.
- Inputs / outputs / interfaces:
  - `packaging/desktop/ouro-terminal-picker.desktop` -> `/usr/share/applications/ouro-terminal-picker.desktop`.
  - `packaging/icons/ouro-terminal-picker.png` -> `/usr/share/pixmaps/ouro-terminal-picker.png`.
  - `packaging/icons/ouro-terminal-picker-dark.png` -> `/usr/share/pixmaps/ouro-terminal-picker-dark.png`.
  - `target/release/ouro-terminal-picker completions bash|fish|zsh` -> installed to `/usr/share/bash-completion/completions/ouro-terminal-picker`, `/usr/share/fish/vendor_completions.d/ouro-terminal-picker.fish`, and `/usr/share/zsh/site-functions/_ouro-terminal-picker`.
  - `packaging/arch/PKGBUILD` and `packaging/arch/.SRCINFO` -> `0.1.1-1` single-tarball AUR submission recipe (pre-tag 128-zero `sha512sums` placeholder refreshed immediately upon tagging `v0.1.1`).
  - `packaging/tests/verify-desktop-and-aur.sh` -> hermetic packaging and desktop entry validation script.
  - `docs/wiki/aur-packaging.md` -> AUR submission guidelines and manual upload workflow.
- Config keys: none.
- Acceptance criteria:
  1. `packaging/desktop/ouro-terminal-picker.desktop` passes `desktop-file-validate --warn-kde` with zero errors/warnings, uses `Terminal=true`, `Exec=ouro-terminal-picker`, `TryExec=ouro-terminal-picker`, `Name=OurioPicker`, `Icon=ouro-terminal-picker`, `Categories=System;ConsoleOnly;`, and has no Kitty/Rio hard dependency or hardcoded shell command.
  2. Both owner-accepted PNG icons are committed verbatim and match `quality/binary-allow.txt` on `main`.
  3. `Cargo.toml` and `Cargo.lock` are bumped to `0.1.1` cleanly (offline on the workstation).
  4. `packaging/arch/PKGBUILD` passes `bash -n`, builds from `OurioPicker-${pkgver}` (`0.1.1-1`), generates Bash/Fish/Zsh completions from `./target/release/ouro-terminal-picker` in `build()`, installs binary, `.desktop`, both pixmap icons, all three completions, LICENSE, README, and wiki docs, uses no `SKIP`, and uses no `replaces=`.
  5. `packaging/arch/.SRCINFO` matches `makepkg --printsrcinfo` byte-for-byte.
  6. `packaging/tests/verify-desktop-and-aur.sh` passes locally and the full Rust test/clippy/fmt/doc/deny/machete + release completion suite passes on the build workstation.
  7. Fresh independent review PASS before merge to `main`.
- Target tests:
  - Hermetic shell suite `packaging/tests/verify-desktop-and-aur.sh` (9 checks covering desktop entry, `desktop-file-validate --warn-kde`, PNG magic & `quality/binary-allow.txt` blob SHAs, `bash -n`, `PKGBUILD` 0.1.1 rules & install targets, `.SRCINFO` byte-sync, absence of `.pkg.tar*`, and AUR docs).
  - Workstation Rust verification: `cargo +1.100.0 fmt --all --check`, `cargo +1.100.0 clippy --locked --all-targets --all-features -- -D warnings`, `cargo +1.100.0 test --locked --all-features`, `cargo +1.100.0 doc --locked --no-deps`, `cargo-deny`, `cargo-machete`, and release completion generation (`bash`, `fish`, `zsh`).

## 2. Skip (reuse)
| Candidate tool/standard | Maturity | Licence | Already available? | Decision |
|---|---|---|---|---|
| `desktop-file-validate` (`desktop-file-utils` 0.29) | Standard Freedesktop validator | GPL-2.0 | yes | Use for `.desktop` validation |
| `makepkg` (`pacman` 7.1.0) | Official Arch build tool | GPL-2.0 | yes | Use for `--printsrcinfo` and package builds |
| `ouro-terminal-picker completions` (`PICK-003`) | Merged on `main` | MIT | yes | Invoke freshly built binary in `build()` |

## 3. Remove (cascade and refactor)
- Systems affected: `Cargo.toml`, `Cargo.lock`, `packaging/`, `docs/`, `README.md`, `FEATURES.md`, `CURRENT_WORK.md`.
- Simplest design chosen: bump to `0.1.1` so `PKGBUILD` builds from a single GitHub release tarball containing source, `.desktop`, icons, and `clap_complete` support.
- Rollback or migration: revert commit before tagging `v0.1.1`.

## 4. Optimize
- Packaging metadata and static launcher/icon assets only; no runtime hot path.

## 5. Failure
| Failure | Cause (internal/external) | Detection | Handling | User message |
|---|---|---|---|---|
| Unapproved binary file | Icon bytes modified after allowlisting | `quality/binary-allow.txt` check in hub gate + `icons-allowed` test | Reject commit/push | Gate `binary-files` or test failure naming blob SHA |
| Stale `.SRCINFO` | Editing `PKGBUILD` without regenerating `.SRCINFO` | `srcinfo-sync` (`diff` vs `makepkg --printsrcinfo`) | Fail verification script | `FAIL: srcinfo-sync: .SRCINFO differs from makepkg --printsrcinfo` |
| Pre-tag placeholder uploaded to AUR | Skipping post-tag `sha512sums` refresh | `makepkg --verifysource` fails on zero digest | Update via `aur-update-sha.py` after tagging `v0.1.1` and regenerate `.SRCINFO` | `makepkg` checksum failure |

Trust boundaries: `PKGBUILD` fetches only the public GitHub release archive verified by `sha512sums`; completions are generated exclusively from `./target/release/ouro-terminal-picker` built inside the package tree.

## 6. Logging
- Verification script prints `PASS:` / `FAIL:` lines; no private paths or credentials.

## 7. Performance traces
- Not applicable.

## 8. Study (wiki entries)
- Updated `docs/wiki/aur-packaging.md` covering `0.1.1-1` packaged layout, icon paths, completion generation, and manual AUR upload steps.

## 9. Finish
- [x] `origin/main` (`PICK-003` + `quality/binary-allow.txt`) merged into `feature/5-desktop-aur`
- [x] Both owner-accepted PNG icons bundled verbatim matching `quality/binary-allow.txt`
- [x] `Cargo.toml` and `Cargo.lock` bumped to `0.1.1`
- [x] `PKGBUILD` unblocked for `0.1.1-1` and `.SRCINFO` regenerated via `makepkg --printsrcinfo`
- [x] `packaging/tests/verify-desktop-and-aur.sh` green locally
- [x] Workstation `cargo +1.100.0` fmt/clippy/test/doc/deny/machete + release completion generation green
- [ ] Fresh independent review PASS
- [ ] Merge (`Closes #5`), tag `v0.1.1`, update `sha512sums` + `.SRCINFO`, and manual AUR push
