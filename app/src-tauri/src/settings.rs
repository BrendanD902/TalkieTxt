use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::error::{AppError, AppResult};

const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum HotkeyMode {
    Toggle,
    HoldToTalk,
}

impl Default for HotkeyMode {
    fn default() -> Self {
        Self::HoldToTalk
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DefaultModelPreset {
    TinyEn,
    BaseEn,
}

impl Default for DefaultModelPreset {
    fn default() -> Self {
        Self::TinyEn
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl Default for LogLevel {
    fn default() -> Self {
        Self::Info
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub model_path: Option<String>,
    #[serde(default)]
    pub default_model_preset: DefaultModelPreset,
    #[serde(default = "default_true")]
    pub paste_after_transcribe: bool,
    #[serde(default)]
    pub save_debug_wav: bool,
    #[serde(default)]
    pub log_level: LogLevel,
    pub preferred_input_device: Option<String>,
    #[serde(default)]
    pub hotkey_mode: HotkeyMode,
    #[serde(default)]
    pub has_completed_onboarding: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            model_path: None,
            default_model_preset: DefaultModelPreset::TinyEn,
            paste_after_transcribe: true,
            save_debug_wav: false,
            log_level: LogLevel::Info,
            preferred_input_device: None,
            hotkey_mode: HotkeyMode::HoldToTalk,
            has_completed_onboarding: false,
        }
    }
}

impl AppSettings {
    pub fn normalized(mut self) -> Self {
        self.model_path = normalize_optional(self.model_path);
        self.preferred_input_device = normalize_optional(self.preferred_input_device);
        self
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LegacySettings {
    whisper_model_path: Option<String>,
    hotkey_mode: Option<HotkeyMode>,
}

fn default_true() -> bool {
    true
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|candidate| {
        let trimmed = candidate.trim();
        (!trimmed.is_empty()).then(|| trimmed.to_string())
    })
}

pub fn load_settings(app_handle: &AppHandle) -> AppResult<AppSettings> {
    let settings_path = settings_file_path(app_handle)?;
    if !settings_path.exists() {
        return Ok(AppSettings::default());
    }

    let raw = fs::read_to_string(&settings_path).map_err(|error| {
        AppError::Settings(format!(
            "Failed to read settings file {}: {error}",
            settings_path.display()
        ))
    })?;

    parse_settings(&raw)
}

pub fn save_settings(app_handle: &AppHandle, settings: &AppSettings) -> AppResult<()> {
    let settings_path = settings_file_path(app_handle)?;
    let payload = serde_json::to_string_pretty(&settings.clone().normalized())
        .map_err(|error| AppError::Settings(format!("Failed to serialize settings: {error}")))?;

    fs::write(&settings_path, payload).map_err(|error| {
        AppError::Settings(format!(
            "Failed to write settings file {}: {error}",
            settings_path.display()
        ))
    })?;

    Ok(())
}

fn settings_file_path(app_handle: &AppHandle) -> AppResult<PathBuf> {
    let config_dir = app_handle.path().app_config_dir().map_err(|error| {
        AppError::Settings(format!("Failed to resolve app config directory: {error}"))
    })?;

    fs::create_dir_all(&config_dir).map_err(|error| {
        AppError::Settings(format!(
            "Failed to create app config dir {}: {error}",
            config_dir.display()
        ))
    })?;

    Ok(config_dir.join(SETTINGS_FILE_NAME))
}

pub fn parse_settings(raw: &str) -> AppResult<AppSettings> {
    let value: serde_json::Value = serde_json::from_str(raw)
        .map_err(|error| AppError::Settings(format!("Failed to parse settings file: {error}")))?;

    if value.get("whisperModelPath").is_some() || value.get("whisper_model_path").is_some() {
        let legacy: LegacySettings = serde_json::from_value(value).map_err(|error| {
            AppError::Settings(format!("Failed to parse legacy settings file: {error}"))
        })?;
        return Ok(AppSettings {
            model_path: normalize_optional(legacy.whisper_model_path),
            hotkey_mode: legacy.hotkey_mode.unwrap_or_default(),
            ..AppSettings::default()
        });
    }

    serde_json::from_str::<AppSettings>(raw)
        .map(AppSettings::normalized)
        .map_err(|error| AppError::Settings(format!("Failed to parse settings file: {error}")))
}

#[cfg(test)]
mod tests {
    use super::{parse_settings, AppSettings, DefaultModelPreset, HotkeyMode};

    #[test]
    fn migrates_legacy_whisper_settings() {
        let parsed = parse_settings(
            r#"{
                "whisperModelPath": "/tmp/ggml-tiny.en.bin",
                "hotkeyMode": "toggle"
            }"#,
        )
        .expect("legacy settings should parse");

        assert_eq!(parsed.model_path.as_deref(), Some("/tmp/ggml-tiny.en.bin"));
        assert_eq!(parsed.hotkey_mode, HotkeyMode::Toggle);
        assert_eq!(parsed.default_model_preset, DefaultModelPreset::TinyEn);
        assert!(parsed.paste_after_transcribe);
        assert!(!parsed.has_completed_onboarding);
    }

    #[test]
    fn normalizes_empty_optional_fields() {
        let parsed = parse_settings(
            r#"{
                "modelPath": "   ",
                "preferredInputDevice": "",
                "defaultModelPreset": "baseEn",
                "hasCompletedOnboarding": true
            }"#,
        )
        .expect("settings should parse");

        assert_eq!(
            parsed,
            AppSettings {
                model_path: None,
                preferred_input_device: None,
                default_model_preset: DefaultModelPreset::BaseEn,
                has_completed_onboarding: true,
                ..AppSettings::default()
            }
        );
    }
}
