use serde::Serialize;

/// Structured error returned across the Tauri bridge. `code` is stable and
/// machine-readable; `message` is human-friendly and safe to display.
#[derive(Debug, thiserror::Error, Serialize, Clone, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
#[error("{code}: {message}")]
pub struct AppError {
    pub code: ErrorCode,
    pub message: String,
}

#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    InvalidInput,
    TargetMissing,
    AccessDenied,
    Unsupported,
    PolicyBlocked,
    LaunchFailed,
    Storage,
    Io,
    Conflict,
    Cancelled,
    Timeout,
    Integration,
    Internal,
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = serde_json::to_value(self)
            .ok()
            .and_then(|v| v.as_str().map(str::to_owned))
            .unwrap_or_else(|| "internal".into());
        f.write_str(&s)
    }
}

impl AppError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
    pub fn not_found(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, m)
    }
    pub fn invalid(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::InvalidInput, m)
    }
    pub fn policy(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::PolicyBlocked, m)
    }
    pub fn unsupported(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Unsupported, m)
    }
    pub fn internal(m: impl Into<String>) -> Self {
        Self::new(ErrorCode::Internal, m)
    }
}

impl From<rusqlite::Error> for AppError {
    fn from(e: rusqlite::Error) -> Self {
        match e {
            rusqlite::Error::QueryReturnedNoRows => Self::not_found("Record not found"),
            other => Self::new(ErrorCode::Storage, other.to_string()),
        }
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        let code = match e.kind() {
            std::io::ErrorKind::NotFound => ErrorCode::TargetMissing,
            std::io::ErrorKind::PermissionDenied => ErrorCode::AccessDenied,
            std::io::ErrorKind::AlreadyExists => ErrorCode::Conflict,
            _ => ErrorCode::Io,
        };
        Self::new(code, e.to_string())
    }
}

impl From<serde_json::Error> for AppError {
    fn from(e: serde_json::Error) -> Self {
        Self::invalid(format!("Invalid JSON: {e}"))
    }
}

pub type AppResult<T> = Result<T, AppError>;
