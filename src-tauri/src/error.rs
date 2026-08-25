use serde::{Serialize, Serializer};

/// Every command failure the frontend can see. Serialised as a plain string so
/// `invoke()` rejects with a readable message instead of an opaque object.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),

    #[error("{path}: {source}")]
    Io {
        path: String,
        #[source]
        source: std::io::Error,
    },

    #[error("{path} could not be parsed: {source}")]
    Script {
        path: String,
        #[source]
        source: crate::paradox::ParseError,
    },

    #[error("settings could not be read: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{path} is not a supported image: {message}")]
    Image { path: String, message: String },
}

impl AppError {
    pub fn message(text: impl Into<String>) -> Self {
        Self::Message(text.into())
    }

    pub fn io(path: impl AsRef<std::path::Path>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.as_ref().display().to_string(),
            source,
        }
    }
}

impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;
