#[cfg(test)]
mod tests {
    use crate::error::AppError;
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
