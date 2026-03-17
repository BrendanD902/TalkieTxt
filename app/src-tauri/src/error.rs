use std::path::PathBuf;

use thiserror::Error;

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("{0}")]
    Message(String),
    #[error("No microphone input device is available")]
    NoInputDevice,
    #[error("Unsupported microphone sample format: {0}")]
    UnsupportedSampleFormat(String),
    #[error("Failed to start microphone capture: {0}")]
    AudioStream(String),
    #[error("Model is not configured. Select a local GGML Whisper model file.")]
    ModelNotConfigured,
    #[error("Model file is missing: {0}")]
    ModelMissing(PathBuf),
    #[error("Model is not ready: {0}")]
    ModelNotReady(String),
    #[error("Failed to load model: {0}")]
    ModelLoad(String),
    #[error("Transcription failed: {0}")]
    Transcription(String),
    #[error("Clipboard insertion failed: {0}")]
    Clipboard(String),
    #[error("Settings error: {0}")]
    Settings(String),
    #[error("Shortcut registration failed: {0}")]
    Shortcut(String),
    #[error("{0}")]
    InvalidState(String),
}

impl From<anyhow::Error> for AppError {
    fn from(value: anyhow::Error) -> Self {
        Self::Message(value.to_string())
    }
}
