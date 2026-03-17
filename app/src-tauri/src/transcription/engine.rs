use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use serde::Serialize;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{
    error::{AppError, AppResult},
    settings::DefaultModelPreset,
    state::ModelStatus,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TranscriptPayload {
    pub text: String,
    pub engine: String,
}

pub(crate) struct LoadedWhisperModel {
    model_path: PathBuf,
    context: WhisperContext,
}

struct WhisperRuntime {
    model: Option<LoadedWhisperModel>,
    status: ModelStatus,
    message: Option<String>,
}

pub struct WhisperManager {
    inner: Mutex<WhisperRuntime>,
}

impl Default for WhisperManager {
    fn default() -> Self {
        Self {
            inner: Mutex::new(WhisperRuntime {
                model: None,
                status: ModelStatus::Missing,
                message: Some(
                    "TalkieTxt checks its managed models folder automatically. Choose a local GGML Whisper model only if needed."
                        .to_string(),
                ),
            }),
        }
    }
}

impl WhisperManager {
    pub fn begin_reload(
        &self,
        model_path: Option<PathBuf>,
        preset: DefaultModelPreset,
    ) -> Option<PathBuf> {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        guard.model = None;
        match model_path {
            Some(path) => {
                guard.status = ModelStatus::Loading;
                guard.message = Some(format!(
                    "Loading {} for low-latency dictation...",
                    path.display()
                ));
                Some(path)
            }
            None => {
                guard.status = ModelStatus::Missing;
                guard.message = Some(format!(
                    "No model found automatically. Choose {} to enable local dictation",
                    crate::transcription::model::recommended_model_filename(preset)
                ));
                None
            }
        }
    }

    pub fn finish_reload(&self, result: AppResult<LoadedWhisperModel>) {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);

        match result {
            Ok(model) => {
                guard.message = Some(format!("Model ready: {}", model.model_path.display()));
                guard.status = ModelStatus::Ready;
                guard.model = Some(model);
            }
            Err(error) => {
                guard.status = ModelStatus::Failed;
                guard.message = Some(error.to_string());
                guard.model = None;
            }
        }
    }

    pub fn snapshot(&self) -> (ModelStatus, Option<String>) {
        let guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (guard.status, guard.message.clone())
    }

    pub fn ensure_ready(&self) -> AppResult<()> {
        let guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match (&guard.status, &guard.message) {
            (ModelStatus::Ready, _) => Ok(()),
            (ModelStatus::Missing, _) => Err(AppError::ModelNotConfigured),
            (_, Some(message)) => Err(AppError::ModelNotReady(message.clone())),
            _ => Err(AppError::ModelNotReady("Model is not ready".to_string())),
        }
    }

    pub fn transcribe(&self, audio: &[f32]) -> AppResult<TranscriptPayload> {
        let guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let loaded = guard.model.as_ref().ok_or_else(|| {
            let message = guard
                .message
                .clone()
                .unwrap_or_else(|| "Model is not ready".to_string());
            AppError::ModelNotReady(message)
        })?;

        let mut state = loaded.context.create_state().map_err(|error| {
            AppError::Transcription(format!("Failed to create decode state: {error}"))
        })?;

        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 0 });
        params.set_n_threads(preferred_decode_threads() as i32);
        params.set_language(Some("en"));
        params.set_translate(false);
        params.set_no_context(true);
        params.set_no_timestamps(true);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_special(false);
        params.set_print_timestamps(false);

        state.full(params, audio).map_err(|error| {
            AppError::Transcription(format!("whisper-rs decode failed: {error}"))
        })?;

        let raw_text = state
            .as_iter()
            .map(|segment| segment.to_string())
            .collect::<Vec<_>>()
            .join(" ");

        Ok(TranscriptPayload {
            text: cleanup_transcript(&raw_text),
            engine: "whisper-rs".to_string(),
        })
    }
}

pub(crate) fn load_model(model_path: &Path) -> AppResult<LoadedWhisperModel> {
    let context = WhisperContext::new_with_params(
        model_path.to_string_lossy().as_ref(),
        WhisperContextParameters::default(),
    )
    .map_err(|error| AppError::ModelLoad(error.to_string()))?;

    Ok(LoadedWhisperModel {
        model_path: model_path.to_path_buf(),
        context,
    })
}

pub fn cleanup_transcript(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim()
        .to_string()
}

fn preferred_decode_threads() -> usize {
    std::thread::available_parallelism()
        .map(|value| value.get().min(6))
        .unwrap_or(4)
        .max(1)
}

#[cfg(test)]
mod tests {
    use super::cleanup_transcript;

    #[test]
    fn cleanup_normalizes_whitespace() {
        assert_eq!(
            cleanup_transcript("  hello   there \n world "),
            "hello there world"
        );
    }
}
