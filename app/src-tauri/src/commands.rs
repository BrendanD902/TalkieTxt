use std::process::Command;

use tauri::{AppHandle, Emitter, Manager, State};
use tracing::{error, info};

use crate::{
    audio::{self, prepare_for_whisper_audio},
    clipboard,
    error::{AppError, AppResult},
    settings::{self, AppSettings},
    state::{
        DictationState, ModelStatus, RecordingManager, SettingsState, StatusManager, StatusPayload,
    },
    transcription::{self, TranscriptPayload, WhisperManager},
};

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorPayload {
    message: String,
    source: String,
}

pub fn emit_status(app_handle: &AppHandle) {
    let status = app_handle.state::<StatusManager>().snapshot();
    let _ = app_handle.emit("dictation://status", status);
}

fn emit_error(app_handle: &AppHandle, source: &str, error: impl ToString) {
    let message = error.to_string();
    error!(source, "{message}");
    let _ = app_handle.emit(
        "dictation://error",
        ErrorPayload {
            message: message.clone(),
            source: source.to_string(),
        },
    );
    app_handle
        .state::<StatusManager>()
        .set_error_state(source.to_string());
    emit_status(app_handle);
}

fn emit_transcript(app_handle: &AppHandle, transcript: &TranscriptPayload) {
    let _ = app_handle.emit("dictation://transcript", transcript.clone());
}

fn emit_insert_result(app_handle: &AppHandle, result: clipboard::InsertResult) {
    let _ = app_handle.emit("dictation://insert-result", result);
}

fn empty_transcript_message(
    hotkey_mode: crate::settings::HotkeyMode,
    levels: audio::AudioLevelStats,
) -> String {
    if levels.is_effectively_silent() {
        return "No audio was captured. Check microphone permissions and input level.".to_string();
    }

    if levels.is_low_input() {
        return format!(
            "Audio was captured, but it was very quiet (peak {:.0}%, rms {:.1}%). Check your mic input level or try another input device.",
            levels.peak_abs * 100.0,
            levels.rms * 100.0,
        );
    }

    match hotkey_mode {
        crate::settings::HotkeyMode::HoldToTalk => {
            "No speech detected. Hold Option+Space while speaking, then release to transcribe."
                .to_string()
        }
        crate::settings::HotkeyMode::Toggle => {
            "No speech detected. Press Option+Space once to start, speak, then press it again to transcribe."
                .to_string()
        }
    }
}

pub fn reload_model_async(app_handle: AppHandle) {
    let settings = app_handle.state::<SettingsState>().get();
    let whisper_manager = app_handle.state::<WhisperManager>();
    let model_path = transcription::model::resolve_model_path(&app_handle, &settings);

    match model_path {
        Ok(model_path) => {
            let maybe_path =
                whisper_manager.begin_reload(model_path, settings.default_model_preset);
            sync_model_status(&app_handle);

            if let Some(model_path) = maybe_path {
                let load_handle = app_handle.clone();
                tauri::async_runtime::spawn(async move {
                    let result = tauri::async_runtime::spawn_blocking(move || {
                        transcription::engine::load_model(&model_path)
                    })
                    .await
                    .map_err(|error| AppError::ModelLoad(error.to_string()))
                    .and_then(|result| result);

                    load_handle.state::<WhisperManager>().finish_reload(result);
                    sync_model_status(&load_handle);
                    let (status, message) = load_handle.state::<WhisperManager>().snapshot();
                    if matches!(status, ModelStatus::Failed) {
                        if let Some(message) = message {
                            emit_error(&load_handle, "model", message);
                        }
                    }
                });
            }
        }
        Err(error) => {
            whisper_manager.begin_reload(None, settings.default_model_preset);
            whisper_manager.finish_reload(Err(error));
            sync_model_status(&app_handle);
        }
    }
}

fn sync_model_status(app_handle: &AppHandle) {
    let (model_status, model_message) = app_handle.state::<WhisperManager>().snapshot();
    app_handle
        .state::<StatusManager>()
        .update_model_status(model_status, model_message);
    emit_status(app_handle);
}

fn apply_settings_snapshot(app_handle: &AppHandle, settings: &AppSettings) {
    app_handle.state::<StatusManager>().apply_settings(settings);
    emit_status(app_handle);
}

pub fn handle_hotkey_event(
    app_handle: &AppHandle,
    shortcut: &tauri_plugin_global_shortcut::Shortcut,
    event: tauri_plugin_global_shortcut::ShortcutEvent,
) {
    let status = app_handle.state::<StatusManager>().snapshot();
    let active_shortcut = app_handle.state::<StatusManager>().shortcut();
    info!(
        event = ?event.state(),
        dictation_state = ?status.state,
        hotkey_mode = ?status.hotkey_mode,
        "Received hotkey event"
    );

    if event.state() == tauri_plugin_global_shortcut::ShortcutState::Pressed
        && shortcut != &active_shortcut
    {
        return;
    }

    let result = match status.hotkey_mode {
        crate::settings::HotkeyMode::Toggle => {
            if event.state() != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                return;
            }
            toggle_recording_internal(app_handle)
        }
        crate::settings::HotkeyMode::HoldToTalk => match event.state() {
            tauri_plugin_global_shortcut::ShortcutState::Pressed => {
                if matches!(status.state, DictationState::Idle | DictationState::Error) {
                    start_recording_internal(app_handle, "hotkey")
                } else {
                    Ok(())
                }
            }
            tauri_plugin_global_shortcut::ShortcutState::Released => {
                if status.state == DictationState::Listening {
                    stop_recording_internal(app_handle, "hotkey")
                } else {
                    Ok(())
                }
            }
        },
    };

    if let Err(error) = result {
        emit_error(app_handle, "hotkey", error);
    }
}

fn start_recording_internal(app_handle: &AppHandle, source: &str) -> AppResult<()> {
    let status = app_handle.state::<StatusManager>().snapshot();
    if matches!(
        status.state,
        DictationState::Listening | DictationState::Processing
    ) {
        return Err(AppError::InvalidState(format!(
            "Cannot start recording while app is {:?}",
            status.state
        )));
    }

    app_handle.state::<WhisperManager>().ensure_ready()?;
    let settings = app_handle.state::<SettingsState>().get();
    let paste_target_bundle_id = clipboard::frontmost_app_bundle_id();
    let session = audio::capture::start_recording(
        settings.preferred_input_device.as_deref(),
        paste_target_bundle_id.clone(),
    )?;
    let start_info = session.start_info().clone();
    info!(
        source,
        device = %start_info.device_name,
        sample_rate = start_info.sample_rate,
        channels = start_info.channels,
        paste_target = ?paste_target_bundle_id,
        "Recording started"
    );
    app_handle.state::<RecordingManager>().start(session)?;
    app_handle.state::<StatusManager>().update_start_details(
        start_info.device_name,
        start_info.sample_rate,
        start_info.channels,
    );
    app_handle
        .state::<StatusManager>()
        .update_state(DictationState::Listening, source.to_string());
    emit_status(app_handle);
    Ok(())
}

fn stop_recording_internal(app_handle: &AppHandle, source: &str) -> AppResult<()> {
    let status = app_handle.state::<StatusManager>().snapshot();
    if status.state != DictationState::Listening {
        return Err(AppError::InvalidState(format!(
            "Cannot stop recording while app is {:?}",
            status.state
        )));
    }

    let captured = app_handle.state::<RecordingManager>().stop()?;
    info!(
        source,
        device = %captured.device_name,
        sample_rate = captured.sample_rate,
        channels = captured.channels,
        samples = captured.samples.len(),
        "Recording stopped"
    );
    app_handle
        .state::<StatusManager>()
        .update_capture_details(&captured);
    app_handle
        .state::<StatusManager>()
        .update_state(DictationState::Processing, source.to_string());
    emit_status(app_handle);
    run_transcription_job(app_handle.clone(), captured);
    Ok(())
}

fn toggle_recording_internal(app_handle: &AppHandle) -> AppResult<()> {
    match app_handle.state::<StatusManager>().snapshot().state {
        DictationState::Idle | DictationState::Error => {
            start_recording_internal(app_handle, "command")
        }
        DictationState::Listening => stop_recording_internal(app_handle, "command"),
        DictationState::Processing => Err(AppError::InvalidState(
            "Transcription is still in progress".to_string(),
        )),
    }
}

fn run_transcription_job(app_handle: AppHandle, captured: audio::CapturedAudio) {
    tauri::async_runtime::spawn(async move {
        let settings = app_handle.state::<SettingsState>().get();
        let prepared = match prepare_for_whisper_audio(&captured) {
            Ok(prepared) => prepared,
            Err(error) => {
                emit_error(&app_handle, "audio", error);
                return;
            }
        };
        info!(
            input_samples = captured.samples.len(),
            prepared_samples = prepared.samples.len(),
            input_peak = prepared.stats_before_normalization.peak_abs,
            input_rms = prepared.stats_before_normalization.rms,
            normalized_peak = prepared.stats_after_normalization.peak_abs,
            normalized_rms = prepared.stats_after_normalization.rms,
            applied_gain = prepared.applied_gain,
            "Prepared audio for transcription"
        );

        if settings.save_debug_wav {
            match audio::write_debug_wav(&prepared.samples, 16_000, "dictation-debug") {
                Ok(path) => info!("Saved debug WAV to {}", path.display()),
                Err(error) => error!("Failed to save debug WAV: {error}"),
            }
        }

        let prepared_levels = prepared.stats_before_normalization;
        let prepared_samples = prepared.samples;
        let decode_handle = app_handle.clone();
        let transcript_result = tauri::async_runtime::spawn_blocking(move || {
            decode_handle
                .state::<WhisperManager>()
                .transcribe(&prepared_samples)
        })
        .await
        .map_err(|error| AppError::Transcription(error.to_string()))
        .and_then(|result| result);

        match transcript_result {
            Ok(transcript) => {
                info!(
                    transcript_chars = transcript.text.len(),
                    transcript_words = transcript.text.split_whitespace().count(),
                    "Transcription finished"
                );
                emit_transcript(&app_handle, &transcript);

                if transcript.text.trim().is_empty() {
                    let message = empty_transcript_message(settings.hotkey_mode, prepared_levels);

                    emit_insert_result(
                        &app_handle,
                        clipboard::InsertResult {
                            status: clipboard::InsertStatus::Skipped,
                            message,
                        },
                    );
                    app_handle
                        .state::<StatusManager>()
                        .update_state(DictationState::Idle, "transcription".to_string());
                    emit_status(&app_handle);
                    return;
                }

                match clipboard::insert_text(
                    &transcript.text,
                    settings.paste_after_transcribe,
                    captured.paste_target_bundle_id.as_deref(),
                ) {
                    Ok(result) => {
                        info!(status = ?result.status, message = %result.message, "Insert result");
                        emit_insert_result(&app_handle, result);
                        app_handle
                            .state::<StatusManager>()
                            .update_state(DictationState::Idle, "transcription".to_string());
                        emit_status(&app_handle);
                    }
                    Err(error) => emit_error(&app_handle, "clipboard", error),
                }
            }
            Err(error) => emit_error(&app_handle, "transcription", error),
        }
    });
}

#[tauri::command]
pub fn get_status(status_manager: State<'_, StatusManager>) -> StatusPayload {
    status_manager.snapshot()
}

#[tauri::command]
pub fn get_settings(settings_state: State<'_, SettingsState>) -> AppSettings {
    settings_state.get()
}

#[tauri::command]
pub fn save_settings(
    app_handle: AppHandle,
    settings_state: State<'_, SettingsState>,
    settings: AppSettings,
) -> Result<AppSettings, String> {
    let normalized =
        transcription::model::autofill_settings_model_path(&app_handle, &settings.normalized());
    settings::save_settings(&app_handle, &normalized).map_err(|error| error.to_string())?;
    settings_state.replace(normalized.clone());
    apply_settings_snapshot(&app_handle, &normalized);
    reload_model_async(app_handle);
    Ok(normalized)
}

#[tauri::command]
pub fn list_audio_devices() -> Result<Vec<audio::AudioDeviceInfo>, String> {
    audio::list_input_devices().map_err(|error| error.to_string())
}

#[tauri::command]
pub fn start_recording(app_handle: AppHandle) -> Result<(), String> {
    start_recording_internal(&app_handle, "command").map_err(|error| error.to_string())
}

#[tauri::command]
pub fn stop_recording(app_handle: AppHandle) -> Result<(), String> {
    stop_recording_internal(&app_handle, "command").map_err(|error| error.to_string())
}

#[tauri::command]
pub fn toggle_recording(app_handle: AppHandle) -> Result<(), String> {
    toggle_recording_internal(&app_handle).map_err(|error| error.to_string())
}

#[tauri::command]
pub fn reset_dictation_state(
    app_handle: AppHandle,
    recording_manager: State<'_, RecordingManager>,
) -> Result<(), String> {
    let status = app_handle.state::<StatusManager>().snapshot();
    if status.state == DictationState::Processing {
        return Err("Cannot reset while transcription is still running".to_string());
    }

    recording_manager
        .reset()
        .map_err(|error| error.to_string())?;
    app_handle
        .state::<StatusManager>()
        .update_state(DictationState::Idle, "command".to_string());
    emit_status(&app_handle);
    Ok(())
}

#[tauri::command]
pub async fn test_transcription_pipeline(
    app_handle: AppHandle,
) -> Result<TranscriptPayload, String> {
    let handle = app_handle.clone();
    tauri::async_runtime::spawn_blocking(move || {
        handle
            .state::<WhisperManager>()
            .transcribe(&vec![0.0f32; 16_000])
    })
    .await
    .map_err(|error| error.to_string())?
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub fn open_macos_preference_pane(kind: String) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let url = match kind.as_str() {
            "microphone" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone"
            }
            "accessibility" => {
                "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"
            }
            _ => return Err(format!("Unsupported preference pane: {kind}")),
        };

        Command::new("open")
            .arg(url)
            .status()
            .map_err(|error| format!("Failed to open macOS preference pane: {error}"))?
            .success()
            .then_some(())
            .ok_or_else(|| "macOS did not open the requested preference pane".to_string())
    }

    #[cfg(not(target_os = "macos"))]
    {
        let _ = kind;
        Err("This command is only supported on macOS".to_string())
    }
}
