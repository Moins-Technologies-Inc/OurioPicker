//! Terminal raw-mode, alternate-screen lifecycle, panic restoration, and key mapping.

use std::env;
use std::io::{self, IsTerminal, Write};
use std::sync::Once;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::style::{Attribute, SetAttribute};
use crossterm::terminal::{
    Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode,
    enable_raw_mode, size,
};
use crossterm::{execute, queue};
use thiserror::Error;

use crate::launch::Choice;
use crate::picker::{Key, Picker, Step};

static PANIC_HOOK_ONCE: Once = Once::new();

/// Errors from terminal I/O during picker interaction.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::TerminalError;
///
/// let err = TerminalError::Io {
///     source: std::io::Error::from(std::io::ErrorKind::BrokenPipe),
/// };
/// assert!(err.to_string().contains("terminal I/O"));
/// ```
#[derive(Debug, Error)]
pub enum TerminalError {
    /// Underlying terminal read/write failure.
    #[error("terminal I/O error: {source}")]
    Io {
        /// Underlying OS error.
        #[source]
        source: io::Error,
    },
}

/// Map a [`crossterm::event::KeyEvent`] to a pure picker [`Key`].
///
/// Ignores key-release events so protocols that emit release events do not duplicate actions.
///
/// # Examples
///
/// ```
/// use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
/// use ouro_terminal_picker::{Key, map_key_event};
///
/// let ev = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
/// assert_eq!(map_key_event(ev), Some(Key::CtrlC));
/// ```
pub fn map_key_event(event: KeyEvent) -> Option<Key> {
    if event.kind == KeyEventKind::Release {
        return None;
    }

    if event.modifiers.contains(KeyModifiers::CONTROL) {
        return match event.code {
            KeyCode::Char('c' | 'C') => Some(Key::CtrlC),
            _ => None,
        };
    }

    if event.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }

    match event.code {
        KeyCode::Up => Some(Key::Up),
        KeyCode::Down => Some(Key::Down),
        KeyCode::Enter => Some(Key::Enter),
        KeyCode::Esc => Some(Key::Esc),
        KeyCode::Backspace => Some(Key::Backspace),
        KeyCode::Char(ch) => Some(Key::Char(ch)),
        _ => None,
    }
}

/// Return `true` when both stdin and stdout are connected to an interactive terminal.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::is_interactive_tty;
///
/// let _ = is_interactive_tty();
/// ```
pub fn is_interactive_tty() -> bool {
    io::stdin().is_terminal() && io::stdout().is_terminal()
}

/// Return `true` when `NO_COLOR` is set and non-empty.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::no_color_enabled;
///
/// let _ = no_color_enabled();
/// ```
pub fn no_color_enabled() -> bool {
    env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty())
}

/// Best-effort restoration of main screen, cursor visibility, and cooked mode.
///
/// Safe to call multiple times or from a panic hook.
///
/// # Examples
///
/// ```no_run
/// use ouro_terminal_picker::restore_terminal;
///
/// restore_terminal();
/// ```
pub fn restore_terminal() {
    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = execute!(
        stdout,
        SetAttribute(Attribute::Reset),
        Show,
        LeaveAlternateScreen
    );
    let _ = stdout.flush();
}

/// Install a panic hook that restores the terminal before reporting a short notice.
///
/// # Examples
///
/// ```no_run
/// use ouro_terminal_picker::install_panic_hook;
///
/// install_panic_hook();
/// ```
pub fn install_panic_hook() {
    PANIC_HOOK_ONCE.call_once(|| {
        std::panic::set_hook(Box::new(|_info| {
            restore_terminal();
            let _ = writeln!(
                io::stderr(),
                "ouro-terminal-picker encountered an internal error; opening your shell."
            );
        }));
    });
}

/// RAII guard that enables raw mode and enters the alternate screen, restoring cooked mode
/// and the main screen unconditionally on drop (including during stack unwinding).
///
/// # Examples
///
/// ```no_run
/// use ouro_terminal_picker::TerminalGuard;
///
/// let _guard = TerminalGuard::enter()?;
/// # Ok::<(), ouro_terminal_picker::TerminalError>(())
/// ```
#[derive(Debug)]
pub struct TerminalGuard;

impl TerminalGuard {
    /// Enter raw mode, switch to the alternate screen, and hide the cursor.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ouro_terminal_picker::TerminalGuard;
    ///
    /// let guard = TerminalGuard::enter()?;
    /// drop(guard);
    /// # Ok::<(), ouro_terminal_picker::TerminalError>(())
    /// ```
    pub fn enter() -> Result<Self, TerminalError> {
        enable_raw_mode().map_err(|source| TerminalError::Io { source })?;
        let mut stdout = io::stdout();
        if let Err(source) = execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = disable_raw_mode();
            return Err(TerminalError::Io { source });
        }
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore_terminal();
    }
}

fn draw(picker: &Picker, no_color: bool) -> Result<(), TerminalError> {
    let (width, height) = size().unwrap_or((80, 24));
    let lines = picker.render_lines(width, height);
    let mut stdout = io::stdout();

    queue!(stdout, MoveTo(0, 0), Clear(ClearType::All))
        .map_err(|source| TerminalError::Io { source })?;

    for (row_idx, line) in lines.iter().enumerate() {
        let row = u16::try_from(row_idx).unwrap_or(u16::MAX);
        queue!(stdout, MoveTo(0, row)).map_err(|source| TerminalError::Io { source })?;
        let is_selected = line.starts_with("> ");
        if !no_color && is_selected {
            queue!(stdout, SetAttribute(Attribute::Reverse))
                .map_err(|source| TerminalError::Io { source })?;
        }
        stdout
            .write_all(line.as_bytes())
            .map_err(|source| TerminalError::Io { source })?;
        if !no_color && is_selected {
            queue!(stdout, SetAttribute(Attribute::Reset))
                .map_err(|source| TerminalError::Io { source })?;
        }
    }

    stdout
        .flush()
        .map_err(|source| TerminalError::Io { source })?;
    Ok(())
}

/// Run the interactive alternate-screen TUI for `picker`, restoring the terminal on every exit.
///
/// # Examples
///
/// ```no_run
/// use ouro_terminal_picker::{Picker, run_tui};
///
/// let mut picker = Picker::new(Vec::new(), None);
/// let choice = run_tui(&mut picker)?;
/// println!("selected kind: {}", choice.kind());
/// # Ok::<(), ouro_terminal_picker::TerminalError>(())
/// ```
pub fn run_tui(picker: &mut Picker) -> Result<Choice, TerminalError> {
    let _guard = TerminalGuard::enter()?;
    let no_color = no_color_enabled();

    loop {
        draw(picker, no_color)?;
        match event::read().map_err(|source| TerminalError::Io { source })? {
            Event::Key(key_event) => {
                if let Some(key) = map_key_event(key_event)
                    && let Step::Done(choice) = picker.handle(key)
                {
                    return Ok(choice);
                }
            }
            Event::Resize(_, _) => {}
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

    use super::*;

    #[test]
    fn maps_navigation_prompt_and_ctrl_c_keys() {
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE)),
            Some(Key::Up)
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            Some(Key::Down)
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some(Key::Enter)
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)),
            Some(Key::Esc)
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE)),
            Some(Key::Backspace)
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE)),
            Some(Key::Char('n'))
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL)),
            Some(Key::CtrlC)
        );
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('C'), KeyModifiers::CONTROL)),
            Some(Key::CtrlC)
        );
    }

    #[test]
    fn ignores_release_events_and_other_control_chords() {
        let release = KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        };
        assert_eq!(map_key_event(release), None);
        assert_eq!(
            map_key_event(KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)),
            None
        );
    }
}
