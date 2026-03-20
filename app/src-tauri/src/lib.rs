mod audio;
mod clipboard;
mod commands;
mod error;
mod settings;
mod state;
mod transcription;

use settings::AppSettings;
use state::DictationState;
use tauri::{
    image::Image,
    menu::MenuBuilder,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Listener, Manager, WindowEvent,
};
use tauri_plugin_global_shortcut::{
    Builder as GlobalShortcutBuilder, Code, GlobalShortcutExt, Modifiers, Shortcut,
};
use tracing_subscriber::{fmt, EnvFilter};

use crate::{
    commands::{
        get_settings, get_status, handle_hotkey_event, list_audio_devices,
        open_macos_preference_pane, reload_model_async, reset_dictation_state, save_settings,
        start_recording, stop_recording, test_transcription_pipeline, toggle_recording,
    },
    error::AppError,
    settings::load_settings,
    state::{RecordingManager, SettingsState, StatusManager},
    transcription::WhisperManager,
};

const MAIN_WINDOW_LABEL: &str = "main";
const OVERLAY_WINDOW_LABEL: &str = "overlay";
const TRAY_ICON_ID: &str = "walkie-tray";
const TRAY_MENU_SHOW_ID: &str = "tray_show";
const TRAY_MENU_QUIT_ID: &str = "tray_quit";

fn default_shortcut() -> Shortcut {
    Shortcut::new(Some(Modifiers::ALT), Code::Space)
}

fn init_logging() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = fmt().with_env_filter(filter).with_target(false).try_init();
}

fn show_main_window(app_handle: &tauri::AppHandle) {
    if let Some(window) = app_handle.get_webview_window(MAIN_WINDOW_LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn sync_overlay_visibility(app_handle: &tauri::AppHandle, state: DictationState) {
    if let Some(overlay) = app_handle.get_webview_window(OVERLAY_WINDOW_LABEL) {
        match state {
            DictationState::Listening | DictationState::Processing => {
                let _ = overlay.show();
            }
            DictationState::Idle | DictationState::Error => {
                let _ = overlay.hide();
            }
        }
    }
}

fn update_tray_icon(app_handle: &tauri::AppHandle, state: DictationState) {
    let tray = match app_handle.tray_by_id(TRAY_ICON_ID) {
        Some(tray) => tray,
        None => return,
    };

    #[cfg(target_os = "macos")]
    {
        let icon_bytes = match state {
            DictationState::Listening => include_bytes!("../icons/trayColor.png").as_slice(),
            _ => include_bytes!("../icons/trayTemplate.png").as_slice(),
        };

        if let Ok(icon) = Image::from_bytes(icon_bytes) {
            let is_template = !matches!(state, DictationState::Listening);
            let _ = tray.set_icon(Some(icon));
            let _ = tray.set_icon_as_template(is_template);
        }
    }
}

fn install_tray_icon(app_handle: &tauri::AppHandle) -> Result<(), AppError> {
    let tray_menu = MenuBuilder::new(app_handle)
        .text(TRAY_MENU_SHOW_ID, "Show")
        .separator()
        .text(TRAY_MENU_QUIT_ID, "Quit")
        .build()
        .map_err(|error| AppError::Message(format!("Failed to build tray menu: {error}")))?;

    let mut tray_builder = TrayIconBuilder::with_id(TRAY_ICON_ID)
        .menu(&tray_menu)
        .tooltip("TalkieTxt")
        .show_menu_on_left_click(false)
        .on_menu_event(|app_handle, event| {
            if event.id() == TRAY_MENU_SHOW_ID {
                show_main_window(app_handle);
                return;
            }

            if event.id() == TRAY_MENU_QUIT_ID {
                app_handle.exit(0);
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button,
                button_state,
                ..
            } = event
            {
                if button == MouseButton::Left && button_state == MouseButtonState::Up {
                    show_main_window(tray.app_handle());
                }
            }
        });

    #[cfg(target_os = "macos")]
    {
        let icon = Image::from_bytes(include_bytes!("../icons/trayTemplate.png"))
            .map_err(|error| AppError::Message(format!("Failed to load tray icon: {error}")))?;
        tray_builder = tray_builder.icon(icon).icon_as_template(true);
    }

    #[cfg(not(target_os = "macos"))]
    {
        if let Some(icon) = app_handle.default_window_icon().cloned() {
            tray_builder = tray_builder.icon(icon);
        }
    }

    tray_builder
        .build(app_handle)
        .map_err(|error| AppError::Message(format!("Failed to create tray icon: {error}")))?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    init_logging();

    tauri::Builder::default()
        .manage(SettingsState::new(AppSettings::default()))
        .manage(StatusManager::new(default_shortcut()))
        .manage(RecordingManager::default())
        .manage(WhisperManager::default())
        .plugin(
            GlobalShortcutBuilder::new()
                .with_handler(handle_hotkey_event)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == MAIN_WINDOW_LABEL {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .setup(|app| {
            let app_handle = app.handle().clone();
            let shortcut = app.state::<StatusManager>().shortcut();

            app.global_shortcut()
                .register(shortcut)
                .map_err(|error| AppError::Shortcut(error.to_string()))?;

            let loaded_settings = load_settings(&app_handle).unwrap_or_default();
            let settings =
                transcription::model::autofill_settings_model_path(&app_handle, &loaded_settings);
            if settings != loaded_settings {
                let _ = crate::settings::save_settings(&app_handle, &settings);
            }
            app.state::<SettingsState>().replace(settings.clone());
            app.state::<StatusManager>().apply_settings(&settings);

            install_tray_icon(&app_handle)?;

            // Listen for status changes to update overlay and tray
            let status_handle = app_handle.clone();
            app.listen("dictation://status", move |_event| {
                let state = status_handle.state::<StatusManager>().snapshot().state;
                sync_overlay_visibility(&status_handle, state);
                update_tray_icon(&status_handle, state);
            });

            commands::emit_status(&app_handle);
            reload_model_async(app_handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_settings,
            save_settings,
            list_audio_devices,
            start_recording,
            stop_recording,
            toggle_recording,
            reset_dictation_state,
            test_transcription_pipeline,
            open_macos_preference_pane
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
