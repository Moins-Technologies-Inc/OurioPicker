//! Choice representation, argv construction, shell resolution, and execution.

use std::env;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::path::{Path, PathBuf};
use std::process::Command;

use thiserror::Error;

use crate::name::SessionName;

/// Default POSIX fallback shell when `$SHELL` and `/etc/passwd` do not yield an executable shell.
pub const FALLBACK_SHELL: &str = "/bin/sh";

/// Action selected by the user in the session picker.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::{Choice, SessionName};
///
/// let attach = Choice::Attach(SessionName::new("work")?);
/// assert_eq!(attach.kind(), "attach");
/// assert_eq!(attach.session_name_len(), Some(4));
/// assert_eq!(Choice::Shell.kind(), "shell");
/// # Ok::<(), ouro_terminal_picker::NameError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// Attach to (or resurrect) an existing Zellij session.
    Attach(SessionName),
    /// Create a new Zellij session, optionally with a user-supplied name.
    Create(Option<SessionName>),
    /// Start the user's plain login/interactive shell without Zellij.
    Shell,
}

impl Choice {
    /// Return a static label describing the choice kind for structured logging.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::Choice;
    ///
    /// assert_eq!(Choice::Create(None).kind(), "create");
    /// ```
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Attach(_) => "attach",
            Self::Create(_) => "create",
            Self::Shell => "shell",
        }
    }

    /// Return the length of the chosen session name, if any (never logs the name itself).
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::{Choice, SessionName};
    ///
    /// let choice = Choice::Create(Some(SessionName::new("dev")?));
    /// assert_eq!(choice.session_name_len(), Some(3));
    /// # Ok::<(), ouro_terminal_picker::NameError>(())
    /// ```
    pub fn session_name_len(&self) -> Option<usize> {
        match self {
            Self::Attach(name) | Self::Create(Some(name)) => Some(name.as_str().len()),
            Self::Create(None) | Self::Shell => None,
        }
    }
}

/// Errors returned while launching Zellij or replacing the process with a shell.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use ouro_terminal_picker::LaunchError;
///
/// let err = LaunchError::ExecShell {
///     shell: PathBuf::from("/bin/sh"),
///     source: std::io::Error::from(std::io::ErrorKind::NotFound),
/// };
/// assert!(err.to_string().contains("/bin/sh"));
/// ```
#[derive(Debug, Error)]
pub enum LaunchError {
    /// Failed to spawn Zellij.
    #[error("failed to start zellij: {source}")]
    SpawnZellij {
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },
    /// Failed to exec the resolved shell.
    #[error("failed to exec shell {shell}: {source}")]
    ExecShell {
        /// Shell path that could not be executed.
        shell: PathBuf,
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },
}

/// Build the `zellij` [`Command`] for a picker [`Choice`], or `None` for [`Choice::Shell`].
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::{Choice, SessionName, zellij_command};
///
/// let cmd = zellij_command(&Choice::Attach(SessionName::new("work")?)).unwrap();
/// let args: Vec<_> = cmd.get_args().map(|a| a.to_string_lossy()).collect();
/// assert_eq!(args, ["attach", "work", "options", "--on-force-close", "detach"]);
/// assert!(zellij_command(&Choice::Shell).is_none());
/// # Ok::<(), ouro_terminal_picker::NameError>(())
/// ```
pub fn zellij_command(choice: &Choice) -> Option<Command> {
    zellij_command_with(Path::new("zellij"), choice)
}

/// Build the `zellij` [`Command`] using an explicit `zellij_bin` path.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ouro_terminal_picker::{Choice, zellij_command_with};
///
/// let cmd = zellij_command_with(Path::new("/usr/bin/zellij"), &Choice::Create(None)).unwrap();
/// assert_eq!(cmd.get_program(), Path::new("/usr/bin/zellij"));
/// ```
pub fn zellij_command_with(zellij_bin: &Path, choice: &Choice) -> Option<Command> {
    let mut cmd = Command::new(zellij_bin);
    match choice {
        Choice::Attach(name) => {
            cmd.args([
                "attach",
                name.as_str(),
                "options",
                "--on-force-close",
                "detach",
            ]);
            Some(cmd)
        }
        Choice::Create(Some(name)) => {
            cmd.args([
                "attach",
                "--create",
                name.as_str(),
                "options",
                "--on-force-close",
                "detach",
            ]);
            Some(cmd)
        }
        Choice::Create(None) => {
            cmd.args(["options", "--on-force-close", "detach"]);
            Some(cmd)
        }
        Choice::Shell => None,
    }
}

/// Check whether `path` is an absolute path to a regular file with any execute bit set.
///
/// # Examples
///
/// ```
/// use std::path::Path;
/// use ouro_terminal_picker::is_executable_file;
///
/// assert!(!is_executable_file(Path::new("relative/bin")));
/// ```
pub fn is_executable_file(path: &Path) -> bool {
    if !path.is_absolute() {
        return false;
    }
    let Ok(meta) = fs::metadata(path) else {
        return false;
    };
    meta.is_file() && (meta.permissions().mode() & 0o111) != 0
}

/// Locate an executable named `program` inside the absolute directories of `path_env`.
///
/// # Examples
///
/// ```
/// use std::ffi::OsStr;
/// use ouro_terminal_picker::find_in_path;
///
/// assert!(find_in_path("definitely-missing-binary-xyz", Some(OsStr::new("/bin:/usr/bin"))).is_none());
/// ```
pub fn find_in_path(program: &str, path_env: Option<&OsStr>) -> Option<PathBuf> {
    if program.is_empty() || program.contains('/') {
        return None;
    }
    let path_env = path_env?;
    for dir in env::split_paths(path_env) {
        if !dir.is_absolute() {
            continue;
        }
        let candidate = dir.join(program);
        if is_executable_file(&candidate) {
            return Some(candidate);
        }
    }
    None
}

/// Decide whether the picker must be bypassed directly to the user's shell.
///
/// # Examples
///
/// ```
/// use std::ffi::OsStr;
/// use std::path::Path;
/// use ouro_terminal_picker::should_bypass_picker;
///
/// assert!(should_bypass_picker(Some(OsStr::new("0")), Some(Path::new("/usr/bin/zellij")), true));
/// assert!(should_bypass_picker(None, None, true));
/// assert!(should_bypass_picker(None, Some(Path::new("/usr/bin/zellij")), false));
/// assert!(!should_bypass_picker(None, Some(Path::new("/usr/bin/zellij")), true));
/// ```
pub fn should_bypass_picker(
    zellij_env: Option<&OsStr>,
    zellij_bin: Option<&Path>,
    is_tty: bool,
) -> bool {
    let inside_zellij = zellij_env.is_some_and(|v| !v.is_empty());
    inside_zellij || zellij_bin.is_none() || !is_tty
}

/// Parse the login shell for `uid` from `/etc/passwd`-formatted text.
///
/// # Examples
///
/// ```
/// use std::path::PathBuf;
/// use ouro_terminal_picker::parse_passwd_shell;
///
/// let entries = "root:x:0:0:root:/root:/bin/bash\nalice:x:1000:1000::/srv/alice:/usr/bin/zsh\n";
/// assert_eq!(parse_passwd_shell(entries, 1000), Some(PathBuf::from("/usr/bin/zsh")));
/// ```
pub fn parse_passwd_shell(passwd_content: &str, uid: u32) -> Option<PathBuf> {
    for line in passwd_content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        let mut fields = trimmed.split(':');
        let Some(_name) = fields.next() else {
            continue;
        };
        let Some(_pw) = fields.next() else {
            continue;
        };
        let Some(uid_str) = fields.next() else {
            continue;
        };
        let Ok(entry_uid) = uid_str.parse::<u32>() else {
            continue;
        };
        if entry_uid != uid {
            continue;
        }
        let Some(_gid) = fields.next() else {
            continue;
        };
        let Some(_gecos) = fields.next() else {
            continue;
        };
        let Some(_home) = fields.next() else {
            continue;
        };
        let Some(raw_shell) = fields.next() else {
            continue;
        };
        let shell = raw_shell.trim();
        if !shell.is_empty() {
            return Some(PathBuf::from(shell));
        }
    }
    None
}

/// Resolve the user's shell from injectable inputs (for deterministic testing).
///
/// Order: `$SHELL` if absolute and executable, else the `/etc/passwd` shell for `uid`
/// if absolute and executable, else `/bin/sh`.
///
/// # Examples
///
/// ```
/// use std::ffi::OsStr;
/// use std::path::PathBuf;
/// use ouro_terminal_picker::resolve_shell_with;
///
/// let shell = resolve_shell_with(
///     Some(OsStr::new("/usr/bin/fish")),
///     None,
///     1000,
///     |p| p == std::path::Path::new("/usr/bin/fish"),
/// );
/// assert_eq!(shell, PathBuf::from("/usr/bin/fish"));
/// ```
pub fn resolve_shell_with<F>(
    env_shell: Option<&OsStr>,
    passwd_content: Option<&str>,
    uid: u32,
    is_exec: F,
) -> PathBuf
where
    F: Fn(&Path) -> bool,
{
    if let Some(raw) = env_shell {
        let candidate = Path::new(raw);
        if candidate.is_absolute() && is_exec(candidate) {
            return candidate.to_path_buf();
        }
    }
    if let Some(passwd) = passwd_content
        && let Some(candidate) = parse_passwd_shell(passwd, uid)
        && candidate.is_absolute()
        && is_exec(&candidate)
    {
        return candidate;
    }
    PathBuf::from(FALLBACK_SHELL)
}

/// Resolve the current user's shell using the live environment and `/etc/passwd`.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::resolve_shell;
///
/// let shell = resolve_shell();
/// assert!(shell.is_absolute());
/// ```
pub fn resolve_shell() -> PathBuf {
    let env_shell = env::var_os("SHELL");
    let passwd = fs::read_to_string("/etc/passwd").ok();
    let uid = fs::metadata("/proc/self")
        .map(|m| m.uid())
        .unwrap_or(u32::MAX);
    resolve_shell_with(
        env_shell.as_deref(),
        passwd.as_deref(),
        uid,
        is_executable_file,
    )
}

/// Replace the current process with `shell` (falling back to `/bin/sh` on exec failure).
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use ouro_terminal_picker::exec_shell;
///
/// let err = exec_shell(Path::new("/bin/sh"));
/// eprintln!("exec failed: {err}");
/// ```
pub fn exec_shell(shell: &Path) -> LaunchError {
    let err = Command::new(shell).exec();
    if shell != Path::new(FALLBACK_SHELL) {
        let _ = Command::new(FALLBACK_SHELL).exec();
    }
    LaunchError::ExecShell {
        shell: shell.to_path_buf(),
        source: err,
    }
}

/// Run the selected [`Choice`]: launch Zellij as a child process and fall back to
/// `shell` on non-zero exit, or exec `shell` directly for [`Choice::Shell`].
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use ouro_terminal_picker::{Choice, run_choice};
///
/// let code = run_choice(&Choice::Create(None), Path::new("zellij"), Path::new("/bin/sh"))?;
/// assert_eq!(code, 0);
/// # Ok::<(), ouro_terminal_picker::LaunchError>(())
/// ```
pub fn run_choice(choice: &Choice, zellij_bin: &Path, shell: &Path) -> Result<i32, LaunchError> {
    tracing::info!(
        kind = choice.kind(),
        session_name_len = choice.session_name_len(),
        "choice launched"
    );

    let Some(mut cmd) = zellij_command_with(zellij_bin, choice) else {
        return Err(exec_shell(shell));
    };

    let status = cmd
        .status()
        .map_err(|source| LaunchError::SpawnZellij { source })?;

    if status.success() {
        return Ok(0);
    }

    let code = status
        .code()
        .or_else(|| status.signal().map(|sig| 128 + sig))
        .unwrap_or(1);
    tracing::warn!(status = code, "zellij fallback");
    eprintln!("Zellij exited with status {code}; opening your shell.");
    Err(exec_shell(shell))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use tempfile::TempDir;

    use super::*;

    fn args_of(cmd: &Command) -> Vec<String> {
        cmd.get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect()
    }

    #[test]
    fn zellij_command_for_attach_matches_plan_argv() {
        let name = SessionName::new("work").unwrap();
        let cmd = zellij_command(&Choice::Attach(name)).expect("attach returns command");
        assert_eq!(cmd.get_program(), OsStr::new("zellij"));
        assert_eq!(
            args_of(&cmd),
            vec!["attach", "work", "options", "--on-force-close", "detach"]
        );
    }

    #[test]
    fn zellij_command_for_create_named_matches_plan_argv() {
        let name = SessionName::new("dev-2").unwrap();
        let cmd = zellij_command(&Choice::Create(Some(name))).expect("create named returns cmd");
        assert_eq!(cmd.get_program(), OsStr::new("zellij"));
        assert_eq!(
            args_of(&cmd),
            vec![
                "attach",
                "--create",
                "dev-2",
                "options",
                "--on-force-close",
                "detach"
            ]
        );
    }

    #[test]
    fn zellij_command_for_create_unnamed_matches_plan_argv() {
        let cmd = zellij_command(&Choice::Create(None)).expect("create unnamed returns cmd");
        assert_eq!(cmd.get_program(), OsStr::new("zellij"));
        assert_eq!(args_of(&cmd), vec!["options", "--on-force-close", "detach"]);
    }

    #[test]
    fn zellij_command_for_shell_is_none() {
        assert!(zellij_command(&Choice::Shell).is_none());
    }

    #[test]
    fn bypass_triggers_when_nested_missing_zellij_or_not_tty() {
        let bin = Path::new("/usr/bin/zellij");
        assert!(should_bypass_picker(Some(OsStr::new("0")), Some(bin), true));
        assert!(should_bypass_picker(None, None, true));
        assert!(should_bypass_picker(None, Some(bin), false));
        assert!(!should_bypass_picker(None, Some(bin), true));
        assert!(!should_bypass_picker(Some(OsStr::new("")), Some(bin), true));
    }

    #[test]
    fn resolve_shell_prefers_valid_env_then_passwd_then_bin_sh() {
        let entries = "root:x:0:0:root:/root:/bin/bash\nuser:x:1000:1000::/srv/u:/usr/bin/zsh\n";

        let from_env = resolve_shell_with(
            Some(OsStr::new("/usr/bin/fish")),
            Some(entries),
            1000,
            |p| p == Path::new("/usr/bin/fish") || p == Path::new("/usr/bin/zsh"),
        );
        assert_eq!(from_env, PathBuf::from("/usr/bin/fish"));

        let from_passwd_relative_env =
            resolve_shell_with(Some(OsStr::new("fish")), Some(entries), 1000, |p| {
                p == Path::new("/usr/bin/zsh")
            });
        assert_eq!(from_passwd_relative_env, PathBuf::from("/usr/bin/zsh"));

        let from_passwd_non_exec_env = resolve_shell_with(
            Some(OsStr::new("/usr/bin/missing")),
            Some(entries),
            1000,
            |p| p == Path::new("/usr/bin/zsh"),
        );
        assert_eq!(from_passwd_non_exec_env, PathBuf::from("/usr/bin/zsh"));

        let fallback = resolve_shell_with(None, Some(entries), 1000, |_| false);
        assert_eq!(fallback, PathBuf::from("/bin/sh"));

        let fallback_unknown_uid = resolve_shell_with(None, Some(entries), 9999, |_| true);
        assert_eq!(fallback_unknown_uid, PathBuf::from("/bin/sh"));
    }

    #[test]
    fn find_in_path_finds_executable_and_ignores_relative_or_non_executable() {
        let dir = TempDir::new().unwrap();
        let bin_dir = dir.path().join("bin");
        fs::create_dir(&bin_dir).unwrap();

        let non_exec = bin_dir.join("zellij");
        fs::write(&non_exec, "#!/bin/sh\n").unwrap();
        let mut perms = fs::metadata(&non_exec).unwrap().permissions();
        perms.set_mode(0o644);
        fs::set_permissions(&non_exec, perms).unwrap();

        let path_val = format!("relative_dir:{}", bin_dir.display());
        assert!(find_in_path("zellij", Some(OsStr::new(&path_val))).is_none());

        let mut perms = fs::metadata(&non_exec).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&non_exec, perms).unwrap();

        assert_eq!(
            find_in_path("zellij", Some(OsStr::new(&path_val))),
            Some(non_exec)
        );
    }
}
