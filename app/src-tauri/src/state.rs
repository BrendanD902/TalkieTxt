use std::sync::Mutex;

use serde::Serialize;
use tauri_plugin_global_shortcut::Shortcut;

use crate::{
    audio::{CapturedAudio, RecordingSession},
    error::{AppError, AppResult},
    settings::{AppSettings, DefaultModelPreset, HotkeyMode},
};

pub const DEFAULT_SHORTCUT_LABEL: &str = "Option+Space";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DictationState {
    Idle,
    Listening,
    Processing,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ModelStatus {
    Loading,
    Ready,
    Missing,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatusPayload {
    pub state: DictationState,
    pub source: String,
    pub model_status: ModelStatus,
    pub model_message: Option<String>,
    pub hotkey_hint: String,
    pub hotkey_mode: HotkeyMode,
    pub model_path: Option<String>,
    pub default_model_preset: DefaultModelPreset,
    pub paste_after_transcribe: bool,
    pub preferred_input_device: Option<String>,
    pub current_input_device: Option<String>,
    pub last_capture_sample_rate: Option<u32>,
    pub last_capture_channels: Option<u16>,
}

impl Default for StatusPayload {
    fn default() -> Self {
        Self {
            state: DictationState::Idle,
            source: "bootstrap".to_string(),
            model_status: ModelStatus::Missing,
            model_message: Some(
                "TalkieTxt checks its managed models folder automatically. Choose a local GGML model only if needed."
                    .to_string(),
            ),
            hotkey_hint: DEFAULT_SHORTCUT_LABEL.to_string(),
            hotkey_mode: HotkeyMode::HoldToTalk,
            model_path: None,
            default_model_preset: DefaultModelPreset::TinyEn,
            paste_after_transcribe: true,
            preferred_input_device: None,
            current_input_device: None,
            last_capture_sample_rate: None,
            last_capture_channels: None,
        }
    }
}

pub struct SettingsState {
    inner: Mutex<AppSettings>,
}

impl SettingsState {
    pub fn new(settings: AppSettings) -> Self {
        Self {
            inner: Mutex::new(settings),
        }
    }

    pub fn get(&self) -> AppSettings {
        self.inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn replace(&self, settings: AppSettings) {
        let mut guard = self
            .inner
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *guard = settings;
    }
}

pub struct StatusManager {
    shortcut: Mutex<Shortcut>,
    snapshot: Mutex<StatusPayload>,
}

impl StatusManager {
    pub fn new(shortcut: Shortcut) -> Self {
        Self {
            shortcut: Mutex::new(shortcut),
            snapshot: Mutex::new(StatusPayload::default()),
        }
    }

    pub fn shortcut(&self) -> Shortcut {
        self.shortcut
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn snapshot(&self) -> StatusPayload {
        self.snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn apply_settings(&self, settings: &AppSettings) {
        self.update(|snapshot| {
            snapshot.hotkey_mode = settings.hotkey_mode;
            snapshot.model_path = settings.model_path.clone();
            snapshot.default_model_preset = settings.default_model_preset;
            snapshot.paste_after_transcribe = settings.paste_after_transcribe;
            snapshot.preferred_input_device = settings.preferred_input_device.clone();
        });
    }

    pub fn update_state(&self, state: DictationState, source: impl Into<String>) {
        self.update(|snapshot| {
            snapshot.state = state;
            snapshot.source = source.into();
        });
    }

    pub fn update_model_status(&self, status: ModelStatus, message: Option<String>) {
        self.update(|snapshot| {
            snapshot.model_status = status;
            snapshot.model_message = message;
        });
    }

    pub fn update_capture_details(&self, audio: &CapturedAudio) {
        self.update(|snapshot| {
            snapshot.current_input_device = Some(audio.device_name.clone());
            snapshot.last_capture_sample_rate = Some(audio.sample_rate);
            snapshot.last_capture_channels = Some(audio.channels);
        });
    }

    pub fn update_start_details(&self, device_name: String, sample_rate: u32, channels: u16) {
        self.update(|snapshot| {
            snapshot.current_input_device = Some(device_name);
            snapshot.last_capture_sample_rate = Some(sample_rate);
            snapshot.last_capture_channels = Some(channels);
        });
    }

    pub fn set_error_state(&self, source: impl Into<String>) {
        self.update_state(DictationState::Error, source);
    }

    fn update<F>(&self, mutate: F)
    where
        F: FnOnce(&mut StatusPayload),
    {
        let mut guard = self
            .snapshot
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        mutate(&mut guard);
    }
}

pub struct RecordingManager {
    session: Mutex<Option<RecordingSession>>,
}

impl Default for RecordingManager {
    fn default() -> Self {
        Self {
            session: Mutex::new(None),
        }
    }
}

impl RecordingManager {
    pub fn start(&self, session: RecordingSession) -> AppResult<()> {
        let mut guard = self
            .session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if guard.is_some() {
            return Err(AppError::InvalidState(
                "Recording is already active".to_string(),
            ));
        }
        *guard = Some(session);
        Ok(())
    }

    pub fn stop(&self) -> AppResult<CapturedAudio> {
        let session = self
            .session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take()
            .ok_or_else(|| AppError::InvalidState("Recording is not active".to_string()))?;

        session.stop()
    }

    pub fn reset(&self) -> AppResult<()> {
        let maybe_session = self
            .session
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();

        if let Some(session) = maybe_session {
            let _ = session.stop();
        }

        Ok(())
    }
}
