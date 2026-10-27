#![doc = include_str!("../README.md")]

pub mod audio;
pub mod cache;
pub mod cost;
pub mod markdown;
pub mod request;
pub mod service;
pub mod terms;
pub mod transcript;

pub use request::RequestOptions;
pub use transcript::Response;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{0}")]
    Invalid(String),
    #[error("{operation}: {source}")]
    Io {
        operation: String,
        #[source]
        source: std::io::Error,
    },
    #[error("decoding transcription data: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Deepgram request failed: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Deepgram returned HTTP {status}: {body}")]
    Api { status: u16, body: String },
    #[error("operation cancelled")]
    Cancelled,
    #[error("audio preprocessing: {0}")]
    Audio(#[source] cli_deps::Error),
    #[error("Deepgram request exceeded its deadline")]
    Deadline,
    #[error("no cached transcription or job matched {0}")]
    Missing(String),
}

pub(crate) fn io(operation: impl Into<String>, source: std::io::Error) -> Error {
    Error::Io {
        operation: operation.into(),
        source,
    }
}
