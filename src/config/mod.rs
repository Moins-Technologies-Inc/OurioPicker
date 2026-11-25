//! Terminal configuration planners and atomic filesystem writers for Kitty and Rio/Ourio.

pub mod kitty;
pub mod rio;

use std::ffi::OsStr;
use std::fmt;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use thiserror::Error;

static TMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Supported terminal emulators for `install`, `uninstall`, and `print-config`.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::TerminalKind;
///
/// assert_eq!(TerminalKind::Kitty.as_str(), "kitty");
/// assert_eq!(TerminalKind::Rio.as_str(), "rio");
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalKind {
    /// Kitty terminal emulator (`kitty.conf` + `ouro-terminal-picker.conf`).
    Kitty,
    /// Rio / Ourio terminal emulator (`config.toml` `[shell]` table).
    Rio,
}

impl TerminalKind {
    /// Lowercase CLI identifier for this terminal.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::TerminalKind;
    ///
    /// assert_eq!(TerminalKind::Rio.as_str(), "rio");
    /// ```
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Kitty => "kitty",
            Self::Rio => "rio",
        }
    }
}

impl fmt::Display for TerminalKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Errors produced while planning or applying terminal configuration changes.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use ouro_terminal_picker::ConfigError;
///
/// let err = ConfigError::InvalidBinaryPath { path: PathBuf::from("relative/bin") };
/// assert!(err.to_string().contains("relative/bin"));
/// ```
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Binary path is not an absolute UTF-8 path without control characters.
    #[error("binary path must be absolute without control characters: {path}")]
    InvalidBinaryPath {
        /// Rejected path.
        path: PathBuf,
    },
    /// A parent directory along the target path is a symlink.
    #[error("refusing to follow symlinked parent directory: {path}")]
    SymlinkParent {
        /// Symlinked ancestor directory.
        path: PathBuf,
    },
    /// Target path contains a parent-directory traversal (`..`) component.
    #[error("refusing parent directory traversal (..): {path}")]
    ParentTraversal {
        /// Rejected path containing `..`.
        path: PathBuf,
    },
    /// Target path has no parent directory (for example `/` or an empty path).
    #[error("target path has no parent directory: {path}")]
    MissingParent {
        /// Rejected path without a parent directory.
        path: PathBuf,
    },
    /// Existing configuration file is not valid UTF-8.
    #[error("configuration file is not valid UTF-8 at {path}: {source}")]
    InvalidUtf8 {
        /// Path to the non-UTF-8 file.
        path: PathBuf,
        /// UTF-8 decoding error.
        #[source]
        source: std::str::Utf8Error,
    },
    /// Target configuration file is itself a symlink.
    #[error("refusing to edit symlinked config {path}; add manually:\n{manual_hint}")]
    SymlinkTarget {
        /// Symlinked config path.
        path: PathBuf,
        /// Exact lines the user can add manually.
        manual_hint: String,
    },
    /// Target configuration file is read-only / not writable.
    #[error("refusing to edit read-only config {path}; add manually:\n{manual_hint}")]
    NotWritable {
        /// Read-only config path.
        path: PathBuf,
        /// Exact lines the user can add manually.
        manual_hint: String,
    },
    /// Existing Rio `[shell]` table points to a different program and `--force` was not given.
    #[error(
        "existing [shell] table in {path} differs ({existing}); re-run with --force or configure manually:\n{manual_hint}"
    )]
    ForeignShell {
        /// Path to Rio's `config.toml`.
        path: PathBuf,
        /// Summary of the existing `[shell]` setting.
        existing: String,
        /// Manual TOML snippet.
        manual_hint: String,
    },
    /// Failed to parse an existing TOML configuration file.
    #[error("failed to parse TOML in {path}: {message}")]
    TomlParse {
        /// Path to the malformed file.
        path: PathBuf,
        /// Parser diagnostic.
        message: String,
    },
    /// Neither `$XDG_CONFIG_HOME` / terminal-specific env nor `$HOME` yielded an absolute path.
    #[error("could not determine an absolute configuration directory from environment")]
    NoConfigDir,
    /// Filesystem I/O failure.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// Path involved in the failing operation.
        path: PathBuf,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },
}

/// One atomic filesystem operation planned by `install` or `uninstall`.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use ouro_terminal_picker::FileOp;
///
/// let op = FileOp::Write {
///     path: PathBuf::from("/tmp/example.conf"),
///     before: None,
///     after: b"shell /usr/bin/env\n".to_vec(),
/// };
/// assert_eq!(op.path(), std::path::Path::new("/tmp/example.conf"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileOp {
    /// Create a private (`0600`) backup file before overwriting a foreign setting.
    Backup {
        /// Destination backup path.
        path: PathBuf,
        /// Exact bytes to save.
        content: Vec<u8>,
    },
    /// Atomically write `after` bytes to `path` (preserving original mode if present).
    Write {
        /// Target file path.
        path: PathBuf,
        /// Previous bytes, if the file existed.
        before: Option<Vec<u8>>,
        /// Exact replacement bytes.
        after: Vec<u8>,
    },
    /// Remove a file owned by `ouro-terminal-picker`.
    Remove {
        /// Path to remove.
        path: PathBuf,
        /// Bytes previously stored in the file.
        before: Vec<u8>,
    },
}

impl FileOp {
    /// Return the filesystem path affected by this operation.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::path::{Path, PathBuf};
    /// use ouro_terminal_picker::FileOp;
    ///
    /// let op = FileOp::Remove { path: PathBuf::from("/tmp/a"), before: Vec::new() };
    /// assert_eq!(op.path(), Path::new("/tmp/a"));
    /// ```
    pub fn path(&self) -> &Path {
        match self {
            Self::Backup { path, .. } | Self::Write { path, .. } | Self::Remove { path, .. } => {
                path
            }
        }
    }
}

/// Planned sequence of configuration edits returned by Kitty and Rio planners.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::{ConfigPlan, TerminalKind};
///
/// let plan = ConfigPlan { terminal: TerminalKind::Kitty, ops: Vec::new() };
/// assert!(plan.is_empty());
/// assert!(plan.format_dry_run().contains("no changes"));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigPlan {
    /// Target terminal emulator.
    pub terminal: TerminalKind,
    /// Ordered file operations to execute.
    pub ops: Vec<FileOp>,
}

impl ConfigPlan {
    /// Return `true` when applying this plan would make no filesystem changes.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::{ConfigPlan, TerminalKind};
    ///
    /// let plan = ConfigPlan { terminal: TerminalKind::Rio, ops: Vec::new() };
    /// assert!(plan.is_empty());
    /// ```
    pub fn is_empty(&self) -> bool {
        self.ops.is_empty()
    }

    /// Render a human-readable diff-like preview suitable for `--dry-run`.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::{ConfigPlan, TerminalKind};
    ///
    /// let plan = ConfigPlan { terminal: TerminalKind::Kitty, ops: Vec::new() };
    /// assert!(!plan.format_dry_run().is_empty());
    /// ```
    pub fn format_dry_run(&self) -> String {
        if self.ops.is_empty() {
            return format!("{}: no changes needed.\n", self.terminal);
        }
        let mut out = String::new();
        for op in &self.ops {
            match op {
                FileOp::Backup { path, .. } => {
                    out.push_str(&format!("backup {} (mode 0600)\n", path.display()));
                }
                FileOp::Write {
                    path,
                    before,
                    after,
                } => {
                    out.push_str(&format!("--- {}\n+++ {}\n", path.display(), path.display()));
                    let old_text = before
                        .as_deref()
                        .map(|b| String::from_utf8_lossy(b).into_owned())
                        .unwrap_or_default();
                    let new_text = String::from_utf8_lossy(after);
                    let old_lines: Vec<&str> = old_text.lines().collect();
                    let new_lines: Vec<&str> = new_text.lines().collect();
                    append_line_diff(&mut out, &old_lines, &new_lines);
                }
                FileOp::Remove { path, before } => {
                    out.push_str(&format!("remove {}\n", path.display()));
                    for line in String::from_utf8_lossy(before).lines() {
                        out.push_str(&format!("-{line}\n"));
                    }
                }
            }
        }
        out
    }
}

const MAX_LCS_LINES: usize = 2048;

fn append_line_diff(out: &mut String, old_lines: &[&str], new_lines: &[&str]) {
    let n = old_lines.len();
    let m = new_lines.len();
    if n == 0 {
        for line in new_lines {
            out.push_str(&format!("+{line}\n"));
        }
        return;
    }
    if m == 0 {
        for line in old_lines {
            out.push_str(&format!("-{line}\n"));
        }
        return;
    }
    if n > MAX_LCS_LINES || m > MAX_LCS_LINES {
        for line in old_lines {
            out.push_str(&format!("-{line}\n"));
        }
        for line in new_lines {
            out.push_str(&format!("+{line}\n"));
        }
        return;
    }

    let cols = m + 1;
    let mut dp = vec![0usize; (n + 1) * cols];
    for i in (0..n).rev() {
        let old_line = old_lines.get(i).copied();
        for j in (0..m).rev() {
            let val = if old_line == new_lines.get(j).copied() {
                dp.get((i + 1) * cols + (j + 1)).copied().unwrap_or(0) + 1
            } else {
                let down = dp.get((i + 1) * cols + j).copied().unwrap_or(0);
                let right = dp.get(i * cols + (j + 1)).copied().unwrap_or(0);
                down.max(right)
            };
            if let Some(slot) = dp.get_mut(i * cols + j) {
                *slot = val;
            }
        }
    }

    let mut i = 0usize;
    let mut j = 0usize;
    while i < n && j < m {
        let ol = old_lines.get(i).copied().unwrap_or("");
        let nl = new_lines.get(j).copied().unwrap_or("");
        if ol == nl {
            i += 1;
            j += 1;
        } else {
            let down = dp.get((i + 1) * cols + j).copied().unwrap_or(0);
            let right = dp.get(i * cols + (j + 1)).copied().unwrap_or(0);
            if down >= right {
                out.push_str(&format!("-{ol}\n"));
                i += 1;
            } else {
                out.push_str(&format!("+{nl}\n"));
                j += 1;
            }
        }
    }
    for ol in old_lines.iter().skip(i) {
        out.push_str(&format!("-{ol}\n"));
    }
    for nl in new_lines.iter().skip(j) {
        out.push_str(&format!("+{nl}\n"));
    }
}

/// Validate that `binary_path` is an absolute UTF-8 path free of control characters.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ouro_terminal_picker::config::validate_binary_path;
///
/// assert_eq!(
///     validate_binary_path(Path::new("/usr/local/bin/ouro-terminal-picker")).unwrap(),
///     "/usr/local/bin/ouro-terminal-picker"
/// );
/// assert!(validate_binary_path(Path::new("relative/bin")).is_err());
/// ```
pub fn validate_binary_path(binary_path: &Path) -> Result<&str, ConfigError> {
    if !binary_path.is_absolute() {
        return Err(ConfigError::InvalidBinaryPath {
            path: binary_path.to_path_buf(),
        });
    }
    let Some(text) = binary_path.to_str() else {
        return Err(ConfigError::InvalidBinaryPath {
            path: binary_path.to_path_buf(),
        });
    };
    if text.is_empty() || text.chars().any(char::is_control) {
        return Err(ConfigError::InvalidBinaryPath {
            path: binary_path.to_path_buf(),
        });
    }
    Ok(text)
}

/// Verify that no existing ancestor component of `dir` is a symbolic link.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ouro_terminal_picker::config::ensure_no_symlink_ancestors;
///
/// assert!(ensure_no_symlink_ancestors(Path::new("/")).is_ok());
/// ```
pub fn ensure_no_symlink_ancestors(dir: &Path) -> Result<(), ConfigError> {
    if dir
        .components()
        .any(|comp| matches!(comp, Component::ParentDir))
    {
        return Err(ConfigError::ParentTraversal {
            path: dir.to_path_buf(),
        });
    }

    let mut current = PathBuf::new();
    for comp in dir.components() {
        match comp {
            Component::RootDir => current.push(Path::new("/")),
            Component::Normal(part) => {
                current.push(part);
                match fs::symlink_metadata(&current) {
                    Ok(meta) => {
                        if meta.file_type().is_symlink() {
                            return Err(ConfigError::SymlinkParent { path: current });
                        }
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                        // Non-existent suffix components will be created fresh.
                        break;
                    }
                    Err(source) => {
                        return Err(ConfigError::Io {
                            path: current,
                            source,
                        });
                    }
                }
            }
            Component::ParentDir => {
                return Err(ConfigError::ParentTraversal {
                    path: dir.to_path_buf(),
                });
            }
            Component::CurDir | Component::Prefix(_) => {
                current.push(comp.as_os_str());
            }
        }
    }
    Ok(())
}

pub(crate) fn absolute_env_path(val: Option<&OsStr>) -> Option<PathBuf> {
    let raw = val?;
    if raw.is_empty() {
        return None;
    }
    let path = PathBuf::from(raw);
    if path.is_absolute() { Some(path) } else { None }
}

pub(crate) fn read_checked_target(
    path: &Path,
    manual_hint: &str,
) -> Result<Option<Vec<u8>>, ConfigError> {
    if let Some(parent) = path.parent() {
        ensure_no_symlink_ancestors(parent)?;
    }
    match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(ConfigError::SymlinkTarget {
                    path: path.to_path_buf(),
                    manual_hint: manual_hint.to_owned(),
                });
            }
            if (meta.permissions().mode() & 0o200) == 0 {
                return Err(ConfigError::NotWritable {
                    path: path.to_path_buf(),
                    manual_hint: manual_hint.to_owned(),
                });
            }
            let bytes = fs::read(path).map_err(|source| ConfigError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            Ok(Some(bytes))
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(ConfigError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn atomic_write(path: &Path, content: &[u8], default_mode: u32) -> Result<(), ConfigError> {
    let Some(parent) = path.parent() else {
        return Err(ConfigError::MissingParent {
            path: path.to_path_buf(),
        });
    };
    if parent.as_os_str().is_empty() {
        return Err(ConfigError::MissingParent {
            path: path.to_path_buf(),
        });
    }
    ensure_no_symlink_ancestors(parent)?;
    fs::create_dir_all(parent).map_err(|source| ConfigError::Io {
        path: parent.to_path_buf(),
        source,
    })?;

    let existing_mode = match fs::symlink_metadata(path) {
        Ok(meta) => {
            if meta.file_type().is_symlink() {
                return Err(ConfigError::SymlinkTarget {
                    path: path.to_path_buf(),
                    manual_hint: String::new(),
                });
            }
            let mode = meta.permissions().mode();
            if (mode & 0o200) == 0 {
                return Err(ConfigError::NotWritable {
                    path: path.to_path_buf(),
                    manual_hint: String::new(),
                });
            }
            Some(mode & 0o777)
        }
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(ConfigError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };

    let mode = existing_mode.unwrap_or(default_mode);
    let seq = TMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let tmp_name = format!(".ouro-terminal-picker.tmp.{}.{}", std::process::id(), seq);
    let tmp_path = parent.join(tmp_name);

    let write_res = (|| -> Result<(), ConfigError> {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(mode)
            .open(&tmp_path)
            .map_err(|source| ConfigError::Io {
                path: tmp_path.clone(),
                source,
            })?;
        file.set_permissions(fs::Permissions::from_mode(mode))
            .map_err(|source| ConfigError::Io {
                path: tmp_path.clone(),
                source,
            })?;
        file.write_all(content).map_err(|source| ConfigError::Io {
            path: tmp_path.clone(),
            source,
        })?;
        file.sync_all().map_err(|source| ConfigError::Io {
            path: tmp_path.clone(),
            source,
        })?;
        fs::rename(&tmp_path, path).map_err(|source| ConfigError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        Ok(())
    })();

    if write_res.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    write_res
}

/// Apply all operations in `plan` atomically, refusing symlinked parents or targets.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::{ConfigPlan, TerminalKind, apply_plan};
///
/// let plan = ConfigPlan { terminal: TerminalKind::Kitty, ops: Vec::new() };
/// apply_plan(&plan)?;
/// # Ok::<(), ouro_terminal_picker::ConfigError>(())
/// ```
pub fn apply_plan(plan: &ConfigPlan) -> Result<(), ConfigError> {
    for op in &plan.ops {
        match op {
            FileOp::Backup { path, content } => {
                atomic_write(path, content, 0o600)?;
                tracing::info!(
                    terminal = plan.terminal.as_str(),
                    path = %path.display(),
                    "config backup written"
                );
            }
            FileOp::Write { path, after, .. } => {
                atomic_write(path, after, 0o644)?;
                tracing::info!(
                    terminal = plan.terminal.as_str(),
                    path = %path.display(),
                    "config written"
                );
            }
            FileOp::Remove { path, .. } => {
                if let Some(parent) = path.parent() {
                    ensure_no_symlink_ancestors(parent)?;
                }
                match fs::symlink_metadata(path) {
                    Ok(meta) if meta.file_type().is_symlink() => {
                        return Err(ConfigError::SymlinkTarget {
                            path: path.to_path_buf(),
                            manual_hint: String::new(),
                        });
                    }
                    Ok(_) => {
                        fs::remove_file(path).map_err(|source| ConfigError::Io {
                            path: path.to_path_buf(),
                            source,
                        })?;
                        tracing::info!(
                            terminal = plan.terminal.as_str(),
                            path = %path.display(),
                            "config removed"
                        );
                    }
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                    Err(source) => {
                        return Err(ConfigError::Io {
                            path: path.to_path_buf(),
                            source,
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use tempfile::TempDir;

    use super::*;

    #[test]
    fn f4_ensure_no_symlink_ancestors_rejects_parent_dir_traversal() {
        let dir = TempDir::new().unwrap();
        let traversal = dir.path().join("sub").join("..").join("other");
        let err = ensure_no_symlink_ancestors(&traversal)
            .expect_err("Component::ParentDir must be rejected");
        assert!(
            err.to_string().contains(".."),
            "unexpected error for parent traversal: {err}"
        );

        let missing_then_traversal = dir.path().join("does-not-exist").join("..").join("other");
        assert!(
            ensure_no_symlink_ancestors(&missing_then_traversal).is_err(),
            "ParentDir after non-existent prefix must also be rejected"
        );
    }

    #[test]
    fn f5_atomic_write_rechecks_readonly_target_and_rejects_missing_parent() {
        let dir = TempDir::new().unwrap();
        let ro_file = dir.path().join("readonly.conf");
        fs::write(&ro_file, b"old\n").unwrap();
        let mut perms = fs::metadata(&ro_file).unwrap().permissions();
        perms.set_mode(0o444);
        fs::set_permissions(&ro_file, perms).unwrap();

        let plan = ConfigPlan {
            terminal: TerminalKind::Kitty,
            ops: vec![FileOp::Write {
                path: ro_file.clone(),
                before: Some(b"old\n".to_vec()),
                after: b"new\n".to_vec(),
            }],
        };
        let err = apply_plan(&plan).expect_err("apply_plan must refuse read-only target");
        let mut perms = fs::metadata(&ro_file).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&ro_file, perms).unwrap();
        assert!(
            matches!(err, ConfigError::NotWritable { .. }),
            "expected NotWritable, got {err:?}"
        );

        let no_parent_plan = ConfigPlan {
            terminal: TerminalKind::Kitty,
            ops: vec![FileOp::Write {
                path: PathBuf::from("/"),
                before: None,
                after: b"x\n".to_vec(),
            }],
        };
        let err = apply_plan(&no_parent_plan).expect_err("root path has no parent");
        assert!(
            matches!(err, ConfigError::MissingParent { .. }),
            "expected MissingParent, got {err:?}"
        );
    }

    #[test]
    fn f9_format_dry_run_shows_added_duplicate_lines() {
        let plan = ConfigPlan {
            terminal: TerminalKind::Kitty,
            ops: vec![FileOp::Write {
                path: PathBuf::from("/tmp/example.conf"),
                before: Some(b"alpha\nbeta\n".to_vec()),
                after: b"alpha\nbeta\nalpha\n".to_vec(),
            }],
        };
        let diff = plan.format_dry_run();
        assert!(
            diff.contains("+alpha\n"),
            "F9: real line diff must show added duplicate line '+alpha':\n{diff}"
        );
    }
}
