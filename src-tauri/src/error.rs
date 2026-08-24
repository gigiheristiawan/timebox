use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// A setting the app refuses to store rather than silently rewrite. The
    /// message is the one the user sees.
    #[error("{0}")]
    Rejected(String),
}

/// Commands return a plain string to the webview. The webview never sees the
/// internal error shape, only a message it can display.
impl Serialize for AppError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
