use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{message}")]
    Message { code: &'static str, message: String },
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

impl AppError {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self::Message {
            code,
            message: message.into(),
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let (code, message) = match self {
            Self::Message { code, message } => (*code, message.clone()),
            other => ("INTERNAL", other.to_string()),
        };
        let mut state = serializer.serialize_struct("AppError", 2)?;
        state.serialize_field("code", code)?;
        state.serialize_field("message", &message)?;
        state.end()
    }
}

pub type AppResult<T> = Result<T, AppError>;
