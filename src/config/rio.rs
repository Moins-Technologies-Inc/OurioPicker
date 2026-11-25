//! Rio / Ourio configuration planner (`config.toml` `[shell]` table via `toml_edit`).

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use toml_edit::{Array, DocumentMut, Item, Table, value};

use crate::config::{
    ConfigError, ConfigPlan, FileOp, TerminalKind, absolute_env_path, ensure_no_symlink_ancestors,
    read_checked_target, validate_binary_path,
};

/// Resolve the Rio/Ourio config file path per `rio.5`:
/// `$RIO_CONFIG_HOME/config.toml`, else `$XDG_CONFIG_HOME/rio/config.toml`,
/// else `$HOME/.config/rio/config.toml`.
///
/// # Examples
///
/// ```
/// use std::ffi::OsStr;
/// use std::path::PathBuf;
/// use ouro_terminal_picker::config::rio::rio_config_path;
///
/// let p = rio_config_path(Some(OsStr::new("/tmp/rio")), None, None)?;
/// assert_eq!(p, PathBuf::from("/tmp/rio/config.toml"));
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn rio_config_path(
    rio_config_home: Option<&OsStr>,
    xdg_config_home: Option<&OsStr>,
    home: Option<&OsStr>,
) -> Result<PathBuf, ConfigError> {
    if let Some(rio_home) = absolute_env_path(rio_config_home) {
        return Ok(rio_home.join("config.toml"));
    }
    if let Some(xdg) = absolute_env_path(xdg_config_home) {
        return Ok(xdg.join("rio").join("config.toml"));
    }
    if let Some(h) = absolute_env_path(home) {
        return Ok(h.join(".config").join("rio").join("config.toml"));
    }
    Err(ConfigError::NoConfigDir)
}

/// Render the manual `[shell]` TOML snippet for `binary_path`.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ouro_terminal_picker::config::rio::render_shell_snippet;
///
/// let toml = render_shell_snippet(Path::new("/usr/local/bin/ouro-terminal-picker"))?;
/// assert!(toml.contains("[shell]"));
/// assert!(toml.contains("program = \"/usr/local/bin/ouro-terminal-picker\""));
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn render_shell_snippet(binary_path: &Path) -> Result<String, ConfigError> {
    let bin_str = validate_binary_path(binary_path)?;
    let mut doc = DocumentMut::new();
    let mut table = Table::new();
    table.insert("program", value(bin_str));
    table.insert("args", value(Array::new()));
    doc.insert("shell", Item::Table(table));
    Ok(doc.to_string())
}

fn shell_matches_ours(item: &Item, bin_str: &str) -> bool {
    let Some(table) = item.as_table_like() else {
        return false;
    };
    let program_ok = table
        .get("program")
        .and_then(Item::as_str)
        .is_some_and(|p| p == bin_str);
    if !program_ok {
        return false;
    }
    match table.get("args") {
        None => true,
        Some(args_item) => args_item.as_array().is_some_and(Array::is_empty),
    }
}

fn summarize_shell_item(item: &Item) -> String {
    if let Some(table) = item.as_table_like() {
        let program = table
            .get("program")
            .and_then(Item::as_str)
            .unwrap_or("<non-string>");
        return format!("program = {program:?}");
    }
    item.to_string().trim().to_owned()
}

fn backup_path_for(config_path: &Path, timestamp: u64) -> PathBuf {
    let name = config_path
        .file_name()
        .and_then(OsStr::to_str)
        .unwrap_or("config.toml");
    config_path.with_file_name(format!("{name}.ouro-terminal-picker.bak.{timestamp}"))
}

fn parse_doc(config_path: &Path, existing: Option<&[u8]>) -> Result<DocumentMut, ConfigError> {
    let Some(bytes) = existing else {
        return Ok(DocumentMut::new());
    };
    let text = std::str::from_utf8(bytes).map_err(|err| ConfigError::TomlParse {
        path: config_path.to_path_buf(),
        message: err.to_string(),
    })?;
    text.parse::<DocumentMut>()
        .map_err(|err| ConfigError::TomlParse {
            path: config_path.to_path_buf(),
            message: err.to_string(),
        })
}

/// Plan installing `[shell]` into a Rio/Ourio `config.toml` using in-memory bytes.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ouro_terminal_picker::config::rio::plan_install_bytes;
///
/// let plan = plan_install_bytes(
///     Path::new("/tmp/rio/config.toml"),
///     Some(b"[window]\nwidth = 900\n"),
///     Path::new("/usr/local/bin/ouro-terminal-picker"),
///     false,
///     1_700_000_000,
/// )?;
/// assert_eq!(plan.ops.len(), 1);
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn plan_install_bytes(
    config_path: &Path,
    existing: Option<&[u8]>,
    binary_path: &Path,
    force: bool,
    backup_timestamp: u64,
) -> Result<ConfigPlan, ConfigError> {
    let bin_str = validate_binary_path(binary_path)?;
    let manual_hint = render_shell_snippet(binary_path)?;
    let mut doc = parse_doc(config_path, existing)?;
    let mut ops = Vec::new();

    if let Some(shell_item) = doc.get("shell") {
        if shell_matches_ours(shell_item, bin_str) {
            return Ok(ConfigPlan {
                terminal: TerminalKind::Rio,
                ops,
            });
        }
        if !force {
            return Err(ConfigError::ForeignShell {
                path: config_path.to_path_buf(),
                existing: summarize_shell_item(shell_item),
                manual_hint,
            });
        }
        if let Some(orig) = existing {
            ops.push(FileOp::Backup {
                path: backup_path_for(config_path, backup_timestamp),
                content: orig.to_owned(),
            });
        }
    }

    let mut table = Table::new();
    table.insert("program", value(bin_str));
    table.insert("args", value(Array::new()));
    doc.insert("shell", Item::Table(table));

    let after = doc.to_string().into_bytes();
    ops.push(FileOp::Write {
        path: config_path.to_path_buf(),
        before: existing.map(ToOwned::to_owned),
        after,
    });

    Ok(ConfigPlan {
        terminal: TerminalKind::Rio,
        ops,
    })
}

/// Inspect `config_path` on disk and plan installing OurioPicker into Rio/Ourio.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use ouro_terminal_picker::config::rio::plan_install;
///
/// let plan = plan_install(
///     Path::new("/tmp/rio/config.toml"),
///     Path::new("/usr/local/bin/ouro-terminal-picker"),
///     false,
/// )?;
/// println!("{}", plan.format_dry_run());
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn plan_install(
    config_path: &Path,
    binary_path: &Path,
    force: bool,
) -> Result<ConfigPlan, ConfigError> {
    let manual_hint = render_shell_snippet(binary_path)?;
    if let Some(parent) = config_path.parent() {
        ensure_no_symlink_ancestors(parent)?;
    }
    let existing = read_checked_target(config_path, &manual_hint)?;
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    plan_install_bytes(config_path, existing.as_deref(), binary_path, force, ts)
}

/// Plan removing `[shell]` from Rio/Ourio `config.toml` using in-memory bytes.
///
/// Removes `[shell]` only when it still equals our `binary_path` with empty `args`.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ouro_terminal_picker::config::rio::plan_uninstall_bytes;
///
/// let existing = b"[shell]\nprogram = \"/usr/local/bin/ouro-terminal-picker\"\nargs = []\n";
/// let plan = plan_uninstall_bytes(
///     Path::new("/tmp/rio/config.toml"),
///     Some(existing),
///     Path::new("/usr/local/bin/ouro-terminal-picker"),
/// )?;
/// assert_eq!(plan.ops.len(), 1);
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn plan_uninstall_bytes(
    config_path: &Path,
    existing: Option<&[u8]>,
    binary_path: &Path,
) -> Result<ConfigPlan, ConfigError> {
    let bin_str = validate_binary_path(binary_path)?;
    let Some(orig) = existing else {
        return Ok(ConfigPlan {
            terminal: TerminalKind::Rio,
            ops: Vec::new(),
        });
    };

    let mut doc = parse_doc(config_path, Some(orig))?;
    let matches = doc
        .get("shell")
        .is_some_and(|item| shell_matches_ours(item, bin_str));
    if !matches {
        return Ok(ConfigPlan {
            terminal: TerminalKind::Rio,
            ops: Vec::new(),
        });
    }

    let _ = doc.remove("shell");
    let after = doc.to_string().into_bytes();
    Ok(ConfigPlan {
        terminal: TerminalKind::Rio,
        ops: vec![FileOp::Write {
            path: config_path.to_path_buf(),
            before: Some(orig.to_owned()),
            after,
        }],
    })
}

/// Inspect `config_path` on disk and plan removing OurioPicker from Rio/Ourio.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use ouro_terminal_picker::config::rio::plan_uninstall;
///
/// let plan = plan_uninstall(
///     Path::new("/tmp/rio/config.toml"),
///     Path::new("/usr/local/bin/ouro-terminal-picker"),
/// )?;
/// println!("{}", plan.format_dry_run());
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn plan_uninstall(config_path: &Path, binary_path: &Path) -> Result<ConfigPlan, ConfigError> {
    let manual_hint = render_shell_snippet(binary_path)?;
    if let Some(parent) = config_path.parent() {
        ensure_no_symlink_ancestors(parent)?;
    }
    let existing = read_checked_target(config_path, &manual_hint)?;
    plan_uninstall_bytes(config_path, existing.as_deref(), binary_path)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::{PermissionsExt, symlink};

    use tempfile::TempDir;

    use crate::config::{FileOp, apply_plan};

    use super::*;

    #[test]
    fn resolves_rio_config_path_in_rio5_precedence_order() {
        assert_eq!(
            rio_config_path(
                Some(OsStr::new("/tmp/rio-home")),
                Some(OsStr::new("/tmp/xdg")),
                Some(OsStr::new("/tmp/home")),
            )
            .unwrap(),
            PathBuf::from("/tmp/rio-home/config.toml")
        );
        assert_eq!(
            rio_config_path(
                None,
                Some(OsStr::new("/tmp/xdg")),
                Some(OsStr::new("/tmp/home")),
            )
            .unwrap(),
            PathBuf::from("/tmp/xdg/rio/config.toml")
        );
        assert_eq!(
            rio_config_path(None, None, Some(OsStr::new("/tmp/home"))).unwrap(),
            PathBuf::from("/tmp/home/.config/rio/config.toml")
        );
        assert!(rio_config_path(Some(OsStr::new("relative")), None, None).is_err());
    }

    #[test]
    fn install_preserves_comments_is_idempotent_and_uninstall_restores_exact_bytes() {
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("rio").join("config.toml");
        fs::create_dir(cfg.parent().unwrap()).unwrap();
        let original = b"# Rio user config\n\n[window]\nwidth = 1200 # keep comment\n";
        fs::write(&cfg, original).unwrap();

        let bin = Path::new("/usr/local/bin/ouro-terminal-picker");
        let plan = plan_install(&cfg, bin, false).unwrap();
        assert_eq!(plan.ops.len(), 1);
        apply_plan(&plan).unwrap();

        let installed = fs::read_to_string(&cfg).unwrap();
        assert!(installed.contains("# Rio user config"));
        assert!(installed.contains("width = 1200 # keep comment"));
        assert!(installed.contains("[shell]"));
        assert!(installed.contains("program = \"/usr/local/bin/ouro-terminal-picker\""));
        assert!(installed.contains("args = []"));

        // Repeat install is idempotent
        let repeat = plan_install(&cfg, bin, false).unwrap();
        assert!(
            repeat.is_empty(),
            "repeat install must be no-op: {repeat:?}"
        );

        // Uninstall restores exact original bytes
        let uninstall = plan_uninstall(&cfg, bin).unwrap();
        apply_plan(&uninstall).unwrap();
        assert_eq!(fs::read(&cfg).unwrap(), original);
    }

    #[test]
    fn refuses_foreign_shell_unless_force_and_backs_up_with_0600_on_force() {
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("config.toml");
        let foreign = b"[shell]\nprogram = \"/usr/bin/fish\"\nargs = [\"--login\"]\n";
        fs::write(&cfg, foreign).unwrap();
        let bin = Path::new("/usr/local/bin/ouro-terminal-picker");

        let err =
            plan_install(&cfg, bin, false).expect_err("foreign [shell] must fail without force");
        assert!(matches!(err, ConfigError::ForeignShell { .. }));
        assert!(err.to_string().contains("/usr/bin/fish"));

        let forced = plan_install(&cfg, bin, true).unwrap();
        assert_eq!(forced.ops.len(), 2);
        let backup_path = match &forced.ops[0] {
            FileOp::Backup { path, content } => {
                assert_eq!(content, foreign);
                path.clone()
            }
            other => panic!("expected Backup first, got {other:?}"),
        };
        apply_plan(&forced).unwrap();

        assert!(backup_path.exists());
        assert_eq!(
            fs::metadata(&backup_path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let updated = fs::read_to_string(&cfg).unwrap();
        assert!(updated.contains("program = \"/usr/local/bin/ouro-terminal-picker\""));
        assert!(updated.contains("args = []"));
    }

    #[test]
    fn uninstall_leaves_manually_edited_shell_untouched() {
        let dir = TempDir::new().unwrap();
        let cfg = dir.path().join("config.toml");
        let edited = b"[shell]\nprogram = \"/usr/bin/zsh\"\nargs = []\n";
        fs::write(&cfg, edited).unwrap();
        let bin = Path::new("/usr/local/bin/ouro-terminal-picker");

        let plan = plan_uninstall(&cfg, bin).unwrap();
        assert!(plan.is_empty(), "foreign [shell] must not be removed");
        assert_eq!(fs::read(&cfg).unwrap(), edited);
    }

    #[test]
    fn refuses_symlinked_rio_config_and_symlinked_parent() {
        let dir = TempDir::new().unwrap();
        let bin = Path::new("/usr/local/bin/ouro-terminal-picker");

        let real = dir.path().join("real.toml");
        fs::write(&real, "").unwrap();
        let sym = dir.path().join("config.toml");
        symlink(&real, &sym).unwrap();
        let err = plan_install(&sym, bin, false).expect_err("symlink target must fail");
        assert!(matches!(err, ConfigError::SymlinkTarget { .. }));

        let real_dir = dir.path().join("real-dir");
        fs::create_dir(&real_dir).unwrap();
        let sym_dir = dir.path().join("sym-dir");
        symlink(&real_dir, &sym_dir).unwrap();
        let err = plan_install(&sym_dir.join("config.toml"), bin, false)
            .expect_err("symlink parent must fail");
        assert!(matches!(err, ConfigError::SymlinkParent { .. }));
    }
}
