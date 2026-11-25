#!/usr/bin/env bash
# Hermetic verification suite for OurioPicker desktop launcher, icons and AUR packaging (PICK-002).
# Zero network calls, zero sudo, zero package installation, zero Cargo builds.
set -uo pipefail

SCRIPT_DIR="$(cd "$(/usr/bin/dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd -P)"

DESKTOP_FILE="${REPO_ROOT}/packaging/desktop/ouro-terminal-picker.desktop"
ALLOW_FILE="${REPO_ROOT}/quality/binary-allow.txt"
CARGO_TOML="${REPO_ROOT}/Cargo.toml"
PKGBUILD_FILE="${REPO_ROOT}/packaging/arch/PKGBUILD"
SRCINFO_FILE="${REPO_ROOT}/packaging/arch/.SRCINFO"
AUR_DOC_FILE="${REPO_ROOT}/docs/wiki/aur-packaging.md"

passed=0
failed=0

pass() {
  passed=$((passed + 1))
  printf 'PASS: %s\n' "$1"
}

fail() {
  failed=$((failed + 1))
  printf 'FAIL: %s\n' "$1" >&2
}

# 1. Desktop entry exists
if [[ -f "${DESKTOP_FILE}" ]]; then
  pass "desktop-exists"
else
  fail "desktop-exists: missing ${DESKTOP_FILE}"
fi

# 2. Desktop entry passes desktop-file-validate --warn-kde with zero output
if [[ -f "${DESKTOP_FILE}" ]]; then
  val_out="$(/usr/bin/desktop-file-validate --warn-kde "${DESKTOP_FILE}" 2>&1)"
  val_rc=$?
  if [[ "${val_rc}" -eq 0 && -z "${val_out}" ]]; then
    pass "desktop-validate"
  else
    fail "desktop-validate: exit=${val_rc} output=${val_out}"
  fi
else
  fail "desktop-validate: skipped (desktop file missing)"
fi

# 3. Required desktop keys and no hardcoded terminal emulator / shell wrapper
if [[ -f "${DESKTOP_FILE}" ]]; then
  keys_ok=1
  for required in \
    "^\[Desktop Entry\]$" \
    "^Type=Application$" \
    "^Version=1\.5$" \
    "^Name=OurioPicker$" \
    "^TryExec=ouro-terminal-picker$" \
    "^Exec=ouro-terminal-picker$" \
    "^Icon=ouro-terminal-picker$" \
    "^Terminal=true$" \
    "^Categories=System;ConsoleOnly;$"
  do
    if ! /usr/bin/grep -Eq "${required}" "${DESKTOP_FILE}"; then
      fail "desktop-keys: missing required line matching ${required}"
      keys_ok=0
    fi
  done
  if /usr/bin/grep -E '^(Exec|TryExec)=.*(kitty|rio|alacritty|foot|wezterm|xterm|konsole|gnome-terminal|/bin/sh|bash -c|zsh -c)' "${DESKTOP_FILE}" >/dev/null 2>&1; then
    fail "desktop-keys: Exec/TryExec must not hardcode a terminal emulator or shell command"
    keys_ok=0
  fi
  if [[ "${keys_ok}" -eq 1 ]]; then
    pass "desktop-keys"
  fi
else
  fail "desktop-keys: skipped (desktop file missing)"
fi

# 4. Both owner-accepted PNG icons exist, have PNG magic bytes, and match quality/binary-allow.txt
icons_ok=1
for rel in "packaging/icons/ouro-terminal-picker.png" "packaging/icons/ouro-terminal-picker-dark.png"; do
  abs="${REPO_ROOT}/${rel}"
  if [[ ! -f "${abs}" ]]; then
    fail "icons-allowed: missing ${rel}"
    icons_ok=0
    continue
  fi
  magic="$(/usr/bin/od -An -tx1 -N8 "${abs}" | /usr/bin/tr -d ' \n')"
  if [[ "${magic}" != "89504e470d0a1a0a" ]]; then
    fail "icons-allowed: ${rel} is not a valid PNG file (magic=${magic})"
    icons_ok=0
    continue
  fi
  blob="$(/usr/bin/git -C "${REPO_ROOT}" hash-object "${abs}")"
  if [[ ! -f "${ALLOW_FILE}" ]] || ! /usr/bin/grep -Eq "^${rel}[[:space:]]+${blob}([[:space:]]|$)" "${ALLOW_FILE}"; then
    fail "icons-allowed: ${rel} (blob ${blob}) is not allowlisted in quality/binary-allow.txt"
    icons_ok=0
  fi
done
if [[ "${icons_ok}" -eq 1 ]]; then
  pass "icons-allowed"
fi

# 5. PKGBUILD syntax check
if [[ -f "${PKGBUILD_FILE}" ]] && /usr/bin/bash -n "${PKGBUILD_FILE}"; then
  pass "pkgbuild-syntax"
else
  fail "pkgbuild-syntax: bash -n failed or PKGBUILD missing"
fi

# 6. PKGBUILD AUR rules, version alignment, completion generation, and install paths
if [[ -f "${PKGBUILD_FILE}" && -f "${CARGO_TOML}" ]]; then
  pkg_ok=1
  cargo_ver="$(/usr/bin/awk -F'"' '/^version = / { print $2; exit }' "${CARGO_TOML}")"
  pkg_ver="$(/usr/bin/awk -F'=' '/^pkgver=/ { print $2; exit }' "${PKGBUILD_FILE}")"
  if [[ "${cargo_ver}" != "0.1.1" || "${pkg_ver}" != "${cargo_ver}" ]]; then
    fail "pkgbuild-rules: expected Cargo.toml version and PKGBUILD pkgver to both equal 0.1.1 (got cargo=${cargo_ver}, pkgver=${pkg_ver})"
    pkg_ok=0
  fi
  if ! /usr/bin/grep -Eq "^pkgrel=1$" "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: expected pkgrel=1 for 0.1.1 release"
    pkg_ok=0
  fi
  if ! /usr/bin/grep -Fxq 'source=("${pkgname}-${pkgver}.tar.gz::https://github.com/Moins-Technologies-Inc/OurioPicker/archive/v${pkgver}.tar.gz")' "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: expected single immutable GitHub release tarball source"
    pkg_ok=0
  fi
  if /usr/bin/grep -Eq "['\"]SKIP['\"]" "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: SKIP checksums are forbidden"
    pkg_ok=0
  fi
  if ! /usr/bin/grep -Eq "^sha512sums=\('[0-9a-f]{128}'\)" "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: sha512sums must contain a valid 128-hex-character digest"
    pkg_ok=0
  fi
  if /usr/bin/grep -Eq "^replaces=" "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: replaces= must not be used in AUR PKGBUILD"
    pkg_ok=0
  fi
  if /usr/bin/grep -Eq 'package build is blocked|return 1' "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: prepare() blocker should be removed for 0.1.1"
    pkg_ok=0
  fi
  for gen_line in \
    "./target/release/ouro-terminal-picker completions bash" \
    "./target/release/ouro-terminal-picker completions fish" \
    "./target/release/ouro-terminal-picker completions zsh"
  do
    if ! /usr/bin/grep -Fq "${gen_line}" "${PKGBUILD_FILE}"; then
      fail "pkgbuild-rules: build() must generate completions from freshly built binary (${gen_line})"
      pkg_ok=0
    fi
  done
  for install_target in \
    'usr/bin' \
    'usr/share/applications/ouro-terminal-picker.desktop' \
    'usr/share/pixmaps/ouro-terminal-picker.png' \
    'usr/share/pixmaps/ouro-terminal-picker-dark.png' \
    'usr/share/bash-completion/completions/ouro-terminal-picker' \
    'usr/share/fish/vendor_completions.d/ouro-terminal-picker.fish' \
    'usr/share/zsh/site-functions/_ouro-terminal-picker' \
    'usr/share/licenses/${pkgname}/LICENSE' \
    'usr/share/doc/${pkgname}/README.md' \
    'usr/share/doc/${pkgname}/terminal-wiring.md' \
    'usr/share/doc/${pkgname}/shell-completions.md' \
    'usr/share/doc/${pkgname}/zellij-session-list.md' \
    'usr/share/doc/${pkgname}/aur-packaging.md'
  do
    if ! /usr/bin/grep -Fq "${install_target}" "${PKGBUILD_FILE}"; then
      fail "pkgbuild-rules: package() missing install target ${install_target}"
      pkg_ok=0
    fi
  done
  if [[ "${pkg_ok}" -eq 1 ]]; then
    pass "pkgbuild-rules"
  fi
else
  fail "pkgbuild-rules: PKGBUILD or Cargo.toml missing"
fi

# 7. .SRCINFO exists and matches makepkg --printsrcinfo byte-for-byte
if [[ -f "${SRCINFO_FILE}" && -f "${PKGBUILD_FILE}" ]]; then
  tmp_srcinfo="$(/usr/bin/mktemp)"
  if (cd "${REPO_ROOT}/packaging/arch" && /usr/bin/makepkg --printsrcinfo > "${tmp_srcinfo}") 2>/dev/null \
     && /usr/bin/diff -u "${SRCINFO_FILE}" "${tmp_srcinfo}" >/dev/null; then
    pass "srcinfo-sync"
  else
    fail "srcinfo-sync: .SRCINFO differs from makepkg --printsrcinfo"
  fi
  /usr/bin/rm -f "${tmp_srcinfo}"
else
  fail "srcinfo-sync: missing ${SRCINFO_FILE}"
fi

# 8. No built package tarballs inside packaging/arch/
pkg_tarballs="$(/usr/bin/find "${REPO_ROOT}/packaging/arch" -maxdepth 2 -name '*.pkg.tar*' 2>/dev/null || true)"
if [[ -z "${pkg_tarballs}" ]]; then
  pass "no-pkg-artifacts"
else
  fail "no-pkg-artifacts: built package tarball(s) found in packaging/arch: ${pkg_tarballs}"
fi

# 9. AUR documentation exists and covers required manual steps and paths
if [[ -f "${AUR_DOC_FILE}" ]]; then
  doc_ok=1
  for pat in \
    "aur@aur\.archlinux\.org/ouro-terminal-picker\.git" \
    "makepkg --printsrcinfo" \
    "PKGBUILD" \
    "\.SRCINFO" \
    "pkg\.tar" \
    "usr/share/pixmaps/ouro-terminal-picker\.png" \
    "usr/share/pixmaps/ouro-terminal-picker-dark\.png" \
    "usr/share/bash-completion/completions/ouro-terminal-picker" \
    "usr/share/fish/vendor_completions\.d/ouro-terminal-picker\.fish" \
    "usr/share/zsh/site-functions/_ouro-terminal-picker"
  do
    if ! /usr/bin/grep -Eq "${pat}" "${AUR_DOC_FILE}"; then
      fail "docs-aur-guide: missing required content matching ${pat}"
      doc_ok=0
    fi
  done
  if [[ "${doc_ok}" -eq 1 ]]; then
    pass "docs-aur-guide"
  fi
else
  fail "docs-aur-guide: missing ${AUR_DOC_FILE}"
fi

printf '\nSummary: %d passed, %d failed\n' "${passed}" "${failed}"
[[ "${failed}" -eq 0 ]]
