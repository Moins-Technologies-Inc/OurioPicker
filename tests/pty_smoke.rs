//! Pseudo-terminal smoke tests driving `ouro-terminal-picker` in a real Linux PTY.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

fn bin_path() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_ouro-terminal-picker"))
}

fn write_exec(dir: &Path, name: &str, body: &str) -> PathBuf {
    let path = dir.join(name);
    let mut child = Command::new("/bin/sh")
        .args(["-c", "cat > \"$1\" && chmod 0755 \"$1\"", "sh"])
        .arg(&path)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(body.as_bytes())
        .unwrap();
    assert!(child.wait().unwrap().success());
    path
}

const PTY_DRIVER: &str = r#"
import fcntl, os, pty, select, struct, subprocess, sys, termios, time

bin_path = sys.argv[1]
fake_bin = sys.argv[2]
fake_shell = sys.argv[3]
log_level = sys.argv[4]
chunks = [bytes.fromhex(h) for h in sys.argv[5:]]

master, slave = pty.openpty()
winsize = struct.pack("HHHH", 24, 80, 0, 0)
fcntl.ioctl(slave, termios.TIOCSWINSZ, winsize)

env = {
    "PATH": f"{fake_bin}:/usr/bin:/bin",
    "SHELL": fake_shell,
    "TERM": "xterm-256color",
    "NO_COLOR": "1",
}
if log_level:
    env["OURO_TERMINAL_PICKER_LOG"] = log_level

proc = subprocess.Popen(
    [bin_path],
    stdin=slave,
    stdout=slave,
    stderr=slave,
    env=env,
    close_fds=True,
    start_new_session=True,
)
os.close(slave)

buf = bytearray()
deadline = time.monotonic() + 5.0
while time.monotonic() < deadline and b"Zellij sessions" not in buf:
    r, _, _ = select.select([master], [], [], 0.05)
    if r:
        try:
            data = os.read(master, 4096)
            if not data:
                break
            buf.extend(data)
        except OSError:
            break

for chunk in chunks:
    os.write(master, chunk)
    time.sleep(0.06)
    r, _, _ = select.select([master], [], [], 0.05)
    if r:
        try:
            buf.extend(os.read(master, 4096))
        except OSError:
            pass

drain_deadline = time.monotonic() + 5.0
while time.monotonic() < drain_deadline:
    r, _, _ = select.select([master], [], [], 0.05)
    if r:
        try:
            data = os.read(master, 4096)
            if not data:
                break
            buf.extend(data)
        except OSError:
            break
    elif proc.poll() is not None:
        break

os.close(master)
rc = proc.wait(timeout=5.0)
sys.stdout.buffer.write(buf)
sys.stdout.buffer.flush()
sys.exit(rc)
"#;

fn run_in_pty(fake_bin_dir: &Path, fake_shell: &Path, log_level: &str, keys: &[&[u8]]) -> Output {
    let mut cmd = Command::new("/usr/bin/python3");
    cmd.args(["-I", "-c", PTY_DRIVER])
        .arg(bin_path())
        .arg(fake_bin_dir)
        .arg(fake_shell)
        .arg(log_level);
    for chunk in keys {
        let hex: String = chunk.iter().map(|b| format!("{b:02x}")).collect();
        cmd.arg(hex);
    }
    cmd.output().expect("run python3 PTY driver")
}

#[test]
fn pty_new_named_session_records_exact_zellij_argv_and_restores_screen() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("zellij-argv.txt");
    let fake_shell = write_exec(dir.path(), "fake-shell", "#!/bin/sh\nexit 77\n");
    let _fake_zellij = write_exec(
        dir.path(),
        "zellij",
        &format!(
            "#!/bin/sh\n\
if [ \"$1\" = \"list-sessions\" ] && [ \"$2\" = \"--no-formatting\" ]; then\n\
  printf 'work [Created 10days 7h 11m 4s ago] (EXITED - attach to resurrect)\\nd580883b86d4 [Created 8s ago]\\n'\n\
  exit 0\n\
fi\n\
printf '%s\\n' \"$@\" > '{}'\n\
exit 0\n",
            argv_file.display()
        ),
    );

    let out = run_in_pty(dir.path(), &fake_shell, "", &[b"n", b"smoke-1", b"\r"]);
    assert!(
        out.status.success(),
        "status={:?} stdout={} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let pty_stream = String::from_utf8_lossy(&out.stdout);
    assert!(
        pty_stream.contains("\x1b[?1049h"),
        "picker must enter alternate screen"
    );
    assert!(
        pty_stream.contains("\x1b[?1049l"),
        "picker must leave alternate screen before launch"
    );

    let recorded: Vec<String> = fs::read_to_string(&argv_file)
        .expect("fake zellij must record argv")
        .lines()
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(
        recorded,
        vec![
            "attach",
            "--create",
            "smoke-1",
            "options",
            "--on-force-close",
            "detach"
        ]
    );
}

#[test]
fn pty_esc_launches_plain_shell_without_running_zellij_attach() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("zellij-argv.txt");
    let shell_marker = dir.path().join("shell-marker.txt");
    let fake_shell = write_exec(
        dir.path(),
        "fake-shell",
        &format!(
            "#!/bin/sh\necho plain-shell-ok > '{}'\nexit 0\n",
            shell_marker.display()
        ),
    );
    let _fake_zellij = write_exec(
        dir.path(),
        "zellij",
        &format!(
            "#!/bin/sh\n\
if [ \"$1\" = \"list-sessions\" ]; then\n\
  echo 'No active zellij sessions found.' >&2\n\
  exit 1\n\
fi\n\
printf '%s\\n' \"$@\" > '{}'\n\
exit 0\n",
            argv_file.display()
        ),
    );

    let out = run_in_pty(dir.path(), &fake_shell, "", &[b"\x1b"]);
    assert!(
        out.status.success(),
        "status={:?} stdout={} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read_to_string(&shell_marker).unwrap().trim(),
        "plain-shell-ok"
    );
    assert!(!argv_file.exists(), "zellij must not be launched on Esc");
}

#[test]
fn pty_nonzero_zellij_exit_prints_notice_and_falls_back_to_shell() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("zellij-argv.txt");
    let shell_marker = dir.path().join("shell-marker.txt");
    let fake_shell = write_exec(
        dir.path(),
        "fake-shell",
        &format!(
            "#!/bin/sh\necho fallback-ok > '{}'\nexit 0\n",
            shell_marker.display()
        ),
    );
    let _fake_zellij = write_exec(
        dir.path(),
        "zellij",
        &format!(
            "#!/bin/sh\n\
if [ \"$1\" = \"list-sessions\" ]; then\n\
  printf 'work [Created 10days 7h 11m 4s ago] (EXITED - attach to resurrect)\\nd580883b86d4 [Created 8s ago]\\n'\n\
  exit 0\n\
fi\n\
printf '%s\\n' \"$@\" > '{}'\n\
exit 42\n",
            argv_file.display()
        ),
    );

    let out = run_in_pty(dir.path(), &fake_shell, "", &[b"\r"]);
    assert!(
        out.status.success(),
        "status={:?} stdout={} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let recorded: Vec<String> = fs::read_to_string(&argv_file)
        .unwrap()
        .lines()
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(
        recorded,
        vec![
            "attach",
            "d580883b86d4",
            "options",
            "--on-force-close",
            "detach"
        ]
    );
    let pty_stream = String::from_utf8_lossy(&out.stdout);
    assert!(
        pty_stream.contains("Zellij exited with status 42; opening your shell."),
        "missing fallback notice in output: {pty_stream}"
    );
    assert_eq!(
        fs::read_to_string(&shell_marker).unwrap().trim(),
        "fallback-ok"
    );
}

#[test]
fn pty_create_unnamed_session_and_resurrect_exited_session() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("zellij-argv.txt");
    let fake_shell = write_exec(dir.path(), "fake-shell", "#!/bin/sh\nexit 77\n");
    let _fake_zellij = write_exec(
        dir.path(),
        "zellij",
        &format!(
            "#!/bin/sh\n\
if [ \"$1\" = \"list-sessions\" ]; then\n\
  printf 'work [Created 10days 7h 11m 4s ago] (EXITED - attach to resurrect)\\nd580883b86d4 [Created 8s ago]\\n'\n\
  exit 0\n\
fi\n\
printf '%s\\n' \"$@\" > '{}'\n\
exit 0\n",
            argv_file.display()
        ),
    );

    let out = run_in_pty(dir.path(), &fake_shell, "", &[b"n", b"\r"]);
    assert!(out.status.success());
    let recorded: Vec<String> = fs::read_to_string(&argv_file)
        .unwrap()
        .lines()
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(recorded, vec!["options", "--on-force-close", "detach"]);

    let _ = fs::remove_file(&argv_file);
    let out2 = run_in_pty(dir.path(), &fake_shell, "", &[b"j", b"\r"]);
    assert!(out2.status.success());
    let recorded2: Vec<String> = fs::read_to_string(&argv_file)
        .unwrap()
        .lines()
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(
        recorded2,
        vec!["attach", "work", "options", "--on-force-close", "detach"]
    );
}

#[test]
fn pty_logs_only_after_terminal_restore_and_never_logs_session_names() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("zellij-argv.txt");
    let fake_shell = write_exec(dir.path(), "fake-shell", "#!/bin/sh\nexit 0\n");
    let _fake_zellij = write_exec(
        dir.path(),
        "zellij",
        &format!(
            "#!/bin/sh\n\
if [ \"$1\" = \"list-sessions\" ]; then\n\
  printf 'private-alpha [Created 8s ago]\\n'\n\
  exit 0\n\
fi\n\
printf '%s\\n' \"$@\" > '{}'\n\
exit 0\n",
            argv_file.display()
        ),
    );

    let out = run_in_pty(dir.path(), &fake_shell, "debug", &[b"\r"]);
    assert!(out.status.success());
    let pty_stream = String::from_utf8_lossy(&out.stdout);
    let leave_pos = pty_stream
        .rfind("\x1b[?1049l")
        .expect("must leave alternate screen");
    let after_restore = &pty_stream[leave_pos..];
    assert!(
        after_restore.contains("sessions listed"),
        "debug log must be emitted after terminal restore: {after_restore:?}"
    );
    assert!(
        after_restore.contains("choice launched"),
        "info log must be emitted after terminal restore: {after_restore:?}"
    );
    assert!(
        !after_restore.contains("private-alpha"),
        "session name must never appear in logs: {after_restore:?}"
    );
}
