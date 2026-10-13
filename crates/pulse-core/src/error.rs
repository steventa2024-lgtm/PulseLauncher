use serde::{ser::SerializeStruct, Serialize, Serializer};

pub type Result<T> = std::result::Result<T, CoreError>;

/// Structured error returned across the Tauri bridge as `{ code, message }`.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Security(String),
    #[error("{0}")]
    Unsupported(String),
    #[error("{0}")]
    Launch(String),
    #[error("operation cancelled")]
    Cancelled,
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::Invalid(_) => "invalid_input",
            CoreError::NotFound(_) => "not_found",
            CoreError::Security(_) => "security_policy",
            CoreError::Unsupported(_) => "unsupported",
            CoreError::Launch(_) => "launch_failed",
            CoreError::Cancelled => "cancelled",
            CoreError::Io(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                "permission_denied"
            }
            CoreError::Io(_) => "io_error",
            CoreError::Db(_) => "database_error",
            CoreError::Json(_) => "json_error",
        }
    }

    pub fn invalid(msg: impl Into<String>) -> Self {
        CoreError::Invalid(msg.into())
    }
}

impl Serialize for CoreError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut st = s.serialize_struct("CoreError", 2)?;
        st.serialize_field("code", self.code())?;
        st.serialize_field("message", &self.to_string())?;
        st.end()
    }
}
