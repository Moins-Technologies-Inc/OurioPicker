//! `ouro-terminal-picker` binary entry point.

use std::env;
use std::io::{self, Write};
use std::panic::AssertUnwindSafe;
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use clap_complete::aot::{Generator, Shell};
use ouro_terminal_picker::config::{kitty, rio};
use ouro_terminal_picker::{
    Choice, Picker, TerminalKind, apply_plan, exec_shell, find_in_path, install_panic_hook,
    is_interactive_tty, list_sessions_with, no_color_enabled, resolve_shell, restore_terminal,
    run_choice, run_tui, should_bypass_picker,
};
use tracing_subscriber::EnvFilter;
use tracing_subscriber::fmt::MakeWriter;

const LOG_ENV_VAR: &str = "OURO_TERMINAL_PICKER_LOG";

#[derive(Debug, Parser)]
#[command(
    name = "ouro-terminal-picker",
    version,
    about = "In-terminal Zellij session picker and terminal configuration helper"
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Wire ouro-terminal-picker into a terminal emulator's configuration.
    Install {
        /// Target terminal emulator.
        #[arg(long, value_enum)]
        terminal: CliTerminal,
        /// Print planned changes without writing files.
        #[arg(long)]
        dry_run: bool,
        /// Replace an existing different [shell] table in Rio/Ourio (saves a 0600 backup).
        #[arg(long)]
        force: bool,
    },
    /// Remove ouro-terminal-picker wiring from a terminal emulator's configuration.
    Uninstall {
        /// Target terminal emulator.
        #[arg(long, value_enum)]
        terminal: CliTerminal,
        /// Print planned removals without modifying files.
        #[arg(long)]
        dry_run: bool,
    },
    /// Print the configuration snippet for manual terminal setup.
    PrintConfig {
        /// Target terminal emulator.
        #[arg(long, value_enum)]
        terminal: CliTerminal,
    },
    /// Generate shell completion scripts for the requested shell.
    Completions {
        /// Target shell to generate completions for.
        #[arg(value_enum)]
        shell: Shell,
    },
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum CliTerminal {
    Kitty,
    Rio,
}

impl From<CliTerminal> for TerminalKind {
    fn from(value: CliTerminal) -> Self {
        match value {
            CliTerminal::Kitty => Self::Kitty,
            CliTerminal::Rio => Self::Rio,
        }
    }
}

struct LogState {
    passthrough: bool,
    buffer: Vec<u8>,
}

#[derive(Clone)]
struct LogSink {
    state: Arc<Mutex<LogState>>,
}

struct LogSinkWriter {
    state: Arc<Mutex<LogState>>,
}

impl Default for LogSink {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(LogState {
                passthrough: true,
                buffer: Vec::new(),
            })),
        }
    }
}

impl Write for LogSinkWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if let Ok(mut guard) = self.state.lock() {
            if guard.passthrough {
                let _ = io::stderr().write_all(buf);
                let _ = io::stderr().flush();
            } else {
                guard.buffer.extend_from_slice(buf);
            }
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        if let Ok(guard) = self.state.lock()
            && guard.passthrough
        {
            let _ = io::stderr().flush();
        }
        Ok(())
    }
}

impl<'a> MakeWriter<'a> for LogSink {
    type Writer = LogSinkWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogSinkWriter {
            state: Arc::clone(&self.state),
        }
    }
}

impl LogSink {
    fn buffer_until_restore(&self) {
        if let Ok(mut guard) = self.state.lock() {
            guard.passthrough = false;
        }
    }

    fn resume_passthrough(&self) {
        if let Ok(mut guard) = self.state.lock() {
            if !guard.buffer.is_empty() {
                let _ = io::stderr().write_all(&guard.buffer);
                let _ = io::stderr().flush();
                guard.buffer.clear();
            }
            guard.passthrough = true;
        }
    }
}

fn init_logging() -> Option<LogSink> {
    let directive = env::var(LOG_ENV_VAR).ok()?;
    if directive.trim().is_empty() {
        return None;
    }
    let filter = EnvFilter::try_new(directive).unwrap_or_else(|_| EnvFilter::new("info"));
    let sink = LogSink::default();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(!no_color_enabled())
        .with_writer(sink.clone())
        .try_init();
    Some(sink)
}

fn buffer_logs(logs: Option<&LogSink>) {
    if let Some(sink) = logs {
        sink.buffer_until_restore();
    }
}

fn resume_logs(logs: Option<&LogSink>) {
    if let Some(sink) = logs {
        sink.resume_passthrough();
    }
}

fn current_binary_path() -> Result<PathBuf> {
    let exe = env::current_exe().context("failed to determine current executable path")?;
    Ok(exe.canonicalize().unwrap_or(exe))
}

fn run_install(terminal: TerminalKind, dry_run: bool, force: bool) -> Result<()> {
    let bin = current_binary_path()?;
    let plan = match terminal {
        TerminalKind::Kitty => {
            let dir = kitty::kitty_config_dir(
                env::var_os("XDG_CONFIG_HOME").as_deref(),
                env::var_os("HOME").as_deref(),
            )?;
            kitty::plan_install(&dir, &bin)?
        }
        TerminalKind::Rio => {
            let cfg = rio::rio_config_path(
                env::var_os("RIO_CONFIG_HOME").as_deref(),
                env::var_os("XDG_CONFIG_HOME").as_deref(),
                env::var_os("HOME").as_deref(),
            )?;
            rio::plan_install(&cfg, &bin, force)?
        }
    };

    if dry_run {
        print!("{}", plan.format_dry_run());
        return Ok(());
    }

    if plan.is_empty() {
        println!("{terminal}: already configured; no changes needed.");
        return Ok(());
    }

    apply_plan(&plan)?;
    println!(
        "{terminal}: installed configuration ({} operation(s)).",
        plan.ops.len()
    );
    Ok(())
}

fn run_uninstall(terminal: TerminalKind, dry_run: bool) -> Result<()> {
    let bin = current_binary_path()?;
    let plan = match terminal {
        TerminalKind::Kitty => {
            let dir = kitty::kitty_config_dir(
                env::var_os("XDG_CONFIG_HOME").as_deref(),
                env::var_os("HOME").as_deref(),
            )?;
            kitty::plan_uninstall(&dir)?
        }
        TerminalKind::Rio => {
            let cfg = rio::rio_config_path(
                env::var_os("RIO_CONFIG_HOME").as_deref(),
                env::var_os("XDG_CONFIG_HOME").as_deref(),
                env::var_os("HOME").as_deref(),
            )?;
            rio::plan_uninstall(&cfg, &bin)?
        }
    };

    if dry_run {
        print!("{}", plan.format_dry_run());
        return Ok(());
    }

    if plan.is_empty() {
        println!("{terminal}: no managed configuration found; no changes needed.");
        return Ok(());
    }

    apply_plan(&plan)?;
    println!(
        "{terminal}: removed configuration ({} operation(s)).",
        plan.ops.len()
    );
    Ok(())
}

fn run_print_config(terminal: TerminalKind) -> Result<()> {
    let bin = current_binary_path()?;
    match terminal {
        TerminalKind::Kitty => {
            let include_bytes = kitty::render_include_file(&bin)?;
            let include_text = String::from_utf8_lossy(&include_bytes);
            println!(
                "# File: $XDG_CONFIG_HOME/kitty/{}",
                kitty::INCLUDE_FILE_NAME
            );
            print!("{include_text}");
            println!("\n# Add to $XDG_CONFIG_HOME/kitty/kitty.conf:");
            println!("{}", kitty::manual_include_snippet());
        }
        TerminalKind::Rio => {
            let snippet = rio::render_shell_snippet(&bin)?;
            print!("{snippet}");
        }
    }
    Ok(())
}

fn write_completions(shell: Shell, out: &mut dyn Write) -> Result<()> {
    let mut cmd = Cli::command();
    let bin_name = cmd.get_name().to_owned();
    cmd.set_bin_name(bin_name);
    cmd.build();
    let mut buf = Vec::new();
    shell
        .try_generate(&cmd, &mut buf)
        .context("failed to generate shell completions")?;
    out.write_all(&buf)
        .context("failed to write shell completions")?;
    out.flush().context("failed to flush shell completions")?;
    Ok(())
}

fn run_completions(shell: Shell) -> Result<()> {
    let stdout = io::stdout();
    let mut lock = stdout.lock();
    write_completions(shell, &mut lock)
}

fn run_picker_mode(logs: Option<&LogSink>) -> Result<ExitCode> {
    install_panic_hook();
    let shell = resolve_shell();
    let zellij_bin = find_in_path("zellij", env::var_os("PATH").as_deref());
    let zellij_env = env::var_os("ZELLIJ");

    if should_bypass_picker(
        zellij_env.as_deref(),
        zellij_bin.as_deref(),
        is_interactive_tty(),
    ) {
        resume_logs(logs);
        return Err(exec_shell(&shell).into());
    }

    let Some(zellij_path) = zellij_bin else {
        resume_logs(logs);
        return Err(exec_shell(&shell).into());
    };

    buffer_logs(logs);
    let choice_result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        let mut picker = match list_sessions_with(&zellij_path) {
            Ok(sessions) => Picker::new(sessions, None),
            Err(err) => Picker::new(Vec::new(), Some(err.to_string())),
        };
        run_tui(&mut picker)
    }));

    // Ensure terminal is restored before switching tracing back to pass-through stderr.
    let choice = match choice_result {
        Ok(Ok(choice)) => {
            resume_logs(logs);
            choice
        }
        Ok(Err(err)) => {
            restore_terminal();
            resume_logs(logs);
            tracing::warn!(error = %err, "picker TUI failed; falling back to shell");
            Choice::Shell
        }
        Err(_) => {
            restore_terminal();
            resume_logs(logs);
            tracing::warn!("picker panicked; falling back to shell");
            Choice::Shell
        }
    };

    let code = run_choice(&choice, &zellij_path, &shell)?;
    let byte = u8::try_from(code).unwrap_or(1);
    Ok(ExitCode::from(byte))
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let result: Result<ExitCode> = match cli.command {
        Some(Command::Completions { shell }) => run_completions(shell).map(|()| ExitCode::SUCCESS),
        Some(Command::Install {
            terminal,
            dry_run,
            force,
        }) => {
            let logs = init_logging();
            let res = run_install(terminal.into(), dry_run, force).map(|()| ExitCode::SUCCESS);
            resume_logs(logs.as_ref());
            res
        }
        Some(Command::Uninstall { terminal, dry_run }) => {
            let logs = init_logging();
            let res = run_uninstall(terminal.into(), dry_run).map(|()| ExitCode::SUCCESS);
            resume_logs(logs.as_ref());
            res
        }
        Some(Command::PrintConfig { terminal }) => {
            let logs = init_logging();
            let res = run_print_config(terminal.into()).map(|()| ExitCode::SUCCESS);
            resume_logs(logs.as_ref());
            res
        }
        None => {
            let logs = init_logging();
            let res = run_picker_mode(logs.as_ref());
            resume_logs(logs.as_ref());
            res
        }
    };

    match result {
        Ok(code) => code,
        Err(err) => {
            let _ = writeln!(io::stderr(), "ouro-terminal-picker: {err:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FailingWriter;

    impl Write for FailingWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "simulated broken pipe",
            ))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct FailingFlushWriter;

    impl Write for FailingFlushWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "simulated flush failure",
            ))
        }
    }

    #[test]
    fn write_completions_returns_error_on_failing_writer() {
        for &shell in Shell::value_variants() {
            let mut writer = FailingWriter;
            let err = write_completions(shell, &mut writer).unwrap_err();
            let chain = format!("{err:#}");
            assert!(
                chain.contains("simulated broken pipe"),
                "{shell}: unexpected error chain: {chain}"
            );
        }
    }

    #[test]
    fn write_completions_returns_error_on_failing_flush() {
        for &shell in Shell::value_variants() {
            let mut writer = FailingFlushWriter;
            let err = write_completions(shell, &mut writer).unwrap_err();
            let chain = format!("{err:#}");
            assert!(
                chain.contains("simulated flush failure"),
                "{shell}: unexpected error chain: {chain}"
            );
        }
    }

    #[test]
    fn write_completions_succeeds_for_all_clap_shells() {
        for &shell in Shell::value_variants() {
            let mut buf = Vec::new();
            write_completions(shell, &mut buf).unwrap();
            let text = String::from_utf8(buf).unwrap();
            assert!(
                text.contains("ouro-terminal-picker"),
                "{shell} completion missing binary name"
            );
        }
    }
}
