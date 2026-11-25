//! Listing and parsing Zellij sessions.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use thiserror::Error;

use crate::name::SessionName;

const NO_SESSIONS_MARKER: &str = "No active zellij sessions found.";

/// Lifecycle state of a discovered Zellij session.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::SessionState;
///
/// assert_ne!(SessionState::Running, SessionState::Exited);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionState {
    /// Active session that can be attached immediately.
    Running,
    /// Active session marked `(current)` by Zellij.
    Current,
    /// Exited session that can be resurrected on attach.
    Exited,
}

/// A parsed Zellij session entry.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::{Session, SessionName, SessionState};
///
/// let session = Session {
///     name: SessionName::new("work")?,
///     state: SessionState::Running,
/// };
/// assert_eq!(session.name.as_str(), "work");
/// # Ok::<(), ouro_terminal_picker::NameError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// Validated session name.
    pub name: SessionName,
    /// Current lifecycle state.
    pub state: SessionState,
}

/// Failure when invoking `zellij list-sessions --no-formatting`.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::SessionListError;
///
/// let err = SessionListError::CommandFailed {
///     status: Some(2),
///     detail: "socket error".to_owned(),
/// };
/// assert!(err.to_string().contains("socket error"));
/// ```
#[derive(Debug, Error)]
pub enum SessionListError {
    /// Failed to spawn the `zellij` executable.
    #[error("failed to start zellij: {source}")]
    SpawnFailed {
        /// Underlying OS error.
        #[source]
        source: std::io::Error,
    },
    /// `zellij list-sessions` exited with an unexpected non-zero status.
    #[error("zellij list-sessions failed (status {status:?}): {detail}")]
    CommandFailed {
        /// Exit code if terminated normally.
        status: Option<i32>,
        /// Sanitised stderr/stdout detail.
        detail: String,
    },
    /// Output from `zellij` was not valid UTF-8.
    #[error("zellij list-sessions produced non-UTF-8 output: {source}")]
    InvalidUtf8 {
        /// UTF-8 decoding error.
        #[source]
        source: std::string::FromUtf8Error,
    },
}

/// Remove ANSI CSI and OSC escape sequences from `input`.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::strip_ansi;
///
/// assert_eq!(strip_ansi("\x1b[32;1mwork\x1b[0m [Created 8s ago]"), "work [Created 8s ago]");
/// ```
pub fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch != '\x1b' {
            out.push(ch);
            continue;
        }
        match chars.peek().copied() {
            Some('[') => {
                // CSI sequence: ESC [ params intermediates final (0x40..=0x7E)
                let _ = chars.next();
                for next in chars.by_ref() {
                    if ('@'..='~').contains(&next) {
                        break;
                    }
                }
            }
            Some(']') => {
                // OSC sequence: ESC ] ... (BEL or ESC \)
                let _ = chars.next();
                while let Some(next) = chars.next() {
                    if next == '\x07' {
                        break;
                    }
                    if next == '\x1b' && chars.peek() == Some(&'\\') {
                        let _ = chars.next();
                        break;
                    }
                }
            }
            Some(_) => {
                // Two-byte escape sequence
                let _ = chars.next();
            }
            None => {}
        }
    }
    out
}

/// Parse the stdout of `zellij list-sessions --no-formatting`.
///
/// Running and current sessions are returned before exited sessions, preserving
/// Zellij's relative order within each group. Lines with invalid session names
/// are discarded safely.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::{SessionState, parse_sessions};
///
/// let raw = "\
/// work [Created 10days 7h 11m 4s ago] (EXITED - attach to resurrect)
/// d580883b86d4 [Created 8s ago]
/// ";
/// let sessions = parse_sessions(raw);
/// assert_eq!(sessions.len(), 2);
/// assert_eq!(sessions[0].name.as_str(), "d580883b86d4");
/// assert_eq!(sessions[0].state, SessionState::Running);
/// assert_eq!(sessions[1].name.as_str(), "work");
/// assert_eq!(sessions[1].state, SessionState::Exited);
/// ```
pub fn parse_sessions(output: &str) -> Vec<Session> {
    let mut running = Vec::new();
    let mut exited = Vec::new();
    let mut seen = HashSet::new();

    for raw_line in output.lines() {
        let clean = strip_ansi(raw_line);
        let line = clean.trim();
        if line.is_empty() || line.eq_ignore_ascii_case(NO_SESSIONS_MARKER) {
            continue;
        }

        let mut parts = line.splitn(2, char::is_whitespace);
        let Some(token) = parts.next() else {
            continue;
        };
        let Ok(name) = SessionName::new(token) else {
            continue;
        };
        if !seen.insert(name.clone()) {
            continue;
        }

        let rest = parts.next().unwrap_or("");
        let state = if rest.contains("(EXITED") {
            SessionState::Exited
        } else if rest.contains("(current)") {
            SessionState::Current
        } else {
            SessionState::Running
        };

        let session = Session { name, state };
        if state == SessionState::Exited {
            exited.push(session);
        } else {
            running.push(session);
        }
    }

    running.extend(exited);
    running
}

/// List Zellij sessions using the `zellij` binary on `PATH`.
///
/// # Examples
///
/// ```no_run
/// use ouro_terminal_picker::list_sessions;
///
/// let sessions = list_sessions()?;
/// println!("found {} sessions", sessions.len());
/// # Ok::<(), ouro_terminal_picker::SessionListError>(())
/// ```
pub fn list_sessions() -> Result<Vec<Session>, SessionListError> {
    list_sessions_with(Path::new("zellij"))
}

/// List Zellij sessions using an explicit `zellij_bin` path.
///
/// # Examples
///
/// ```no_run
/// use std::path::Path;
/// use ouro_terminal_picker::list_sessions_with;
///
/// let sessions = list_sessions_with(Path::new("/usr/bin/zellij"))?;
/// assert!(sessions.len() < 10_000);
/// # Ok::<(), ouro_terminal_picker::SessionListError>(())
/// ```
pub fn list_sessions_with(zellij_bin: &Path) -> Result<Vec<Session>, SessionListError> {
    let _span = tracing::debug_span!("list_sessions").entered();
    let started = Instant::now();

    let output = Command::new(zellij_bin)
        .args(["list-sessions", "--no-formatting"])
        .output()
        .map_err(|source| SessionListError::SpawnFailed { source })?;

    let stdout = String::from_utf8(output.stdout)
        .map_err(|source| SessionListError::InvalidUtf8 { source })?;
    let stderr = String::from_utf8_lossy(&output.stderr);

    let clean_stdout = strip_ansi(&stdout);
    let clean_stderr = strip_ansi(&stderr);

    if !output.status.success() {
        let no_sessions_in_stderr = clean_stderr.trim().eq_ignore_ascii_case(NO_SESSIONS_MARKER);
        let no_sessions_in_stdout = clean_stdout.trim().eq_ignore_ascii_case(NO_SESSIONS_MARKER);
        if (no_sessions_in_stderr && clean_stdout.trim().is_empty()) || no_sessions_in_stdout {
            let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            tracing::debug!(count = 0usize, duration_ms, "sessions listed");
            return Ok(Vec::new());
        }

        let detail_src = if !clean_stderr.trim().is_empty() {
            clean_stderr.trim()
        } else {
            clean_stdout.trim()
        };
        let detail = if detail_src.is_empty() {
            "unknown error".to_owned()
        } else {
            detail_src
                .lines()
                .next()
                .unwrap_or("unknown error")
                .to_owned()
        };
        return Err(SessionListError::CommandFailed {
            status: output.status.code(),
            detail,
        });
    }

    let sessions = parse_sessions(&clean_stdout);
    let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    tracing::debug!(count = sessions.len(), duration_ms, "sessions listed");
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use tempfile::TempDir;

    use super::*;

    fn write_script(dir: &TempDir, name: &str, body: &str) -> std::path::PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, body).unwrap();
        let mut perms = fs::metadata(&path).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&path, perms).unwrap();
        path
    }

    #[test]
    fn parses_observed_046_single_exited_fixture() {
        let raw = "work [Created 10days 7h 11m 22s ago] (EXITED - attach to resurrect)\n";
        let sessions = parse_sessions(raw);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name.as_str(), "work");
        assert_eq!(sessions[0].state, SessionState::Exited);
    }

    #[test]
    fn parses_observed_046_mixed_fixture_and_sorts_running_before_exited() {
        let raw = "\
work [Created 10days 7h 11m 4s ago] (EXITED - attach to resurrect)
d580883b86d4 [Created 8s ago]
";
        let sessions = parse_sessions(raw);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name.as_str(), "d580883b86d4");
        assert_eq!(sessions[0].state, SessionState::Running);
        assert_eq!(sessions[1].name.as_str(), "work");
        assert_eq!(sessions[1].state, SessionState::Exited);
    }

    #[test]
    fn parses_current_running_and_exited_preserving_group_order() {
        let raw = "\
old-1 [Created 5days ago] (EXITED - attach to resurrect)
alpha [Created 1h ago]
active-now [Created 10m ago] (current)
old-2 [Created 2days ago] (EXITED - attach to resurrect)
beta [Created 5m ago]
";
        let sessions = parse_sessions(raw);
        let summary: Vec<(&str, SessionState)> = sessions
            .iter()
            .map(|s| (s.name.as_str(), s.state))
            .collect();
        assert_eq!(
            summary,
            vec![
                ("alpha", SessionState::Running),
                ("active-now", SessionState::Current),
                ("beta", SessionState::Running),
                ("old-1", SessionState::Exited),
                ("old-2", SessionState::Exited),
            ]
        );
    }

    #[test]
    fn handles_empty_and_no_active_sessions_message() {
        assert!(parse_sessions("").is_empty());
        assert!(parse_sessions("   \n\n").is_empty());
        assert!(parse_sessions("No active zellij sessions found.\n").is_empty());
    }

    #[test]
    fn strips_ansi_csi_and_osc_defensively() {
        let raw = "\x1b[32;1mwork\x1b[0m \x1b[2m[Created 8s ago]\x1b[0m\n\x1b]0;title\x07old [Created 1d ago] (EXITED - attach to resurrect)\n";
        let sessions = parse_sessions(raw);
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name.as_str(), "work");
        assert_eq!(sessions[0].state, SessionState::Running);
        assert_eq!(sessions[1].name.as_str(), "old");
        assert_eq!(sessions[1].state, SessionState::Exited);
    }

    #[test]
    fn skips_invalid_names_and_deduplicates() {
        let raw = "\
--evil [Created 1s ago]
.hidden [Created 2s ago]
bad;name [Created 3s ago]
good [Created 4s ago]
good [Created 5s ago] (EXITED - attach to resurrect)
";
        let sessions = parse_sessions(raw);
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].name.as_str(), "good");
        assert_eq!(sessions[0].state, SessionState::Running);
    }

    #[test]
    fn list_sessions_with_treats_zellij_046_no_sessions_exit_1_as_empty() {
        let dir = TempDir::new().unwrap();
        let fake = write_script(
            &dir,
            "zellij",
            "#!/bin/sh\necho 'No active zellij sessions found.' >&2\nexit 1\n",
        );
        let sessions = list_sessions_with(&fake).expect("no sessions must be Ok(empty)");
        assert!(sessions.is_empty());
    }

    #[test]
    fn list_sessions_with_parses_fake_zellij_output() {
        let dir = TempDir::new().unwrap();
        let fake = write_script(
            &dir,
            "zellij",
            "#!/bin/sh\nif [ \"$1\" = \"list-sessions\" ] && [ \"$2\" = \"--no-formatting\" ]; then\n  printf 'work [Created 10days 7h 11m 4s ago] (EXITED - attach to resurrect)\\nd580883b86d4 [Created 8s ago]\\n'\n  exit 0\nfi\nexit 99\n",
        );
        let sessions = list_sessions_with(&fake).expect("fake zellij must succeed");
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].name.as_str(), "d580883b86d4");
        assert_eq!(sessions[1].name.as_str(), "work");
    }

    #[test]
    fn list_sessions_with_reports_command_failure() {
        let dir = TempDir::new().unwrap();
        let fake = write_script(
            &dir,
            "zellij",
            "#!/bin/sh\necho 'permission denied on socket' >&2\nexit 2\n",
        );
        let err = list_sessions_with(&fake).expect_err("non-zero failure must return Err");
        assert!(matches!(
            err,
            SessionListError::CommandFailed {
                status: Some(2),
                ..
            }
        ));
        assert!(err.to_string().contains("permission denied on socket"));
    }

    #[test]
    fn list_sessions_with_reports_missing_binary() {
        let dir = TempDir::new().unwrap();
        let missing = dir.path().join("does-not-exist");
        let err = list_sessions_with(&missing).expect_err("missing binary must return Err");
        assert!(matches!(err, SessionListError::SpawnFailed { .. }));
    }
}
