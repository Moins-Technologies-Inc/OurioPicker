# PICK-002: Desktop launcher (`.desktop`), icon integration and AUR packaging

Status: In progress (blocked on owner-approved transparent PNG icon and pending `clap_complete` shell completions)
Owner app/crate: OurioPicker (`ouro-terminal-picker`)
Matrix row: `FEATURES.md#PICK-002`
Issue: OurioPicker `#5`

## 1. Plan
- Purpose and user-visible behaviour: provide a standards-compliant Freedesktop `.desktop` launcher so users can launch OurioPicker from any desktop application menu inside their configured default terminal emulator (`Terminal=true`), and prepare the Arch User Repository (`AUR`) packaging recipe (`PKGBUILD`, `.SRCINFO`, local staging directory/tarball, and manual upload documentation).
- Non-goals:
  - No Rust application edits (`src/`, `Cargo.toml`, `Cargo.lock` untouched).
  - No synthetic SVG/PNG icon generation and no unapproved generic icon fallback.
  - No `clap_complete` Rust implementation (owned by a separate parallel worker; coordinated at the packaging layer only).
  - No rewriting or moving the immutable `v0.1.0` tag.
  - No automated AUR publishing, no AUR credentials stored, no `sudo`, and no local package installation.
- Tenants: single-user local desktop packaging files installed system-wide by the user's package manager (`/usr/share/applications/...`); contains no tenant or user data.
- Inputs / outputs / interfaces:
  - `packaging/desktop/ouro-terminal-picker.desktop` -> installed to `/usr/share/applications/ouro-terminal-picker.desktop`.
  - `packaging/arch/PKGBUILD` and `packaging/arch/.SRCINFO` -> AUR submission recipe (currently fail-closed blocked in `prepare()` until icon and completions land).
  - `packaging/tests/verify-desktop-and-aur.sh` -> hermetic packaging and desktop entry validation script.
  - `docs/wiki/aur-packaging.md` -> AUR submission guidelines, manual upload steps, and completion/icon coordination.
- Config keys: none.
- Acceptance criteria:
  1. `packaging/desktop/ouro-terminal-picker.desktop` passes `desktop-file-validate --warn-kde` with zero errors/warnings, uses `Terminal=true`, `Exec=ouro-terminal-picker`, `TryExec=ouro-terminal-picker`, `Name=OurioPicker`, `Icon=ouro-terminal-picker`, `Categories=System;ConsoleOnly;`, and has no Kitty/Rio hard dependency or hardcoded shell command.
  2. `packaging/arch/PKGBUILD` passes `bash -n`, keeps `v0.1.0` immutable, pins immutable publicly fetchable sources with real `sha512sums` (no `SKIP`, no nonexistent icon URL), fails closed in `prepare()` while blocked on the icon and completions, and documents the Arch standard Bash/Fish/Zsh completion target paths.
  3. `packaging/arch/.SRCINFO` matches `makepkg --printsrcinfo` byte-for-byte.
  4. `makepkg --verifysource` verifies all pinned source URLs and `sha512sums`.
  5. `docs/wiki/aur-packaging.md` documents the manual AUR git workflow (`PKGBUILD` + `.SRCINFO` only, no `.pkg.tar*`, `master` branch) and completion generation from the built binary (never host-installed).
  6. `packaging/tests/verify-desktop-and-aur.sh` passes (with RED run recorded before implementation and GREEN run after).
  7. Fresh independent review PASS before merge to `main`.
- Target tests:
  - Hermetic shell suite `packaging/tests/verify-desktop-and-aur.sh` (11 checks: desktop presence, `desktop-file-validate --warn-kde`, required keys and absence of hardcoded terminals/shells, no unapproved icon assets, `bash -n`, AUR PKGBUILD rules, `.SRCINFO` byte-equality with `makepkg --printsrcinfo`, desktop `sha512sum` match across file/`PKGBUILD`/`.SRCINFO`, no `.pkg.tar*` artifacts, fail-closed `prepare()` blocker check, AUR docs coverage).
  - Live source verification: `makepkg --verifysource` in a scratch directory against public GitHub URLs.

## 2. Skip (reuse)
| Candidate tool/standard | Maturity | Licence | Already available? | Decision |
|---|---|---|---|---|
| `desktop-file-validate` (`desktop-file-utils` 0.29) | Standard Freedesktop validator | GPL-2.0 | yes | Use for `.desktop` validation |
| `makepkg` (`pacman` 7.1.0) | Official Arch build tool | GPL-2.0 | yes | Use for `--printsrcinfo` and `--verifysource` |

Custom code that remains and why: one small hermetic verification script (`packaging/tests/verify-desktop-and-aur.sh`) to assert repository-specific invariants not covered by `desktop-file-validate` alone.

## 3. Remove (cascade and refactor)
- Systems affected: `packaging/`, `docs/`, `README.md`, `FEATURES.md`, `CURRENT_WORK.md`. No Rust code or private workstation config modified.
- Simplest design chosen: keep `v0.1.0` immutable and pin `ouro-terminal-picker.desktop` by immutable raw commit URL during staging; recommend cutting a new release tag (`v0.1.1` / `v0.2.0`) once the approved icon and `clap_complete` completions merge so the final AUR `PKGBUILD` fetches a single clean release tarball.
- Rollback or migration: removing the `.desktop` file and reverting `packaging/arch/PKGBUILD` restores `pkgrel=1` cleanly.

## 4. Optimize
- Packaging metadata and static launcher only; no runtime hot path.

## 5. Failure
| Failure | Cause (internal/external) | Detection | Handling | User message |
|---|---|---|---|---|
| Unapproved/missing icon shipped | Premature `makepkg` build or AUR upload | Fail-closed check in `PKGBUILD` `prepare()` + test `blocked-build-guard` | Abort `prepare()` with exit 1 | `ERROR: AUR package build is blocked pending an owner-approved transparent PNG icon and clap_complete shell completions (OurioPicker#5).` |
| Stale `.SRCINFO` | Editing `PKGBUILD` without regenerating `.SRCINFO` | Test `srcinfo-sync` (`diff` vs `makepkg --printsrcinfo`) | Fail verification script | `FAIL: srcinfo-sync: .SRCINFO differs from makepkg --printsrcinfo` |
| Broken desktop entry or invalid category | Typo or non-standard key in `.desktop` | `desktop-file-validate --warn-kde` + test `desktop-keys` | Fail verification script | Exact validator diagnostic |
| Referencing files absent in `v0.1.0.tar.gz` | Copying `packaging/desktop/...` from `$srcdir/OurioPicker-0.1.0` | Test `pkgbuild-rules` + `makepkg --verifysource` | Install from `${srcdir}/${pkgname}-${pkgver}-${pkgrel}.desktop` | Test failure |

Trust boundaries: `PKGBUILD` downloads from public HTTPS GitHub URLs and verifies exact `sha512sums` before any build step. No untrusted host binary is executed to generate completions.

## 6. Logging
- Packaging/verification scripts print structured `PASS:` / `FAIL:` lines to stdout/stderr; no user secrets or private paths.

## 7. Performance traces
- Not applicable (static metadata and packaging scripts).

## 8. Study (wiki entries)
- Added `docs/wiki/aur-packaging.md` citing Arch Wiki AUR submission guidelines, `.SRCINFO`, Freedesktop Desktop Entry Spec 1.5, and Menu Category Registry.

## 9. Finish
- [x] Hermetic packaging tests written first (RED recorded) and passing (GREEN recorded)
- [x] `desktop-file-validate --warn-kde` PASS
- [x] `makepkg --printsrcinfo` and `makepkg --verifysource` verified
- [x] `docs/wiki/aur-packaging.md` added
- [ ] Icon asset resolved (owner-approved transparent PNG or explicit generic fallback authorization)
- [ ] `clap_complete` shell completions (Bash, Fish, Zsh) integrated into `PKGBUILD` + `.SRCINFO` (and final unblocked local `makepkg` build verified)
- [ ] Fresh independent review PASS
- [ ] `FEATURES.md` updated to Done and merged (`Closes #5`)
