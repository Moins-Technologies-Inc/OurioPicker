//! OurioPicker (`ouro-terminal-picker`): in-terminal Zellij session picker
//! and terminal configuration helper for Kitty and Rio/Ourio.

pub mod config;
pub mod launch;
pub mod name;
pub mod picker;
pub mod sessions;
pub mod term;

pub use config::{ConfigError, ConfigPlan, FileOp, TerminalKind, apply_plan};
pub use launch::{
    Choice, FALLBACK_SHELL, LaunchError, exec_shell, find_in_path, is_executable_file,
    parse_passwd_shell, resolve_shell, resolve_shell_with, resolve_shell_with_exe, run_choice,
    should_bypass_picker, zellij_command, zellij_command_with,
};
pub use name::{MAX_SESSION_NAME_LEN, NameError, SessionName};
pub use picker::{Key, Mode, Picker, Step};
pub use sessions::{
    Session, SessionListError, SessionState, list_sessions, list_sessions_with, parse_sessions,
    strip_ansi,
};
pub use term::{
    TerminalError, TerminalGuard, install_panic_hook, is_interactive_tty, map_key_event,
    no_color_enabled, restore_terminal, run_tui,
};
