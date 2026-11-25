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
import fcntl, json, os, pty, select, struct, subprocess, sys, termios, time

argv_json = sys.argv[1]
env_json = sys.argv[2]
wait_picker = sys.argv[3] == "1"
chunks = [bytes.fromhex(h) for h in sys.argv[4:]]

child_argv = json.loads(argv_json)
env = json.loads(env_json)

master, slave = pty.openpty()
winsize = struct.pack("HHHH", 24, 80, 0, 0)
fcntl.ioctl(slave, termios.TIOCSWINSZ, winsize)

proc = subprocess.Popen(
    child_argv,
    stdin=slave,
    stdout=slave,
    stderr=slave,
    env=env,
    close_fds=True,
    start_new_session=True,
)
os.close(slave)

buf = bytearray()
if wait_picker:
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
        elif proc.poll() is not None:
            break

    for chunk in chunks:
        if proc.poll() is not None:
            break
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

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn run_in_pty_custom(
    child_argv: &[&str],
    env_pairs: &[(&str, String)],
    wait_for_picker: bool,
    keys: &[&[u8]],
) -> Output {
    let argv_json = format!(
        "[{}]",
        child_argv
            .iter()
            .map(|a| json_escape(a))
            .collect::<Vec<_>>()
            .join(",")
    );
    let env_json = format!(
        "{{{}}}",
        env_pairs
            .iter()
            .map(|(k, v)| format!("{}:{}", json_escape(k), json_escape(v)))
            .collect::<Vec<_>>()
            .join(",")
    );

    let mut cmd = Command::new("/usr/bin/python3");
    cmd.args(["-I", "-c", PTY_DRIVER])
        .arg(argv_json)
        .arg(env_json)
        .arg(if wait_for_picker { "1" } else { "0" });
    for chunk in keys {
        let hex: String = chunk.iter().map(|b| format!("{b:02x}")).collect();
        cmd.arg(hex);
    }
    cmd.output().expect("run python3 PTY driver")
}

fn run_in_pty(fake_bin_dir: &Path, fake_shell: &Path, log_level: &str, keys: &[&[u8]]) -> Output {
    let mut env = vec![
        ("PATH", format!("{}:/usr/bin:/bin", fake_bin_dir.display())),
        ("SHELL", fake_shell.display().to_string()),
        ("TERM", "xterm-256color".to_owned()),
        ("NO_COLOR", "1".to_owned()),
    ];
    if !log_level.is_empty() {
        env.push(("OURO_TERMINAL_PICKER_LOG", log_level.to_owned()));
    }
    let bin_str = bin_path().to_str().unwrap();
    run_in_pty_custom(&[bin_str], &env, true, keys)
}

fn assert_cooked_mode(stty_output: &str, context: &str) {
    let tokens: Vec<&str> = stty_output.split_whitespace().collect();
    assert!(
        tokens.contains(&"icanon") && !tokens.contains(&"-icanon"),
        "{context}: expected ICANON enabled in stty output:\n{stty_output}"
    );
    assert!(
        tokens.contains(&"echo") && !tokens.contains(&"-echo"),
        "{context}: expected ECHO enabled in stty output:\n{stty_output}"
    );
}

#[test]
fn pty_new_named_session_records_exact_zellij_argv_and_restores_screen() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("zellij-argv.txt");
    let stty_file = dir.path().join("zellij-stty.txt");
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
/usr/bin/stty -a > '{}'\n\
printf '%s\\n' \"$@\" > '{}'\n\
exit 0\n",
            stty_file.display(),
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

    assert_cooked_mode(&fs::read_to_string(&stty_file).unwrap(), "fake zellij");

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
fn pty_esc_launches_plain_shell_with_cooked_mode_and_log() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("zellij-argv.txt");
    let shell_marker = dir.path().join("shell-marker.txt");
    let stty_file = dir.path().join("shell-stty.txt");
    let fake_shell = write_exec(
        dir.path(),
        "fake-shell",
        &format!(
            "#!/bin/sh\n/usr/bin/stty -a > '{}'\nprintf 'plain-shell-ok\\n' > '{}'\nexit 0\n",
            stty_file.display(),
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

    let out = run_in_pty(dir.path(), &fake_shell, "debug", &[b"\x1b"]);
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
    assert_cooked_mode(&fs::read_to_string(&stty_file).unwrap(), "fake shell (Esc)");

    let pty_stream = String::from_utf8_lossy(&out.stdout);
    let leave_pos = pty_stream
        .rfind("\x1b[?1049l")
        .expect("must leave alternate screen");
    let after_restore = &pty_stream[leave_pos..];
    assert!(
        after_restore.contains("choice launched") && after_restore.contains("shell"),
        "F2: Esc path must emit choice launched (shell) before exec: {after_restore:?}"
    );
}

#[test]
fn pty_ctrl_c_launches_plain_shell_with_cooked_mode_and_log() {
    let dir = TempDir::new().unwrap();
    let shell_marker = dir.path().join("shell-marker.txt");
    let stty_file = dir.path().join("shell-stty.txt");
    let fake_shell = write_exec(
        dir.path(),
        "fake-shell",
        &format!(
            "#!/bin/sh\n/usr/bin/stty -a > '{}'\nprintf 'ctrl-c-shell-ok\\n' > '{}'\nexit 0\n",
            stty_file.display(),
            shell_marker.display()
        ),
    );
    let _fake_zellij = write_exec(
        dir.path(),
        "zellij",
        "#!/bin/sh\nif [ \"$1\" = \"list-sessions\" ]; then\n  echo 'No active zellij sessions found.' >&2\n  exit 1\nfi\nexit 99\n",
    );

    let out = run_in_pty(dir.path(), &fake_shell, "debug", &[b"\x03"]);
    assert!(
        out.status.success(),
        "status={:?} stdout={} stderr={}",
        out.status,
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(
        fs::read_to_string(&shell_marker).unwrap().trim(),
        "ctrl-c-shell-ok"
    );
    assert_cooked_mode(
        &fs::read_to_string(&stty_file).unwrap(),
        "fake shell (Ctrl-C)",
    );

    let pty_stream = String::from_utf8_lossy(&out.stdout);
    let leave_pos = pty_stream
        .rfind("\x1b[?1049l")
        .expect("must leave alternate screen");
    let after_restore = &pty_stream[leave_pos..];
    assert!(
        after_restore.contains("choice launched") && after_restore.contains("shell"),
        "F2: Ctrl-C path must emit choice launched (shell) before exec: {after_restore:?}"
    );
}

#[test]
fn pty_nonzero_zellij_exit_prints_notice_logs_fallback_and_execs_shell() {
    let dir = TempDir::new().unwrap();
    let argv_file = dir.path().join("zellij-argv.txt");
    let shell_marker = dir.path().join("shell-marker.txt");
    let stty_file = dir.path().join("shell-stty.txt");
    let fake_shell = write_exec(
        dir.path(),
        "fake-shell",
        &format!(
            "#!/bin/sh\n/usr/bin/stty -a > '{}'\nprintf 'fallback-ok\\n' > '{}'\nexit 0\n",
            stty_file.display(),
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

    let out = run_in_pty(dir.path(), &fake_shell, "debug", &[b"\r"]);
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
    let leave_pos = pty_stream
        .rfind("\x1b[?1049l")
        .expect("must leave alternate screen");
    let after_restore = &pty_stream[leave_pos..];
    assert!(
        after_restore.contains("zellij fallback") && after_restore.contains("42"),
        "F2: Zellij exit 42 must emit zellij fallback log before exec: {after_restore:?}"
    );
    assert_eq!(
        fs::read_to_string(&shell_marker).unwrap().trim(),
        "fallback-ok"
    );
    assert_cooked_mode(
        &fs::read_to_string(&stty_file).unwrap(),
        "fake shell (fallback)",
    );
}

#[test]
fn pty_zellij_spawn_failure_prints_notice_logs_warn_and_execs_shell() {
    let dir = TempDir::new().unwrap();
    let shell_marker = dir.path().join("shell-marker.txt");
    let fake_shell = write_exec(
        dir.path(),
        "fake-shell",
        &format!(
            "#!/bin/sh\nprintf 'spawn-fallback-ok\\n' > '{}'\nexit 0\n",
            shell_marker.display()
        ),
    );
    let zellij_path = dir.path().join("zellij");
    // Succeeds on list-sessions, then revokes its own execute permission so spawn fails with EACCES.
    let _fake_zellij = write_exec(
        dir.path(),
        "zellij",
        &format!(
            "#!/bin/sh\n\
if [ \"$1\" = \"list-sessions\" ]; then\n\
  printf 'work [Created 8s ago]\\n'\n\
  /bin/chmod 000 '{}'\n\
  exit 0\n\
fi\n\
exit 99\n",
            zellij_path.display()
        ),
    );

    let out = run_in_pty(dir.path(), &fake_shell, "debug", &[b"\r"]);
    // Restore perms so TempDir cleanup always succeeds.
    let _ = Command::new("/bin/chmod")
        .args(["0755"])
        .arg(&zellij_path)
        .status();

    assert!(
        out.status.success(),
        "F1: spawn failure must fall back to shell and exit 0, got {:?}: {}",
        out.status,
        String::from_utf8_lossy(&out.stdout)
    );
    let pty_stream = String::from_utf8_lossy(&out.stdout);
    assert!(
        pty_stream.contains("Could not start Zellij (")
            && pty_stream.contains("); opening your shell."),
        "F1: expected 'Could not start Zellij (<reason>); opening your shell.' in: {pty_stream}"
    );
    assert!(
        pty_stream.contains("zellij fallback"),
        "F1: expected warn log on spawn failure: {pty_stream}"
    );
    assert_eq!(
        fs::read_to_string(&shell_marker).unwrap().trim(),
        "spawn-fallback-ok"
    );
}

#[test]
fn pty_bypasses_picker_when_nested_in_zellij_with_real_tty() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("nested-shell-ran.txt");
    let fake_shell = write_exec(
        dir.path(),
        "fake-shell",
        &format!(
            "#!/bin/sh\nprintf 'nested-tty-ok\\n' > '{}'\nexit 0\n",
            marker.display()
        ),
    );
    let _fake_zellij = write_exec(
        dir.path(),
        "zellij",
        "#!/bin/sh\necho SHOULD_NOT_RUN >&2\nexit 99\n",
    );

    let bin_str = bin_path().to_str().unwrap();
    let env = vec![
        ("PATH", format!("{}:/usr/bin:/bin", dir.path().display())),
        ("SHELL", fake_shell.display().to_string()),
        ("ZELLIJ", "0".to_owned()),
        ("TERM", "xterm-256color".to_owned()),
        ("NO_COLOR", "1".to_owned()),
    ];
    let out = run_in_pty_custom(&[bin_str], &env, false, &[]);
    assert!(
        out.status.success(),
        "status={:?} out={}",
        out.status,
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(fs::read_to_string(&marker).unwrap().trim(), "nested-tty-ok");
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("\x1b[?1049h"),
        "nested ZELLIJ must not enter alternate screen"
    );
}

#[test]
fn pty_bypasses_picker_when_zellij_missing_from_path_with_real_tty() {
    let dir = TempDir::new().unwrap();
    let empty_bin = dir.path().join("empty-bin");
    fs::create_dir(&empty_bin).unwrap();
    let marker = dir.path().join("missing-zellij-shell-ran.txt");
    let fake_shell = write_exec(
        dir.path(),
        "fake-shell",
        &format!(
            "#!/bin/sh\nprintf 'missing-zellij-tty-ok\\n' > '{}'\nexit 0\n",
            marker.display()
        ),
    );

    let bin_str = bin_path().to_str().unwrap();
    let env = vec![
        ("PATH", empty_bin.display().to_string()),
        ("SHELL", fake_shell.display().to_string()),
        ("TERM", "xterm-256color".to_owned()),
        ("NO_COLOR", "1".to_owned()),
    ];
    let out = run_in_pty_custom(&[bin_str], &env, false, &[]);
    assert!(
        out.status.success(),
        "status={:?} out={}",
        out.status,
        String::from_utf8_lossy(&out.stdout)
    );
    assert_eq!(
        fs::read_to_string(&marker).unwrap().trim(),
        "missing-zellij-tty-ok"
    );
    assert!(
        !String::from_utf8_lossy(&out.stdout).contains("\x1b[?1049h"),
        "missing zellij must not enter alternate screen"
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

#[test]
fn pty_panic_unwind_restores_cooked_mode_and_main_screen() {
    if std::env::var_os("OURO_TEST_PANIC_CHILD").is_some() {
        let during_path = std::env::var_os("OURO_TEST_STTY_DURING").unwrap();
        let after_path = std::env::var_os("OURO_TEST_STTY_AFTER").unwrap();

        ouro_terminal_picker::install_panic_hook();
        let res = std::panic::catch_unwind(|| {
            let _guard = ouro_terminal_picker::term::TerminalGuard::enter().unwrap();
            let during = Command::new("/usr/bin/stty")
                .arg("-a")
                .stdin(Stdio::inherit())
                .output()
                .expect("stty during raw mode");
            assert!(during.status.success(), "stty during raw mode failed");
            fs::write(&during_path, &during.stdout).unwrap();
            panic!("simulated picker panic");
        });
        assert!(res.is_err(), "simulated panic must unwind");

        let after = Command::new("/usr/bin/stty")
            .arg("-a")
            .stdin(Stdio::inherit())
            .output()
            .expect("stty after unwind");
        assert!(after.status.success(), "stty after unwind failed");
        fs::write(&after_path, &after.stdout).unwrap();
        return;
    }

    let dir = TempDir::new().unwrap();
    let during_file = dir.path().join("stty-during.txt");
    let after_file = dir.path().join("stty-after.txt");
    let test_exe = std::env::current_exe().unwrap();
    let exe_str = test_exe.to_str().unwrap();

    let env = vec![
        ("PATH", "/usr/bin:/bin".to_owned()),
        ("TERM", "xterm-256color".to_owned()),
        ("NO_COLOR", "1".to_owned()),
        ("OURO_TEST_PANIC_CHILD", "1".to_owned()),
        ("OURO_TEST_STTY_DURING", during_file.display().to_string()),
        ("OURO_TEST_STTY_AFTER", after_file.display().to_string()),
    ];
    let out = run_in_pty_custom(
        &[
            exe_str,
            "--exact",
            "pty_panic_unwind_restores_cooked_mode_and_main_screen",
            "--nocapture",
        ],
        &env,
        false,
        &[],
    );
    assert!(
        out.status.success(),
        "panic-unwind child failed: status={:?} out={}",
        out.status,
        String::from_utf8_lossy(&out.stdout)
    );

    let during_stty = fs::read_to_string(&during_file).unwrap();
    let during_tokens: Vec<&str> = during_stty.split_whitespace().collect();
    assert!(
        during_tokens.contains(&"-icanon") && during_tokens.contains(&"-echo"),
        "raw mode must have been active before panic: {during_stty}"
    );

    let after_stty = fs::read_to_string(&after_file).unwrap();
    assert_cooked_mode(&after_stty, "after panic unwind");

    let pty_stream = String::from_utf8_lossy(&out.stdout);
    assert!(pty_stream.contains("\x1b[?1049h"), "must enter alt screen");
    assert!(
        pty_stream.contains("\x1b[?1049l"),
        "must leave alt screen on panic unwind"
    );
    assert!(
        pty_stream
            .contains("ouro-terminal-picker encountered an internal error; opening your shell."),
        "panic hook notice must appear: {pty_stream}"
    );
}
