//! Pure state machine and text layout for the in-terminal session picker.

use crate::launch::Choice;
use crate::name::SessionName;
use crate::sessions::{Session, SessionState};

const MAX_PROMPT_BUFFER: usize = 256;

/// Logical key press consumed by [`Picker::handle`].
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::Key;
///
/// assert_ne!(Key::Enter, Key::Esc);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// Move selection up.
    Up,
    /// Move selection down.
    Down,
    /// Confirm the highlighted item or session name.
    Enter,
    /// Cancel prompt back to list, or pick plain shell from list.
    Esc,
    /// Delete the last character in the name prompt.
    Backspace,
    /// Printable character input.
    Char(char),
    /// Ctrl-C anywhere picks plain shell.
    CtrlC,
}

/// Active screen mode inside [`Picker`].
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::Mode;
///
/// assert_ne!(Mode::List, Mode::NamePrompt);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Selecting from existing sessions, New session, or Plain shell.
    List,
    /// Typing an optional name for a new Zellij session.
    NamePrompt,
}

/// Result of feeding one [`Key`] into [`Picker::handle`].
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::{Choice, Step};
///
/// assert_eq!(Step::Done(Choice::Shell), Step::Done(Choice::Shell));
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Keep running the picker UI.
    Continue,
    /// Exit the picker and execute the selected [`Choice`].
    Done(Choice),
}

/// Pure, terminal-independent picker state machine.
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::{Choice, Key, Picker, Step};
///
/// let mut picker = Picker::new(Vec::new(), None);
/// assert_eq!(picker.handle(Key::Esc), Step::Done(Choice::Shell));
/// ```
#[derive(Debug)]
pub struct Picker {
    sessions: Vec<Session>,
    list_error: Option<String>,
    selected: usize,
    mode: Mode,
    input: String,
    validation_error: Option<String>,
}

impl Picker {
    /// Construct a new picker with discovered sessions and an optional listing error.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::{Mode, Picker};
    ///
    /// let picker = Picker::new(Vec::new(), None);
    /// assert_eq!(picker.mode(), Mode::List);
    /// assert_eq!(picker.selected(), 0);
    /// ```
    pub fn new(sessions: Vec<Session>, list_error: Option<String>) -> Self {
        Self {
            sessions,
            list_error,
            selected: 0,
            mode: Mode::List,
            input: String::new(),
            validation_error: None,
        }
    }

    /// Current picker mode.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::{Mode, Picker};
    ///
    /// assert_eq!(Picker::new(Vec::new(), None).mode(), Mode::List);
    /// ```
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// Currently highlighted row index (`0..sessions.len() + 2`).
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::Picker;
    ///
    /// assert_eq!(Picker::new(Vec::new(), None).selected(), 0);
    /// ```
    pub const fn selected(&self) -> usize {
        self.selected
    }

    /// Current text in the new-session name prompt.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::Picker;
    ///
    /// assert_eq!(Picker::new(Vec::new(), None).input(), "");
    /// ```
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Current inline validation error in the name prompt, if any.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::Picker;
    ///
    /// assert!(Picker::new(Vec::new(), None).validation_error().is_none());
    /// ```
    pub fn validation_error(&self) -> Option<&str> {
        self.validation_error.as_deref()
    }

    /// Session-listing error message, if `zellij list-sessions` failed.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::Picker;
    ///
    /// let picker = Picker::new(Vec::new(), Some("socket error".to_owned()));
    /// assert_eq!(picker.list_error(), Some("socket error"));
    /// ```
    pub fn list_error(&self) -> Option<&str> {
        self.list_error.as_deref()
    }

    /// Discovered sessions held by this picker.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::Picker;
    ///
    /// assert!(Picker::new(Vec::new(), None).sessions().is_empty());
    /// ```
    pub fn sessions(&self) -> &[Session] {
        &self.sessions
    }

    /// Apply one [`Key`] event and return the next [`Step`].
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::{Choice, Key, Picker, Step};
    ///
    /// let mut picker = Picker::new(Vec::new(), None);
    /// assert_eq!(picker.handle(Key::Char('s')), Step::Done(Choice::Shell));
    /// ```
    pub fn handle(&mut self, key: Key) -> Step {
        if key == Key::CtrlC {
            return Step::Done(Choice::Shell);
        }

        match self.mode {
            Mode::List => self.handle_list(key),
            Mode::NamePrompt => self.handle_prompt(key),
        }
    }

    fn total_items(&self) -> usize {
        self.sessions.len() + 2
    }

    fn new_session_index(&self) -> usize {
        self.sessions.len()
    }

    fn handle_list(&mut self, key: Key) -> Step {
        let last = self.total_items().saturating_sub(1);
        match key {
            Key::Up | Key::Char('k') => {
                self.selected = self.selected.saturating_sub(1);
                Step::Continue
            }
            Key::Down | Key::Char('j') => {
                self.selected = (self.selected + 1).min(last);
                Step::Continue
            }
            Key::Enter => {
                if let Some(session) = self.sessions.get(self.selected) {
                    Step::Done(Choice::Attach(session.name.clone()))
                } else if self.selected == self.new_session_index() {
                    self.enter_name_prompt();
                    Step::Continue
                } else {
                    Step::Done(Choice::Shell)
                }
            }
            Key::Char('n') => {
                self.selected = self.new_session_index();
                self.enter_name_prompt();
                Step::Continue
            }
            Key::Char('s') | Key::Esc => Step::Done(Choice::Shell),
            Key::Backspace | Key::Char(_) | Key::CtrlC => Step::Continue,
        }
    }

    fn enter_name_prompt(&mut self) {
        self.mode = Mode::NamePrompt;
        self.input.clear();
        self.validation_error = None;
    }

    fn handle_prompt(&mut self, key: Key) -> Step {
        match key {
            Key::Enter => {
                if self.input.is_empty() {
                    return Step::Done(Choice::Create(None));
                }
                match SessionName::new(&self.input) {
                    Ok(name) => Step::Done(Choice::Create(Some(name))),
                    Err(err) => {
                        self.validation_error = Some(err.to_string());
                        Step::Continue
                    }
                }
            }
            Key::Esc => {
                self.mode = Mode::List;
                self.validation_error = None;
                Step::Continue
            }
            Key::Backspace => {
                let _ = self.input.pop();
                self.validation_error = None;
                Step::Continue
            }
            Key::Char(ch) => {
                if !ch.is_control() && self.input.chars().count() < MAX_PROMPT_BUFFER {
                    self.input.push(ch);
                    self.validation_error = None;
                }
                Step::Continue
            }
            Key::Up | Key::Down | Key::CtrlC => Step::Continue,
        }
    }

    /// Render the picker as plain, width-bounded lines suitable for terminals `20x5` and wider.
    ///
    /// Uses text-only markers (`> `, `[current]`, `[running]`, `[exited]`) and no colour escapes.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::Picker;
    ///
    /// let picker = Picker::new(Vec::new(), None);
    /// let lines = picker.render_lines(40, 10);
    /// assert!(!lines.is_empty());
    /// assert!(lines.len() <= 10);
    /// assert!(lines.iter().all(|l| l.chars().count() <= 40));
    /// ```
    pub fn render_lines(&self, width: u16, height: u16) -> Vec<String> {
        let max_w = usize::from(width.max(1));
        let max_h = usize::from(height.max(1));

        match self.mode {
            Mode::List => self.render_list(max_w, max_h),
            Mode::NamePrompt => self.render_prompt(max_w, max_h),
        }
    }

    fn render_list(&self, max_w: usize, max_h: usize) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(truncate("Zellij sessions", max_w));

        if let Some(err) = &self.list_error
            && max_h >= 4
        {
            lines.push(truncate(
                &format!("Could not list Zellij sessions: {err}"),
                max_w,
            ));
        }

        let footer_budget = usize::from(max_h >= 5);
        let available_rows = max_h
            .saturating_sub(lines.len())
            .saturating_sub(footer_budget)
            .max(1);

        let all_rows = self.list_rows();
        let total = all_rows.len();
        let start = if total <= available_rows {
            0
        } else {
            let half = available_rows / 2;
            let ideal = self.selected.saturating_sub(half);
            ideal.min(total.saturating_sub(available_rows))
        };

        for row in all_rows.into_iter().skip(start).take(available_rows) {
            lines.push(truncate(&row, max_w));
        }

        if footer_budget == 1 && lines.len() < max_h {
            lines.push(truncate("Enter: open | n: new | s/Esc: shell", max_w));
        }

        lines.truncate(max_h);
        lines
    }

    fn list_rows(&self) -> Vec<String> {
        let mut rows = Vec::with_capacity(self.total_items());
        for (idx, session) in self.sessions.iter().enumerate() {
            let marker = if idx == self.selected { "> " } else { "  " };
            let tag = match session.state {
                SessionState::Current => "[current]",
                SessionState::Running => "[running]",
                SessionState::Exited => "[exited]",
            };
            rows.push(format!("{marker}{} {tag}", session.name));
        }

        let new_idx = self.new_session_index();
        let new_marker = if self.selected == new_idx { "> " } else { "  " };
        rows.push(format!("{new_marker}New session (n)"));

        let shell_idx = new_idx + 1;
        let shell_marker = if self.selected == shell_idx {
            "> "
        } else {
            "  "
        };
        rows.push(format!("{shell_marker}Plain shell (s/Esc)"));
        rows
    }

    fn render_prompt(&self, max_w: usize, max_h: usize) -> Vec<String> {
        let mut lines = Vec::new();
        lines.push(truncate("New session name (blank = auto)", max_w));
        lines.push(truncate(&format!("> {}_", self.input), max_w));
        if let Some(err) = &self.validation_error {
            lines.push(truncate(err, max_w));
        }
        if lines.len() < max_h {
            lines.push(truncate("Enter: create | Esc: back", max_w));
        }
        lines.truncate(max_h);
        lines
    }
}

fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    if max_chars <= 1 {
        return text.chars().take(max_chars).collect();
    }
    let mut out: String = text.chars().take(max_chars - 1).collect();
    out.push('~');
    out
}

#[cfg(test)]
mod tests {
    use crate::name::SessionName;
    use crate::sessions::SessionState;

    use super::*;

    fn sample_sessions() -> Vec<Session> {
        vec![
            Session {
                name: SessionName::new("alpha").unwrap(),
                state: SessionState::Current,
            },
            Session {
                name: SessionName::new("beta").unwrap(),
                state: SessionState::Running,
            },
            Session {
                name: SessionName::new("work").unwrap(),
                state: SessionState::Exited,
            },
        ]
    }

    #[test]
    fn list_navigation_up_down_and_jk_with_clamping() {
        let mut picker = Picker::new(sample_sessions(), None);
        // 3 sessions + New session (3) + Plain shell (4)
        assert_eq!(picker.selected(), 0);
        assert_eq!(picker.handle(Key::Up), Step::Continue);
        assert_eq!(picker.selected(), 0);
        assert_eq!(picker.handle(Key::Char('k')), Step::Continue);
        assert_eq!(picker.selected(), 0);

        assert_eq!(picker.handle(Key::Down), Step::Continue);
        assert_eq!(picker.selected(), 1);
        assert_eq!(picker.handle(Key::Char('j')), Step::Continue);
        assert_eq!(picker.selected(), 2);
        assert_eq!(picker.handle(Key::Down), Step::Continue);
        assert_eq!(picker.selected(), 3);
        assert_eq!(picker.handle(Key::Char('j')), Step::Continue);
        assert_eq!(picker.selected(), 4);
        assert_eq!(picker.handle(Key::Down), Step::Continue);
        assert_eq!(picker.selected(), 4);

        assert_eq!(picker.handle(Key::Char('k')), Step::Continue);
        assert_eq!(picker.selected(), 3);
    }

    #[test]
    fn enter_on_running_and_exited_sessions_returns_attach() {
        let mut picker = Picker::new(sample_sessions(), None);
        assert_eq!(
            picker.handle(Key::Enter),
            Step::Done(Choice::Attach(SessionName::new("alpha").unwrap()))
        );

        let mut picker = Picker::new(sample_sessions(), None);
        picker.handle(Key::Down);
        picker.handle(Key::Down);
        assert_eq!(
            picker.handle(Key::Enter),
            Step::Done(Choice::Attach(SessionName::new("work").unwrap()))
        );
    }

    #[test]
    fn enter_on_plain_shell_or_s_or_esc_or_ctrl_c_picks_shell() {
        let mut p1 = Picker::new(sample_sessions(), None);
        for _ in 0..4 {
            p1.handle(Key::Down);
        }
        assert_eq!(p1.handle(Key::Enter), Step::Done(Choice::Shell));

        let mut p2 = Picker::new(sample_sessions(), None);
        assert_eq!(p2.handle(Key::Char('s')), Step::Done(Choice::Shell));

        let mut p3 = Picker::new(sample_sessions(), None);
        assert_eq!(p3.handle(Key::Esc), Step::Done(Choice::Shell));

        let mut p4 = Picker::new(sample_sessions(), None);
        assert_eq!(p4.handle(Key::CtrlC), Step::Done(Choice::Shell));
    }

    #[test]
    fn shortcut_n_and_enter_on_new_session_open_name_prompt() {
        let mut p1 = Picker::new(sample_sessions(), None);
        assert_eq!(p1.handle(Key::Char('n')), Step::Continue);
        assert_eq!(p1.mode(), Mode::NamePrompt);
        assert_eq!(p1.selected(), 3);

        let mut p2 = Picker::new(sample_sessions(), None);
        for _ in 0..3 {
            p2.handle(Key::Down);
        }
        assert_eq!(p2.handle(Key::Enter), Step::Continue);
        assert_eq!(p2.mode(), Mode::NamePrompt);
    }

    #[test]
    fn name_prompt_creates_unnamed_on_empty_enter() {
        let mut picker = Picker::new(sample_sessions(), None);
        picker.handle(Key::Char('n'));
        assert_eq!(picker.handle(Key::Enter), Step::Done(Choice::Create(None)));
    }

    #[test]
    fn name_prompt_typing_backspace_and_valid_enter_creates_named() {
        let mut picker = Picker::new(sample_sessions(), None);
        picker.handle(Key::Char('n'));
        for ch in ['d', 'e', 'v', 'x'] {
            assert_eq!(picker.handle(Key::Char(ch)), Step::Continue);
        }
        assert_eq!(picker.handle(Key::Backspace), Step::Continue);
        assert_eq!(picker.input(), "dev");
        assert_eq!(
            picker.handle(Key::Enter),
            Step::Done(Choice::Create(Some(SessionName::new("dev").unwrap())))
        );
    }

    #[test]
    fn name_prompt_rejects_invalid_name_stays_in_prompt_and_clears_on_edit() {
        let mut picker = Picker::new(sample_sessions(), None);
        picker.handle(Key::Char('n'));
        picker.handle(Key::Char('-'));
        picker.handle(Key::Char('b'));
        assert_eq!(picker.handle(Key::Enter), Step::Continue);
        assert_eq!(picker.mode(), Mode::NamePrompt);
        assert_eq!(
            picker.validation_error(),
            Some("Use letters, digits, dot, dash, underscore (max 64).")
        );

        // Editing clears validation error
        picker.handle(Key::Backspace);
        assert!(picker.validation_error().is_none());
        picker.handle(Key::Backspace);
        picker.handle(Key::Backspace); // safe on empty
        picker.handle(Key::Char('o'));
        picker.handle(Key::Char('k'));
        assert_eq!(
            picker.handle(Key::Enter),
            Step::Done(Choice::Create(Some(SessionName::new("ok").unwrap())))
        );
    }

    #[test]
    fn name_prompt_esc_cancels_to_list_and_ctrl_c_picks_shell() {
        let mut picker = Picker::new(sample_sessions(), None);
        picker.handle(Key::Char('n'));
        picker.handle(Key::Char('-'));
        picker.handle(Key::Enter);
        assert!(picker.validation_error().is_some());

        assert_eq!(picker.handle(Key::Esc), Step::Continue);
        assert_eq!(picker.mode(), Mode::List);
        assert!(picker.validation_error().is_none());

        picker.handle(Key::Char('n'));
        picker.handle(Key::Char('a'));
        assert_eq!(picker.handle(Key::CtrlC), Step::Done(Choice::Shell));
    }

    #[test]
    fn list_error_still_allows_new_session_and_plain_shell() {
        let mut picker = Picker::new(Vec::new(), Some("socket unavailable".to_owned()));
        let lines = picker.render_lines(80, 12);
        let joined = lines.join("\n");
        assert!(joined.contains("Could not list Zellij sessions: socket unavailable"));
        assert!(joined.contains("New session"));
        assert!(joined.contains("Plain shell"));

        assert_eq!(picker.handle(Key::Down), Step::Continue);
        assert_eq!(picker.handle(Key::Enter), Step::Done(Choice::Shell));
    }

    #[test]
    fn render_fits_20x5_with_text_only_markers_and_scrolls_selection() {
        let mut picker = Picker::new(sample_sessions(), None);
        for _ in 0..4 {
            picker.handle(Key::Down);
        }
        let lines = picker.render_lines(20, 5);
        assert!(!lines.is_empty());
        assert!(lines.len() <= 5, "lines exceeded height 5: {lines:?}");
        for line in &lines {
            assert!(
                line.chars().count() <= 20,
                "line exceeded width 20: {line:?}"
            );
            assert!(!line.contains('\x1b'), "render_lines must have no ANSI");
        }
        assert!(
            lines.iter().any(|l| l.starts_with("> ")),
            "selected row must be visible with '> ' marker: {lines:?}"
        );
    }

    #[test]
    fn render_shows_exited_and_current_markers_and_prompt_state() {
        let mut picker = Picker::new(sample_sessions(), None);
        let list_view = picker.render_lines(80, 15).join("\n");
        assert!(list_view.contains("alpha"));
        assert!(list_view.contains("[current]"));
        assert!(list_view.contains("beta"));
        assert!(list_view.contains("[running]"));
        assert!(list_view.contains("work"));
        assert!(list_view.contains("[exited]"));

        picker.handle(Key::Char('n'));
        picker.handle(Key::Char('!'));
        picker.handle(Key::Enter);
        let prompt_view = picker.render_lines(80, 15).join("\n");
        assert!(prompt_view.contains("New session"));
        assert!(prompt_view.contains("Use letters, digits, dot, dash, underscore (max 64)."));
    }
}
