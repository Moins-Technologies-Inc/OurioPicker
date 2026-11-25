//! Validated Zellij session names.

use std::fmt;

use thiserror::Error;

/// Maximum permitted length of a Zellij session name in bytes/characters.
pub const MAX_SESSION_NAME_LEN: usize = 64;

/// Validation error returned by [`SessionName::new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NameError {
    /// Input was empty.
    #[error("Use letters, digits, dot, dash, underscore (max 64).")]
    Empty,
    /// Input exceeded [`MAX_SESSION_NAME_LEN`] characters.
    #[error("Use letters, digits, dot, dash, underscore (max 64).")]
    TooLong {
        /// Actual length of the rejected name.
        len: usize,
        /// Maximum permitted length.
        max: usize,
    },
    /// Input started with a disallowed leading character (`-` or `.`).
    #[error("Use letters, digits, dot, dash, underscore (max 64).")]
    InvalidStart(char),
    /// Input contained a character outside `[A-Za-z0-9._-]`.
    #[error("Use letters, digits, dot, dash, underscore (max 64).")]
    InvalidChar(char),
}

/// A validated Zellij session name (`[A-Za-z0-9._-]`, length 1..=64, not starting with `-` or `.`).
///
/// # Examples
///
/// ```
/// use ouro_terminal_picker::SessionName;
///
/// let name = SessionName::new("work-1_main.dev")?;
/// assert_eq!(name.as_str(), "work-1_main.dev");
/// # Ok::<(), ouro_terminal_picker::NameError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SessionName(String);

impl SessionName {
    /// Validate and construct a new [`SessionName`].
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::{NameError, SessionName};
    ///
    /// assert!(SessionName::new("dev").is_ok());
    /// assert!(matches!(SessionName::new("-bad"), Err(NameError::InvalidStart('-'))));
    /// ```
    pub fn new(raw: &str) -> Result<Self, NameError> {
        let mut chars = raw.chars();
        let Some(first) = chars.next() else {
            return Err(NameError::Empty);
        };
        if matches!(first, '-' | '.') {
            return Err(NameError::InvalidStart(first));
        }
        let mut count = 0usize;
        for ch in raw.chars() {
            if !matches!(ch, 'A'..='Z' | 'a'..='z' | '0'..='9' | '.' | '_' | '-') {
                return Err(NameError::InvalidChar(ch));
            }
            count += 1;
        }
        if count > MAX_SESSION_NAME_LEN {
            return Err(NameError::TooLong {
                len: count,
                max: MAX_SESSION_NAME_LEN,
            });
        }
        Ok(Self(raw.to_owned()))
    }

    /// Return the session name as a string slice.
    ///
    /// # Examples
    ///
    /// ```
    /// use ouro_terminal_picker::SessionName;
    ///
    /// let name = SessionName::new("alpha")?;
    /// assert_eq!(name.as_str(), "alpha");
    /// # Ok::<(), ouro_terminal_picker::NameError>(())
    /// ```
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for SessionName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for SessionName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_valid_alphanumeric_and_punctuation_names() {
        for valid in [
            "a",
            "work",
            "d580883b86d4",
            "Work_01-beta.2",
            "_leading_underscore",
            "0start-digit",
        ] {
            let parsed = SessionName::new(valid).expect("valid name must parse");
            assert_eq!(parsed.as_str(), valid);
            assert_eq!(parsed.to_string(), valid);
        }
    }

    #[test]
    fn accepts_exact_max_length_64() {
        let max_name = "a".repeat(MAX_SESSION_NAME_LEN);
        let parsed = SessionName::new(&max_name).expect("64-char name must be accepted");
        assert_eq!(parsed.as_str().len(), 64);
    }

    #[test]
    fn rejects_empty_name() {
        assert_eq!(SessionName::new(""), Err(NameError::Empty));
    }

    #[test]
    fn rejects_name_longer_than_64() {
        let long_name = "a".repeat(MAX_SESSION_NAME_LEN + 1);
        assert_eq!(
            SessionName::new(&long_name),
            Err(NameError::TooLong { len: 65, max: 64 })
        );
    }

    #[test]
    fn rejects_leading_dash_or_dot() {
        assert_eq!(SessionName::new("-opt"), Err(NameError::InvalidStart('-')));
        assert_eq!(
            SessionName::new("--create"),
            Err(NameError::InvalidStart('-'))
        );
        assert_eq!(
            SessionName::new(".hidden"),
            Err(NameError::InvalidStart('.'))
        );
        assert_eq!(
            SessionName::new("..parent"),
            Err(NameError::InvalidStart('.'))
        );
    }

    #[test]
    fn rejects_spaces_slashes_control_and_non_ascii() {
        for (bad, expected) in [
            ("my session", ' '),
            ("a/b", '/'),
            ("a\\b", '\\'),
            ("a;rm", ';'),
            ("a$HOME", '$'),
            ("a\nb", '\n'),
            ("café", 'é'),
            ("a:b", ':'),
        ] {
            assert_eq!(
                SessionName::new(bad),
                Err(NameError::InvalidChar(expected)),
                "input {bad:?} must fail with InvalidChar({expected:?})"
            );
        }
    }

    #[test]
    fn error_message_matches_user_facing_plan_string() {
        let expected = "Use letters, digits, dot, dash, underscore (max 64).";
        assert_eq!(NameError::Empty.to_string(), expected);
        assert_eq!(
            NameError::TooLong { len: 65, max: 64 }.to_string(),
            expected
        );
        assert_eq!(NameError::InvalidStart('-').to_string(), expected);
        assert_eq!(NameError::InvalidChar(' ').to_string(), expected);
    }
}
