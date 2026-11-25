//! Kitty configuration planner (`ouro-terminal-picker.conf` + `kitty.conf`).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use crate::config::{
    ConfigError, ConfigPlan, FileOp, TerminalKind, absolute_env_path, ensure_no_symlink_ancestors,
    read_checked_target, validate_binary_path,
};

/// Name of the managed include file created inside the Kitty config directory.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::config::kitty::INCLUDE_FILE_NAME;
///
/// assert_eq!(INCLUDE_FILE_NAME, "ouro-terminal-picker.conf");
/// ```
pub const INCLUDE_FILE_NAME: &str = "ouro-terminal-picker.conf";

/// Comment marker written immediately above the managed `include` line in `kitty.conf`.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::config::kitty::MARKER_LINE;
///
/// assert_eq!(MARKER_LINE, "# ouro-terminal-picker");
/// ```
pub const MARKER_LINE: &str = "# ouro-terminal-picker";

/// Kitty include directive referencing [`INCLUDE_FILE_NAME`].
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::config::kitty::INCLUDE_LINE;
///
/// assert_eq!(INCLUDE_LINE, "include ouro-terminal-picker.conf");
/// ```
pub const INCLUDE_LINE: &str = "include ouro-terminal-picker.conf";

/// Resolve the Kitty config directory (`$XDG_CONFIG_HOME/kitty`, else `$HOME/.config/kitty`).
///
/// # Examples
///
/// ```
/// use std::ffi::OsStr;
/// use std::path::PathBuf;
/// use ouro_terminal_picker::config::kitty::kitty_config_dir;
///
/// let dir = kitty_config_dir(Some(OsStr::new("/tmp/xdg")), None)?;
/// assert_eq!(dir, PathBuf::from("/tmp/xdg/kitty"));
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn kitty_config_dir(
    xdg_config_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Result<PathBuf, ConfigError> {
    if let Some(xdg) = absolute_env_path(xdg_config_home) {
        return Ok(xdg.join("kitty"));
    }
    if let Some(h) = absolute_env_path(home) {
        return Ok(h.join(".config").join("kitty"));
    }
    Err(ConfigError::NoConfigDir)
}

/// Render the exact bytes of `ouro-terminal-picker.conf` for `binary_path`.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ouro_terminal_picker::config::kitty::render_include_file;
///
/// let bytes = render_include_file(Path::new("/usr/local/bin/ouro-terminal-picker"))?;
/// let text = String::from_utf8(bytes).unwrap();
/// assert!(text.contains("shell /usr/local/bin/ouro-terminal-picker"));
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn render_include_file(binary_path: &Path) -> Result<Vec<u8>, ConfigError> {
    let bin_str = validate_binary_path(binary_path)?;
    if bin_str.contains('\'') || bin_str.contains('"') {
        return Err(ConfigError::InvalidBinaryPath {
            path: binary_path.to_path_buf(),
        });
    }
    let shell_val = if bin_str.chars().any(char::is_whitespace) {
        format!("'{bin_str}'")
    } else {
        bin_str.to_owned()
    };
    Ok(format!("# Managed by ouro-terminal-picker\nshell {shell_val}\n").into_bytes())
}

/// Exact lines a user can paste into `kitty.conf` when it is externally managed.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::config::kitty::manual_include_snippet;
///
/// assert!(manual_include_snippet().contains("include ouro-terminal-picker.conf"));
/// ```
pub const fn manual_include_snippet() -> &'static str {
    "# ouro-terminal-picker\ninclude ouro-terminal-picker.conf"
}

fn has_managed_block(lines: &[&str]) -> bool {
    lines
        .windows(2)
        .any(|pair| matches!(pair, [a, b] if a.trim() == MARKER_LINE && b.trim() == INCLUDE_LINE))
}

fn decode_kitty_conf<'a>(path: &Path, bytes: &'a [u8]) -> Result<&'a str, ConfigError> {
    std::str::from_utf8(bytes).map_err(|source| ConfigError::InvalidUtf8 {
        path: path.to_path_buf(),
        source,
    })
}

fn append_managed_block(existing: Option<&str>) -> Vec<u8> {
    let mut text = existing.unwrap_or_default().to_owned();
    if !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(MARKER_LINE);
    text.push('\n');
    text.push_str(INCLUDE_LINE);
    text.push('\n');
    text.into_bytes()
}

fn remove_managed_block(text: &str) -> Option<Vec<u8>> {
    let mut out = String::with_capacity(text.len());
    let raw_lines: Vec<&str> = text.split_inclusive('\n').collect();
    let mut idx = 0usize;
    let mut changed = false;

    while idx < raw_lines.len() {
        let cur = raw_lines.get(idx).copied().unwrap_or("");
        let next = raw_lines.get(idx + 1).copied();
        if cur.trim() == MARKER_LINE && next.is_some_and(|n| n.trim() == INCLUDE_LINE) {
            changed = true;
            idx += 2;
            continue;
        }
        out.push_str(cur);
        idx += 1;
    }

    if changed {
        Some(out.into_bytes())
    } else {
        None
    }
}

/// Plan installing OurioPicker wiring into `kitty_dir` using in-memory file contents.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ouro_terminal_picker::config::kitty::plan_install_bytes;
///
/// let plan = plan_install_bytes(
///     Path::new("/tmp/kitty"),
///     Some(b"font_size 12.0\n"),
///     None,
///     Path::new("/usr/local/bin/ouro-terminal-picker"),
/// )?;
/// assert_eq!(plan.ops.len(), 2);
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn plan_install_bytes(
    kitty_dir: &Path,
    existing_kitty_conf: Option<&[u8]>,
    existing_include: Option<&[u8]>,
    binary_path: &Path,
) -> Result<ConfigPlan, ConfigError> {
    let include_bytes = render_include_file(binary_path)?;
    let include_path = kitty_dir.join(INCLUDE_FILE_NAME);
    let kitty_conf_path = kitty_dir.join("kitty.conf");

    let existing_text = match existing_kitty_conf {
        Some(bytes) => Some(decode_kitty_conf(&kitty_conf_path, bytes)?),
        None => None,
    };

    let mut ops = Vec::new();
    if existing_include != Some(include_bytes.as_slice()) {
        ops.push(FileOp::Write {
            path: include_path,
            before: existing_include.map(ToOwned::to_owned),
            after: include_bytes,
        });
    }

    let already_included = existing_text.is_some_and(|text| {
        let lines: Vec<&str> = text.lines().collect();
        has_managed_block(&lines)
    });

    if !already_included {
        let updated = append_managed_block(existing_text);
        ops.push(FileOp::Write {
            path: kitty_conf_path,
            before: existing_kitty_conf.map(ToOwned::to_owned),
            after: updated,
        });
    }

    Ok(ConfigPlan {
        terminal: TerminalKind::Kitty,
        ops,
    })
}

/// Inspect `kitty_dir` on disk and plan installing OurioPicker wiring.
///
/// Refuses symlinked parent directories, symlinked targets, or read-only `kitty.conf`.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use ouro_terminal_picker::config::kitty::plan_install;
///
/// let plan = plan_install(Path::new("/tmp/kitty"), Path::new("/usr/local/bin/ouro-terminal-picker"))?;
/// println!("{}", plan.format_dry_run());
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn plan_install(kitty_dir: &Path, binary_path: &Path) -> Result<ConfigPlan, ConfigError> {
    ensure_no_symlink_ancestors(kitty_dir)?;
    let include_path = kitty_dir.join(INCLUDE_FILE_NAME);
    let kitty_conf_path = kitty_dir.join("kitty.conf");

    let existing_include = read_checked_target(&include_path, manual_include_snippet())?;
    let existing_kitty_conf = read_checked_target(&kitty_conf_path, manual_include_snippet())?;

    plan_install_bytes(
        kitty_dir,
        existing_kitty_conf.as_deref(),
        existing_include.as_deref(),
        binary_path,
    )
}

/// Plan removing OurioPicker wiring from `kitty_dir` using in-memory file contents.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ouro_terminal_picker::config::kitty::plan_uninstall_bytes;
///
/// let plan = plan_uninstall_bytes(
///     Path::new("/tmp/kitty"),
///     Some(b"font_size 12.0\n# ouro-terminal-picker\ninclude ouro-terminal-picker.conf\n"),
///     Some(b"# Managed by ouro-terminal-picker\nshell /usr/local/bin/ouro-terminal-picker\n"),
/// )?;
/// assert_eq!(plan.ops.len(), 2);
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn plan_uninstall_bytes(
    kitty_dir: &Path,
    existing_kitty_conf: Option<&[u8]>,
    existing_include: Option<&[u8]>,
) -> Result<ConfigPlan, ConfigError> {
    let include_path = kitty_dir.join(INCLUDE_FILE_NAME);
    let kitty_conf_path = kitty_dir.join("kitty.conf");
    let mut ops = Vec::new();

    if let Some(conf_bytes) = existing_kitty_conf {
        let conf_text = decode_kitty_conf(&kitty_conf_path, conf_bytes)?;
        if let Some(updated) = remove_managed_block(conf_text) {
            ops.push(FileOp::Write {
                path: kitty_conf_path,
                before: Some(conf_bytes.to_owned()),
                after: updated,
            });
        }
    }

    if let Some(inc_bytes) = existing_include {
        ops.push(FileOp::Remove {
            path: include_path,
            before: inc_bytes.to_owned(),
        });
    }

    Ok(ConfigPlan {
        terminal: TerminalKind::Kitty,
        ops,
    })
}

/// Inspect `kitty_dir` on disk and plan removing OurioPicker wiring.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use ouro_terminal_picker::config::kitty::plan_uninstall;
///
/// let plan = plan_uninstall(Path::new("/tmp/kitty"))?;
/// println!("{}", plan.format_dry_run());
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn plan_uninstall(kitty_dir: &Path) -> Result<ConfigPlan, ConfigError> {
    ensure_no_symlink_ancestors(kitty_dir)?;
    let include_path = kitty_dir.join(INCLUDE_FILE_NAME);
    let kitty_conf_path = kitty_dir.join("kitty.conf");

    let existing_include = read_checked_target(&include_path, manual_include_snippet())?;
    let existing_kitty_conf = read_checked_target(&kitty_conf_path, manual_include_snippet())?;

    plan_uninstall_bytes(
        kitty_dir,
        existing_kitty_conf.as_deref(),
        existing_include.as_deref(),
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use tempfile::TempDir;

    use crate::config::apply_plan;

    use super::*;

    #[test]
    fn resolves_kitty_config_dir_from_xdg_or_home() {
        assert_eq!(
            kitty_config_dir(Some(OsStr::new("/tmp/xdg")), Some(OsStr::new("/tmp/home"))).unwrap(),
            PathBuf::from("/tmp/xdg/kitty")
        );
        assert_eq!(
            kitty_config_dir(None, Some(OsStr::new("/tmp/home"))).unwrap(),
            PathBuf::from("/tmp/home/.config/kitty")
        );
        // Relative paths rejected per trust boundary
        assert!(kitty_config_dir(Some(OsStr::new("relative")), None).is_err());
    }

    #[test]
    fn install_and_uninstall_on_fresh_dir_and_existing_conf_restore_exact_bytes() {
        let dir = TempDir::new().unwrap();
        let kitty_dir = dir.path().join("kitty");
        fs::create_dir(&kitty_dir).unwrap();
        let conf_path = kitty_dir.join("kitty.conf");
        let inc_path = kitty_dir.join(INCLUDE_FILE_NAME);
        let original = b"# My kitty config\nfont_size 13.0\n";
        fs::write(&conf_path, original).unwrap();
        let mut perms = fs::metadata(&conf_path).unwrap().permissions();
        perms.set_mode(0o640);
        fs::set_permissions(&conf_path, perms).unwrap();

        let bin = Path::new("/usr/local/bin/ouro-terminal-picker");
        let install = plan_install(&kitty_dir, bin).unwrap();
        assert_eq!(install.ops.len(), 2);
        assert!(
            install
                .format_dry_run()
                .contains("include ouro-terminal-picker.conf")
        );
        apply_plan(&install).unwrap();

        // Mode preserved
        assert_eq!(
            fs::metadata(&conf_path).unwrap().permissions().mode() & 0o777,
            0o640
        );
        let after_install = fs::read_to_string(&conf_path).unwrap();
        assert_eq!(
            after_install,
            "# My kitty config\nfont_size 13.0\n# ouro-terminal-picker\ninclude ouro-terminal-picker.conf\n"
        );
        let inc_text = fs::read_to_string(&inc_path).unwrap();
        assert!(inc_text.contains("shell /usr/local/bin/ouro-terminal-picker\n"));

        // Repeat install is idempotent
        let second = plan_install(&kitty_dir, bin).unwrap();
        assert!(
            second.is_empty(),
            "repeat install must be no-op: {second:?}"
        );

        // Uninstall restores exact original bytes and removes include file
        let uninstall = plan_uninstall(&kitty_dir).unwrap();
        apply_plan(&uninstall).unwrap();
        assert_eq!(fs::read(&conf_path).unwrap(), original);
        assert!(!inc_path.exists());
    }

    #[test]
    fn uninstall_preserves_manual_edits_added_after_install() {
        let dir = TempDir::new().unwrap();
        let kitty_dir = dir.path().join("kitty");
        let bin = Path::new("/usr/local/bin/ouro-terminal-picker");
        apply_plan(&plan_install(&kitty_dir, bin).unwrap()).unwrap();

        let conf_path = kitty_dir.join("kitty.conf");
        let mut content = fs::read_to_string(&conf_path).unwrap();
        content.push_str("background #111111\n");
        fs::write(&conf_path, &content).unwrap();

        apply_plan(&plan_uninstall(&kitty_dir).unwrap()).unwrap();
        assert_eq!(
            fs::read_to_string(&conf_path).unwrap(),
            "background #111111\n"
        );
        assert!(!kitty_dir.join(INCLUDE_FILE_NAME).exists());
    }

    #[test]
    fn refuses_symlinked_kitty_conf_and_symlinked_parent_and_readonly_conf() {
        let dir = TempDir::new().unwrap();
        let bin = Path::new("/usr/local/bin/ouro-terminal-picker");

        // 1. Symlinked kitty.conf
        let kitty_dir = dir.path().join("kitty-sym");
        fs::create_dir(&kitty_dir).unwrap();
        let real_conf = dir.path().join("managed.conf");
        fs::write(&real_conf, "font_size 12\n").unwrap();
        symlink(&real_conf, kitty_dir.join("kitty.conf")).unwrap();
        let err = plan_install(&kitty_dir, bin).expect_err("symlinked kitty.conf must fail");
        assert!(matches!(err, ConfigError::SymlinkTarget { .. }));
        assert!(
            err.to_string()
                .contains("include ouro-terminal-picker.conf")
        );

        // 2. Symlinked parent directory
        let real_parent = dir.path().join("real-kitty");
        fs::create_dir(&real_parent).unwrap();
        let sym_parent = dir.path().join("sym-kitty");
        symlink(&real_parent, &sym_parent).unwrap();
        let err = plan_install(&sym_parent, bin).expect_err("symlinked parent must fail");
        assert!(matches!(err, ConfigError::SymlinkParent { .. }));

        // 3. Read-only kitty.conf
        let ro_dir = dir.path().join("kitty-ro");
        fs::create_dir(&ro_dir).unwrap();
        let ro_conf = ro_dir.join("kitty.conf");
        fs::write(&ro_conf, "font_size 12\n").unwrap();
        let mut perms = fs::metadata(&ro_conf).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&ro_conf, perms).unwrap();
        let err = plan_install(&ro_dir, bin).expect_err("read-only kitty.conf must fail");
        assert!(matches!(err, ConfigError::NotWritable { .. }));
        // Restore write bit for tempdir cleanup
        let mut perms = fs::metadata(&ro_conf).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&ro_conf, perms).unwrap();
    }

    #[test]
    fn f6_rejects_invalid_utf8_in_kitty_conf() {
        let bin = Path::new("/usr/local/bin/ouro-terminal-picker");
        let invalid = b"font_size 12\n\xff\xfe\n";
        let err_install = plan_install_bytes(Path::new("/tmp/kitty"), Some(invalid), None, bin)
            .expect_err("F6: invalid UTF-8 in kitty.conf must fail cleanly on install");
        assert!(matches!(err_install, ConfigError::InvalidUtf8 { .. }));
        let err_uninstall = plan_uninstall_bytes(Path::new("/tmp/kitty"), Some(invalid), None)
            .expect_err("F6: invalid UTF-8 in kitty.conf must fail cleanly on uninstall");
        assert!(matches!(err_uninstall, ConfigError::InvalidUtf8 { .. }));
    }
}
