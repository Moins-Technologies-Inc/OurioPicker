#!/usr/bin/env bash
# Hermetic verification suite for OurioPicker desktop launcher and AUR packaging (PICK-002).
# Zero network calls, zero sudo, zero package installation, zero Cargo builds.
set -uo pipefail

SCRIPT_DIR="$(cd "$(/usr/bin/dirname "${BASH_SOURCE[0]}")" && pwd -P)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd -P)"

DESKTOP_FILE="${REPO_ROOT}/packaging/desktop/ouro-terminal-picker.desktop"
PKGBUILD_FILE="${REPO_ROOT}/packaging/arch/PKGBUILD"
SRCINFO_FILE="${REPO_ROOT}/packaging/arch/.SRCINFO"
AUR_DOC_FILE="${REPO_ROOT}/docs/wiki/aur-packaging.md"

V010_SHA512="eb6ea8a04fc7a37c51d5e79d0f6b268d316c197185c345c0158263369d2a9bf38c360d16160a68bd2d7e75f8548d8e0180c130c7c0262aed8de1c540661b6721"

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

# 4. No synthetic/placeholder icon assets committed under packaging/
fake_icons="$(/usr/bin/find "${REPO_ROOT}/packaging" -maxdepth 4 \( -name '*.svg' -o -name '*.png' -o -name '*.ico' -o -name '*.xpm' \) 2>/dev/null || true)"
if [[ -z "${fake_icons}" ]]; then
  pass "no-unapproved-icon-assets"
else
  fail "no-unapproved-icon-assets: unexpected icon asset(s) present while icon is pending approval: ${fake_icons}"
fi

# 5. PKGBUILD syntax check
if [[ -f "${PKGBUILD_FILE}" ]] && /usr/bin/bash -n "${PKGBUILD_FILE}"; then
  pass "pkgbuild-syntax"
else
  fail "pkgbuild-syntax: bash -n failed or PKGBUILD missing"
fi

# 6. PKGBUILD AUR rules and desktop/completion coordination
if [[ -f "${PKGBUILD_FILE}" ]]; then
  pkg_ok=1
  if ! /usr/bin/grep -Eq "^pkgver=0\.1\.0$" "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: expected pkgver=0.1.0"
    pkg_ok=0
  fi
  if ! /usr/bin/grep -Eq "^pkgrel=2$" "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: expected pkgrel=2"
    pkg_ok=0
  fi
  if ! /usr/bin/grep -Fq "${V010_SHA512}" "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: missing verified v0.1.0 tarball sha512sum"
    pkg_ok=0
  fi
  if /usr/bin/grep -Eq "['\"]SKIP['\"]" "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: SKIP checksums are forbidden"
    pkg_ok=0
  fi
  if /usr/bin/grep -Eq "^replaces=" "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: replaces= must not be used in AUR PKGBUILD"
    pkg_ok=0
  fi
  if /usr/bin/grep -E "^source=|^  ['\"].*https?://" "${PKGBUILD_FILE}" | /usr/bin/grep -Eq '\.(png|svg|ico)'; then
    fail "pkgbuild-rules: must not reference a nonexistent icon URL in source=()"
    pkg_ok=0
  fi
  if ! /usr/bin/grep -Fq 'usr/share/applications/ouro-terminal-picker.desktop' "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: package() must install usr/share/applications/ouro-terminal-picker.desktop"
    pkg_ok=0
  fi
  # Ensure package() installs from ${srcdir}, not from the v0.1.0 archive working directory
  if /usr/bin/grep -Eq 'install[[:space:]].*packaging/desktop/ouro-terminal-picker\.desktop' "${PKGBUILD_FILE}"; then
    fail "pkgbuild-rules: package() must not reference packaging/desktop/ouro-terminal-picker.desktop absent from v0.1.0 tarball"
    pkg_ok=0
  fi
  for comp_path in \
    "usr/share/bash-completion/completions/ouro-terminal-picker" \
    "usr/share/fish/vendor_completions.d/ouro-terminal-picker.fish" \
    "usr/share/zsh/site-functions/_ouro-terminal-picker"
  do
    if ! /usr/bin/grep -Fq "${comp_path}" "${PKGBUILD_FILE}"; then
      fail "pkgbuild-rules: PKGBUILD must document Arch completion path ${comp_path}"
      pkg_ok=0
    fi
  done
  if [[ "${pkg_ok}" -eq 1 ]]; then
    pass "pkgbuild-rules"
  fi
else
  fail "pkgbuild-rules: PKGBUILD missing"
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

# 8. Desktop file sha512sum matches PKGBUILD and .SRCINFO
if [[ -f "${DESKTOP_FILE}" && -f "${PKGBUILD_FILE}" && -f "${SRCINFO_FILE}" ]]; then
  desktop_sha="$(/usr/bin/sha512sum "${DESKTOP_FILE}" | /usr/bin/awk '{print $1}')"
  if /usr/bin/grep -Fq "${desktop_sha}" "${PKGBUILD_FILE}" && /usr/bin/grep -Fq "${desktop_sha}" "${SRCINFO_FILE}"; then
    pass "desktop-checksum-match"
  else
    fail "desktop-checksum-match: desktop sha512 (${desktop_sha}) missing from PKGBUILD or .SRCINFO"
  fi
else
  fail "desktop-checksum-match: missing desktop/PKGBUILD/.SRCINFO"
fi

# 9. No built package tarballs inside packaging/arch/
pkg_tarballs="$(/usr/bin/find "${REPO_ROOT}/packaging/arch" -maxdepth 2 -name '*.pkg.tar*' 2>/dev/null || true)"
if [[ -z "${pkg_tarballs}" ]]; then
  pass "no-pkg-artifacts"
else
  fail "no-pkg-artifacts: built package tarball(s) found in packaging/arch: ${pkg_tarballs}"
fi

# 10. Fail-closed icon blocker guard in prepare()
if [[ -f "${PKGBUILD_FILE}" ]]; then
  guard_out="$(/usr/bin/bash -c 'source "$1" && prepare' _ "${PKGBUILD_FILE}" 2>&1)"
  guard_rc=$?
  if [[ "${guard_rc}" -ne 0 ]] && printf '%s' "${guard_out}" | /usr/bin/grep -Eqi 'blocked.*icon|transparent png'; then
    pass "blocked-build-guard"
  else
    fail "blocked-build-guard: prepare() must fail closed while icon is pending (rc=${guard_rc}, out=${guard_out})"
  fi
else
  fail "blocked-build-guard: PKGBUILD missing"
fi

# 11. AUR documentation exists and covers required manual steps and blockers
if [[ -f "${AUR_DOC_FILE}" ]]; then
  doc_ok=1
  for pat in \
    "aur@aur\.archlinux\.org/ouro-terminal-picker\.git" \
    "makepkg --printsrcinfo" \
    "PKGBUILD" \
    "\.SRCINFO" \
    "pkg\.tar" \
    "clap_complete" \
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
