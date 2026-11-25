//! Integration tests for CLI subcommands, Kitty/Rio config wiring, and bypass paths.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tempfile::TempDir;

fn bin_path() -> &'static Path {
    Path::new(env!("CARGO_BIN_EXE_ouro-terminal-picker"))
}

fn write_exec(dir: &Path, name: &str, body: &str) -> PathBuf {
    use std::io::Write;
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

#[test]
fn bypasses_picker_when_not_a_tty() {
    let dir = TempDir::new().unwrap();
    let marker = dir.path().join("shell-ran");
    let fake_shell = write_exec(
        dir.path(),
        "fake-shell",
        &format!(
            "#!/bin/sh\necho non-tty-ok > {}\nexit 0\n",
            marker.display()
        ),
    );
    let _fake_zellij = write_exec(
        dir.path(),
        "zellij",
        "#!/bin/sh\necho SHOULD_NOT_RUN >&2\nexit 99\n",
    );

    let out = Command::new(bin_path())
        .env_clear()
        .env("PATH", dir.path())
        .env("SHELL", &fake_shell)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()
        .unwrap();

    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(fs::read_to_string(&marker).unwrap().trim(), "non-tty-ok");
}

#[test]
fn cli_kitty_install_dry_run_apply_validate_and_uninstall() {
    let dir = TempDir::new().unwrap();
    let xdg = dir.path().join("xdg");
    let kitty_dir = xdg.join("kitty");
    fs::create_dir_all(&kitty_dir).unwrap();
    let kitty_conf = kitty_dir.join("kitty.conf");
    let original = b"font_size 11.5\n";
    fs::write(&kitty_conf, original).unwrap();

    // Dry-run does not modify disk
    let dry = Command::new(bin_path())
        .args(["install", "--terminal", "kitty", "--dry-run"])
        .env_clear()
        .env("XDG_CONFIG_HOME", &xdg)
        .output()
        .unwrap();
    assert!(
        dry.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&dry.stderr)
    );
    assert!(
        String::from_utf8_lossy(&dry.stdout).contains("include ouro-terminal-picker.conf"),
        "stdout: {}",
        String::from_utf8_lossy(&dry.stdout)
    );
    assert_eq!(fs::read(&kitty_conf).unwrap(), original);
    assert!(!kitty_dir.join("ouro-terminal-picker.conf").exists());

    // Real install
    let inst = Command::new(bin_path())
        .args(["install", "--terminal", "kitty"])
        .env_clear()
        .env("XDG_CONFIG_HOME", &xdg)
        .output()
        .unwrap();
    assert!(
        inst.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&inst.stderr)
    );
    assert!(kitty_dir.join("ouro-terminal-picker.conf").exists());

    // Validate with Kitty's own parser when kitty is installed on this host
    if Path::new("/usr/bin/kitty").exists() {
        let py = format!(
            "from kitty.config import load_config; bad=[]; o=load_config('{}', accumulate_bad_lines=bad); print(o.shell); assert bad == [], bad",
            kitty_conf.display()
        );
        let check = Command::new("/usr/bin/kitty")
            .args(["+runpy", &py])
            .output()
            .unwrap();
        assert!(
            check.status.success(),
            "kitty parser failed: stdout={} stderr={}",
            String::from_utf8_lossy(&check.stdout),
            String::from_utf8_lossy(&check.stderr)
        );
        let parsed_shell = String::from_utf8_lossy(&check.stdout).trim().to_owned();
        assert_eq!(
            Path::new(&parsed_shell).canonicalize().unwrap(),
            bin_path().canonicalize().unwrap()
        );
    }

    // Uninstall restores exact original bytes
    let uninst = Command::new(bin_path())
        .args(["uninstall", "--terminal", "kitty"])
        .env_clear()
        .env("XDG_CONFIG_HOME", &xdg)
        .output()
        .unwrap();
    assert!(
        uninst.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninst.stderr)
    );
    assert_eq!(fs::read(&kitty_conf).unwrap(), original);
    assert!(!kitty_dir.join("ouro-terminal-picker.conf").exists());
}

#[test]
fn cli_rio_install_force_backup_and_uninstall_and_print_config() {
    let dir = TempDir::new().unwrap();
    let rio_home = dir.path().join("rio-cfg");
    fs::create_dir(&rio_home).unwrap();
    let cfg = rio_home.join("config.toml");
    let foreign = b"# user header\n[shell]\nprogram = \"/usr/bin/bash\"\nargs = [\"-l\"]\n";
    fs::write(&cfg, foreign).unwrap();

    // Without --force, foreign [shell] is refused
    let refused = Command::new(bin_path())
        .args(["install", "--terminal", "rio"])
        .env_clear()
        .env("RIO_CONFIG_HOME", &rio_home)
        .output()
        .unwrap();
    assert!(
        !refused.status.success(),
        "foreign [shell] must fail without --force"
    );
    assert_eq!(fs::read(&cfg).unwrap(), foreign);

    // With --force, creates backup and writes [shell]
    let forced = Command::new(bin_path())
        .args(["install", "--terminal", "rio", "--force"])
        .env_clear()
        .env("RIO_CONFIG_HOME", &rio_home)
        .output()
        .unwrap();
    assert!(
        forced.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&forced.stderr)
    );
    let updated = fs::read_to_string(&cfg).unwrap();
    assert!(updated.contains("# user header"));
    assert!(updated.contains("args = []"));

    // print-config works for both terminals
    let pc_kitty = Command::new(bin_path())
        .args(["print-config", "--terminal", "kitty"])
        .output()
        .unwrap();
    assert!(pc_kitty.status.success());
    assert!(String::from_utf8_lossy(&pc_kitty.stdout).contains("shell "));

    let pc_rio = Command::new(bin_path())
        .args(["print-config", "--terminal", "rio"])
        .output()
        .unwrap();
    assert!(pc_rio.status.success());
    assert!(String::from_utf8_lossy(&pc_rio.stdout).contains("[shell]"));

    // Uninstall removes [shell]
    let uninst = Command::new(bin_path())
        .args(["uninstall", "--terminal", "rio"])
        .env_clear()
        .env("RIO_CONFIG_HOME", &rio_home)
        .output()
        .unwrap();
    assert!(
        uninst.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&uninst.stderr)
    );
    let after_uninst = fs::read_to_string(&cfg).unwrap();
    assert!(!after_uninst.contains("[shell]"));
    assert!(after_uninst.contains("# user header"));
}
