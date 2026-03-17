use std::{process::Command, thread, time::Duration};

use arboard::Clipboard;
#[cfg(target_os = "macos")]
use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode},
    event_source::{CGEventSource, CGEventSourceStateID},
};
use serde::Serialize;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InsertStatus {
    Pasted,
    ClipboardOnly,
    Skipped,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InsertResult {
    pub status: InsertStatus,
    pub message: String,
}

const APP_BUNDLE_ID: &str = "com.brendandalziel.walkietalkie";
const APP_DISPLAY_NAME: &str = "TalkieTxt";

pub fn insert_text(
    text: &str,
    paste_after_transcribe: bool,
    target_app_bundle_id: Option<&str>,
) -> AppResult<InsertResult> {
    insert_text_with(
        text,
        paste_after_transcribe,
        target_app_bundle_id,
        trigger_platform_insert,
    )
}

fn insert_text_with<F>(
    text: &str,
    paste_after_transcribe: bool,
    target_app_bundle_id: Option<&str>,
    paste_fn: F,
) -> AppResult<InsertResult>
where
    F: Fn(&str, Option<&str>) -> AppResult<()>,
{
    if text.trim().is_empty() {
        return Err(AppError::Clipboard(
            "Transcript is empty; nothing to insert".to_string(),
        ));
    }

    let mut clipboard = Clipboard::new()
        .map_err(|error| AppError::Clipboard(format!("Failed to access clipboard: {error}")))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|error| AppError::Clipboard(format!("Failed to copy transcript: {error}")))?;

    if !paste_after_transcribe {
        return Ok(InsertResult {
            status: InsertStatus::ClipboardOnly,
            message: "Transcript copied to clipboard".to_string(),
        });
    }

    if target_app_bundle_id == Some(APP_BUNDLE_ID) {
        return Ok(InsertResult {
            status: InsertStatus::ClipboardOnly,
            message:
                format!(
                    "{APP_DISPLAY_NAME} was frontmost during recording; transcript copied to clipboard. Focus another app before dictating."
                ),
        });
    }

    match paste_fn(text, target_app_bundle_id) {
        Ok(()) => Ok(InsertResult {
            status: InsertStatus::Pasted,
            message: "Transcript pasted at the active cursor".to_string(),
        }),
        Err(error) => Ok(InsertResult {
            status: InsertStatus::ClipboardOnly,
            message: format!("Paste automation failed; transcript copied to clipboard ({error})"),
        }),
    }
}

#[cfg(target_os = "macos")]
pub fn frontmost_app_bundle_id() -> Option<String> {
    let output = Command::new("osascript")
        .arg("-e")
        .arg(
            "tell application \"System Events\" to get bundle identifier of first application process whose frontmost is true",
        )
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let bundle_id = String::from_utf8(output.stdout).ok()?.trim().to_string();
    (!bundle_id.is_empty()).then_some(bundle_id)
}

#[cfg(not(target_os = "macos"))]
pub fn frontmost_app_bundle_id() -> Option<String> {
    None
}

#[cfg(target_os = "macos")]
fn trigger_platform_insert(_text: &str, target_app_bundle_id: Option<&str>) -> AppResult<()> {
    // Keep insertion behavior predictable: write the transcript to the clipboard,
    // then paste it back into the previously focused app.
    trigger_platform_paste(target_app_bundle_id)
}

#[cfg(target_os = "macos")]
fn trigger_platform_paste(_target_app_bundle_id: Option<&str>) -> AppResult<()> {
    thread::sleep(Duration::from_millis(120));

    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
        AppError::Clipboard("Failed to create native macOS HID event source for paste".to_string())
    })?;

    let command_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::COMMAND, true)
        .map_err(|_| {
            AppError::Clipboard("Failed to create native macOS Command-down event".to_string())
        })?;
    command_down.set_flags(CGEventFlags::CGEventFlagCommand);

    let v_down =
        CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_V, true).map_err(|_| {
            AppError::Clipboard("Failed to create native macOS V key-down event".to_string())
        })?;
    v_down.set_flags(CGEventFlags::CGEventFlagCommand);

    let v_up =
        CGEvent::new_keyboard_event(source.clone(), KeyCode::ANSI_V, false).map_err(|_| {
            AppError::Clipboard("Failed to create native macOS V key-up event".to_string())
        })?;
    v_up.set_flags(CGEventFlags::CGEventFlagCommand);

    let command_up =
        CGEvent::new_keyboard_event(source, KeyCode::COMMAND, false).map_err(|_| {
            AppError::Clipboard("Failed to create native macOS Command-up event".to_string())
        })?;
    command_up.set_flags(CGEventFlags::CGEventFlagNull);

    command_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(10));
    v_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(10));
    v_up.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(10));
    command_up.post(CGEventTapLocation::HID);

    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn trigger_platform_insert(_text: &str, _target_app_bundle_id: Option<&str>) -> AppResult<()> {
    Err(AppError::Clipboard(
        "Paste automation is only configured for macOS".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::{insert_text_with, InsertStatus};

    #[test]
    fn copy_only_mode_skips_paste() {
        let result = insert_text_with("hello", false, None, |_, _| panic!("paste should not run"))
            .expect("copy only should succeed");

        assert!(matches!(result.status, InsertStatus::ClipboardOnly));
    }
}
