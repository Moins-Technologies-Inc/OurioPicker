//! OurioPicker (`ouro-terminal-picker`): in-terminal Zellij session picker
//! and terminal configuration helper for Kitty and Rio/Ourio.

pub mod launch;
pub mod name;
pub mod picker;
pub mod sessions;

pub use launch::{
    Choice, FALLBACK_SHELL, LaunchError, exec_shell, find_in_path, is_executable_file,
    parse_passwd_shell, resolve_shell, resolve_shell_with, run_choice, should_bypass_picker,
    zellij_command, zellij_command_with,
};
pub use name::{NameError, SessionName};
pub use picker::{Key, Mode, Picker, Step};
pub use sessions::{
    Session, SessionListError, SessionState, list_sessions, list_sessions_with, parse_sessions,
    strip_ansi,
};
