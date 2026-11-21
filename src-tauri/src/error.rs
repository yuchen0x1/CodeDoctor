#[cfg(test)]
mod tests {
    use crate::error::{AppError, scan_failure_message};
    use std::io::ErrorKind;

    #[test]
    fn io_errors_do_not_expose_file_paths() {
        let io_error = std::io::Error::new(
            ErrorKind::PermissionDenied,
            "denied reading C:\\private\\project\\secret.txt",
        );
        let app_error = AppError::from(io_error);

        assert_eq!(app_error.to_string(), "filesystem operation was denied");
        assert!(!app_error.to_string().to_lowercase().contains("secret"));
    }

    #[test]
    fn database_errors_do_not_expose_database_details() {
        let sqlite_error =
            rusqlite::Error::InvalidParameterName("hidden database value".to_owned());
        let app_error = AppError::from(sqlite_error);

        assert_eq!(app_error.to_string(), "local storage operation failed");
        assert!(!app_error.to_string().contains("hidden"));
    }

    #[test]
    fn persistent_failure_text_is_single_line_bounded_and_unicode_safe() {
        let message =
            scan_failure_message(&format!("\0\t失败\r\n\u{0085}\u{2028}{}", "错".repeat(600)));
        assert!(!message.chars().any(char::is_control));
        assert!(!message.contains(['\n', '\r', '\u{2028}']));
        assert!(message.chars().count() <= 512);
        assert!(message.starts_with("失败"));
        assert_eq!(scan_failure_message(&"错".repeat(600)).chars().count(), 512);
        assert_eq!(scan_failure_message("\0\t\r\n"), "scan failed");
    }
}
use serde::Serialize;
use std::fmt;

#[derive(Debug, Serialize)]
pub enum AppError {
    Io(String),
    Database(String),
    Zip(String),
    Parser(String),
    Process(String),
    Validation(String),
}

impl fmt::Display for AppError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(message) => write!(formatter, "{message}"),
            Self::Database(message) => write!(formatter, "{message}"),
            Self::Zip(message) => write!(formatter, "{message}"),
            Self::Parser(message) => write!(formatter, "{message}"),
            Self::Process(message) => write!(formatter, "{message}"),
            Self::Validation(message) => write!(formatter, "{message}"),
        }
    }
}

impl std::error::Error for AppError {}

/// Bound already-safe AppError display text for persistent scan diagnostics.
/// This is not a redactor: callers must never pass raw OS/tool/database errors.
/// The limit counts Unicode scalar values, not UTF-8 bytes.
pub(crate) fn scan_failure_message(message: &str) -> String {
    let message: String = message
        .chars()
        .map(|character| {
            if character.is_control() || character.is_whitespace() {
                ' '
            } else {
                character
            }
        })
        .take(512)
        .collect();
    let message = message.trim();
    if message.is_empty() {
        "scan failed".to_owned()
    } else {
        message.to_owned()
    }
}

impl From<std::io::Error> for AppError {
    fn from(error: std::io::Error) -> Self {
        let message = match error.kind() {
            std::io::ErrorKind::NotFound => "filesystem item was not found".to_owned(),
            std::io::ErrorKind::PermissionDenied => "filesystem operation was denied".to_owned(),
            _ => "filesystem operation failed".to_owned(),
        };
        Self::Io(message)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(_: rusqlite::Error) -> Self {
        Self::Database("local storage operation failed".to_owned())
    }
}
