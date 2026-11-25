//! Integration tests for the `completions` subcommand.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use tempfile::TempDir;

fn bin_path() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_ouro-terminal-picker"))
}

struct IsolatedEnv {
    dir: TempDir,
    fake_shell: PathBuf,
    shell_marker: PathBuf,
    zellij_marker: PathBuf,
    home: PathBuf,
    xdg_config: PathBuf,
    rio_config: PathBuf,
}

impl IsolatedEnv {
    fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let bin_dir = dir.path().join("bin");
        let home = dir.path().join("home");
        let xdg_config = dir.path().join("xdg-config");
        let rio_config = dir.path().join("rio-config");
        fs::create_dir_all(&bin_dir).unwrap();
        fs::create_dir_all(&home).unwrap();
        fs::create_dir_all(&xdg_config).unwrap();
        fs::create_dir_all(&rio_config).unwrap();

        let shell_marker = dir.path().join("shell-invoked");
        let zellij_marker = dir.path().join("zellij-invoked");

        let fake_shell = write_exec(
            &bin_dir,
            "fake-shell",
            &format!(
                "#!/bin/sh\necho ran > \"{}\"\nexit 98\n",
                shell_marker.display()
            ),
        );
        let _fake_zellij = write_exec(
            &bin_dir,
            "zellij",
            &format!(
                "#!/bin/sh\necho ran > \"{}\"\nexit 99\n",
                zellij_marker.display()
            ),
        );

        Self {
            dir,
            fake_shell,
            shell_marker,
            zellij_marker,
            home,
            xdg_config,
            rio_config,
        }
    }

    fn run_cli(&self, args: &[&str]) -> Output {
        self.run_cli_with_env(args, &[])
    }

    fn run_cli_with_env(&self, args: &[&str], extra_env: &[(&str, &str)]) -> Output {
        let bin_dir = self.dir.path().join("bin");
        let mut cmd = Command::new(bin_path());
        cmd.args(args)
            .env_clear()
            .env("PATH", &bin_dir)
            .env("SHELL", &self.fake_shell)
            .env("ZELLIJ", "1")
            .env("HOME", &self.home)
            .env("XDG_CONFIG_HOME", &self.xdg_config)
            .env("RIO_CONFIG_HOME", &self.rio_config)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        cmd.output().unwrap()
    }

    fn assert_no_side_effects(&self) {
        assert!(
            !self.shell_marker.exists(),
            "completions must not execute SHELL"
        );
        assert!(
            !self.zellij_marker.exists(),
            "completions must not execute zellij"
        );
        assert_eq!(
            fs::read_dir(&self.home).unwrap().count(),
            0,
            "HOME must remain untouched"
        );
        assert_eq!(
            fs::read_dir(&self.xdg_config).unwrap().count(),
            0,
            "XDG_CONFIG_HOME must remain untouched"
        );
        assert_eq!(
            fs::read_dir(&self.rio_config).unwrap().count(),
            0,
            "RIO_CONFIG_HOME must remain untouched"
        );
    }
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

fn assert_contains_cli_tokens(shell: &str, script: &str) {
    assert!(
        !script.trim().is_empty(),
        "{shell} script must be non-empty"
    );
    for token in [
        "ouro-terminal-picker",
        "install",
        "uninstall",
        "print-config",
        "completions",
    ] {
        assert!(
            script.contains(token),
            "{shell} completion script missing expected CLI token `{token}`"
        );
    }
}

fn assert_contains_terminal_values(shell: &str, script: &str) {
    for value in ["kitty", "rio"] {
        assert!(
            script.contains(value),
            "{shell} completion script missing --terminal value `{value}`"
        );
    }
}

#[test]
fn help_lists_completions_subcommand_and_supported_shells() {
    let env = IsolatedEnv::new();

    let root_help = env.run_cli(&["--help"]);
    assert!(
        root_help.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&root_help.stderr)
    );
    let root_stdout = String::from_utf8_lossy(&root_help.stdout);
    assert!(
        root_stdout.contains("completions"),
        "--help should list completions subcommand:\n{root_stdout}"
    );

    let sub_help = env.run_cli(&["completions", "--help"]);
    assert!(
        sub_help.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&sub_help.stderr)
    );
    let sub_stdout = String::from_utf8_lossy(&sub_help.stdout);
    for shell in ["bash", "fish", "zsh"] {
        assert!(
            sub_stdout.contains(shell),
            "completions --help should mention {shell}:\n{sub_stdout}"
        );
    }
    env.assert_no_side_effects();
}

#[test]
fn completions_bash_emits_pure_valid_script_without_side_effects() {
    let env = IsolatedEnv::new();
    let out = env.run_cli(&["completions", "bash"]);

    assert!(
        out.status.success(),
        "expected success; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "completions stderr must be empty; got: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let script = String::from_utf8(out.stdout).expect("bash script must be valid UTF-8");
    assert_contains_cli_tokens("bash", &script);
    assert_contains_terminal_values("bash", &script);
    assert!(script.contains("--terminal"));
    assert!(script.contains("--dry-run"));
    assert!(script.contains("--force"));
    env.assert_no_side_effects();

    if Path::new("/usr/bin/bash").exists() {
        let script_path = env.dir.path().join("ouro-terminal-picker.bash");
        fs::write(&script_path, &script).unwrap();
        let check = Command::new("/usr/bin/bash")
            .args(["-n", script_path.to_str().unwrap()])
            .env_clear()
            .output()
            .unwrap();
        assert!(
            check.status.success(),
            "bash -n failed: {}",
            String::from_utf8_lossy(&check.stderr)
        );
    }
}

#[test]
fn completions_fish_emits_pure_valid_script_without_side_effects() {
    let env = IsolatedEnv::new();
    let out = env.run_cli(&["completions", "fish"]);

    assert!(
        out.status.success(),
        "expected success; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "completions stderr must be empty; got: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let script = String::from_utf8(out.stdout).expect("fish script must be valid UTF-8");
    assert_contains_cli_tokens("fish", &script);
    assert_contains_terminal_values("fish", &script);
    assert!(script.contains("-l terminal"));
    assert!(script.contains("-l dry-run"));
    assert!(script.contains("-l force"));
    env.assert_no_side_effects();

    if Path::new("/usr/bin/fish").exists() {
        let script_path = env.dir.path().join("ouro-terminal-picker.fish");
        let fish_home = env.dir.path().join("fish-home");
        fs::create_dir_all(&fish_home).unwrap();
        fs::write(&script_path, &script).unwrap();
        let check = Command::new("/usr/bin/fish")
            .args(["--no-execute", script_path.to_str().unwrap()])
            .env_clear()
            .env("HOME", &fish_home)
            .env("XDG_CONFIG_HOME", &fish_home)
            .env("XDG_DATA_HOME", &fish_home)
            .env("XDG_CACHE_HOME", &fish_home)
            .output()
            .unwrap();
        assert!(
            check.status.success(),
            "fish --no-execute failed: {}",
            String::from_utf8_lossy(&check.stderr)
        );
    }
}

#[test]
fn completions_zsh_emits_pure_script_without_side_effects() {
    let env = IsolatedEnv::new();
    let out = env.run_cli(&["completions", "zsh"]);

    assert!(
        out.status.success(),
        "expected success; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "completions stderr must be empty; got: {}",
        String::from_utf8_lossy(&out.stderr)
    );

    let script = String::from_utf8(out.stdout).expect("zsh script must be valid UTF-8");
    assert!(
        script.starts_with("#compdef ouro-terminal-picker"),
        "zsh script must start with #compdef header; got:\n{}",
        script.lines().take(5).collect::<Vec<_>>().join("\n")
    );
    assert_contains_cli_tokens("zsh", &script);
    assert_contains_terminal_values("zsh", &script);
    assert!(script.contains("--terminal"));
    assert!(script.contains("--dry-run"));
    assert!(script.contains("--force"));
    env.assert_no_side_effects();

    if Path::new("/usr/bin/zsh").exists() {
        let script_path = env.dir.path().join("_ouro-terminal-picker");
        fs::write(&script_path, &script).unwrap();
        let check = Command::new("/usr/bin/zsh")
            .args(["-n", script_path.to_str().unwrap()])
            .env_clear()
            .output()
            .unwrap();
        assert!(
            check.status.success(),
            "zsh -n failed: {}",
            String::from_utf8_lossy(&check.stderr)
        );
    }
}

#[test]
fn completions_standard_extra_shells_emit_scripts() {
    let env = IsolatedEnv::new();
    for shell in ["elvish", "powershell"] {
        let out = env.run_cli(&["completions", shell]);
        assert!(
            out.status.success(),
            "{shell} expected success; stderr: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        assert!(
            out.stderr.is_empty(),
            "{shell} stderr must be empty; got: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let script = String::from_utf8(out.stdout).expect("script must be valid UTF-8");
        assert_contains_cli_tokens(shell, &script);
        assert!(script.contains("--terminal"));
        assert!(script.contains("--dry-run"));
        assert!(script.contains("--force"));
    }
    env.assert_no_side_effects();
}

#[test]
fn completions_invalid_and_missing_shell_exit_nonzero_without_side_effects() {
    let env = IsolatedEnv::new();

    let invalid = env.run_cli(&["completions", "not-a-real-shell"]);
    assert!(
        !invalid.status.success(),
        "invalid shell must exit non-zero"
    );
    assert!(
        invalid.stdout.is_empty(),
        "invalid shell must not write stdout"
    );
    let invalid_stderr = String::from_utf8_lossy(&invalid.stderr);
    assert!(
        invalid_stderr.contains("not-a-real-shell")
            && !invalid_stderr.contains("unrecognized subcommand"),
        "invalid shell stderr should reject shell value, not subcommand; got:\n{invalid_stderr}"
    );

    let missing = env.run_cli(&["completions"]);
    assert!(
        !missing.status.success(),
        "missing shell argument must exit non-zero"
    );
    assert!(
        missing.stdout.is_empty(),
        "missing shell must not write stdout"
    );
    let missing_stderr = String::from_utf8_lossy(&missing.stderr);
    assert!(
        !missing_stderr.contains("unrecognized subcommand"),
        "missing shell stderr should require <SHELL>, not reject subcommand; got:\n{missing_stderr}"
    );

    env.assert_no_side_effects();
}

#[test]
fn completions_runs_before_tracing_init_even_with_log_env_set() {
    let env = IsolatedEnv::new();
    let out = env.run_cli_with_env(
        &["completions", "bash"],
        &[("OURO_TERMINAL_PICKER_LOG", "trace")],
    );
    assert!(
        out.status.success(),
        "completions must succeed before tracing setup even when OURO_TERMINAL_PICKER_LOG is set; stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert!(
        out.stderr.is_empty(),
        "completions stderr must stay empty with OURO_TERMINAL_PICKER_LOG=trace; got: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let script = String::from_utf8(out.stdout).expect("bash script must be valid UTF-8");
    assert_contains_cli_tokens("bash", &script);
    env.assert_no_side_effects();
}

#[test]
fn completions_broken_stdout_exits_nonzero_without_panicking() {
    let env = IsolatedEnv::new();
    let py = r#"
import os, subprocess, sys
r, w = os.pipe()
os.close(r)
try:
    proc = subprocess.run(
        [sys.argv[1], "completions", sys.argv[2]],
        stdin=subprocess.DEVNULL,
        stdout=w,
        stderr=subprocess.PIPE,
        env={
            "PATH": sys.argv[3],
            "SHELL": sys.argv[4],
            "ZELLIJ": "1",
            "HOME": sys.argv[5],
            "XDG_CONFIG_HOME": sys.argv[6],
            "RIO_CONFIG_HOME": sys.argv[7],
        },
    )
finally:
    os.close(w)
sys.stderr.buffer.write(proc.stderr)
sys.exit(proc.returncode if proc.returncode >= 0 else 128 - proc.returncode)
"#;

    for shell in ["bash", "fish", "zsh", "elvish", "powershell"] {
        let out = Command::new("/usr/bin/python3")
            .args(["-I", "-c", py])
            .arg(bin_path())
            .arg(shell)
            .arg(env.dir.path().join("bin"))
            .arg(&env.fake_shell)
            .arg(&env.home)
            .arg(&env.xdg_config)
            .arg(&env.rio_config)
            .output()
            .unwrap();

        let stderr = String::from_utf8_lossy(&out.stderr);
        assert_eq!(
            out.status.code(),
            Some(1),
            "{shell}: broken stdout must exit cleanly with status 1 (not 0, 2, or 101); stderr: {stderr}"
        );
        assert!(
            !stderr.contains("panicked at"),
            "{shell}: broken stdout must not panic; stderr: {stderr}"
        );
        assert!(
            !stderr.contains("unrecognized subcommand"),
            "{shell}: broken stdout must reach completion generation; stderr: {stderr}"
        );
    }
    env.assert_no_side_effects();
}
