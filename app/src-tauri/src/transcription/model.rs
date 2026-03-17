use std::path::{Path, PathBuf};

use tauri::{AppHandle, Manager};

use crate::{
    error::{AppError, AppResult},
    settings::{AppSettings, DefaultModelPreset},
};

pub fn configured_model_path(settings: &AppSettings) -> Option<PathBuf> {
    settings.model_path.as_ref().map(PathBuf::from)
}

pub fn resolve_model_path(
    app_handle: &AppHandle,
    settings: &AppSettings,
) -> AppResult<Option<PathBuf>> {
    if let Some(configured_path) = configured_model_path(settings) {
        if let Ok(validated) = validate_model_path(&configured_path) {
            return Ok(Some(validated));
        }
    }

    if let Some(discovered_path) =
        discover_managed_model_path(app_handle, settings.default_model_preset)
    {
        return Ok(Some(discovered_path));
    }

    if let Some(configured_path) = configured_model_path(settings) {
        return Err(AppError::ModelMissing(configured_path));
    }

    Ok(None)
}

pub fn autofill_settings_model_path(app_handle: &AppHandle, settings: &AppSettings) -> AppSettings {
    let mut updated = settings.clone();

    if let Ok(Some(path)) = resolve_model_path(app_handle, settings) {
        updated.model_path = Some(path.to_string_lossy().to_string());
    }

    updated.normalized()
}

pub fn validate_model_path(path: &Path) -> AppResult<PathBuf> {
    if !path.exists() || !path.is_file() {
        return Err(AppError::ModelMissing(path.to_path_buf()));
    }
    Ok(path.to_path_buf())
}

pub fn recommended_model_filename(preset: DefaultModelPreset) -> &'static str {
    match preset {
        DefaultModelPreset::TinyEn => "ggml-tiny.en.bin",
        DefaultModelPreset::BaseEn => "ggml-base.en.bin",
    }
}

fn alternate_model_filename(preset: DefaultModelPreset) -> &'static str {
    match preset {
        DefaultModelPreset::TinyEn => recommended_model_filename(DefaultModelPreset::BaseEn),
        DefaultModelPreset::BaseEn => recommended_model_filename(DefaultModelPreset::TinyEn),
    }
}

pub fn discover_managed_model_path(
    app_handle: &AppHandle,
    preset: DefaultModelPreset,
) -> Option<PathBuf> {
    let candidate_filenames = [
        recommended_model_filename(preset),
        alternate_model_filename(preset),
    ];

    let base_dirs = [
        app_handle.path().app_data_dir().ok(),
        app_handle.path().app_config_dir().ok(),
    ];

    for base_dir in base_dirs.into_iter().flatten() {
        let models_dir = base_dir.join("models");
        for filename in candidate_filenames {
            let candidate = models_dir.join(filename);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}
