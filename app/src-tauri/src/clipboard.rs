use std::{process::Command, thread, time::Duration};

use arboard::Clipboard;
#[cfg(target_os = "macos")]
use core_graphics::{
    event::{CGEvent, CGEventFlags, CGEventTapLocation, KeyCode},
    event_source::{CGEventSource, CGEventSourceStateID},
};
use serde::Serialize;
#[cfg(target_os = "macos")]
use tracing::warn;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum InsertStatus {
    Pasted,
    Typed,
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

fn with_diagnostics(summary: impl Into<String>, diagnostics: &[String]) -> String {
    let summary = summary.into();
    if diagnostics.is_empty() {
        return summary;
    }

    format!("{summary} Diagnostics: {}", diagnostics.join(" | "))
}

pub fn insert_text(
    text: &str,
    paste_after_transcribe: bool,
    target_app_bundle_id: Option<&str>,
) -> AppResult<InsertResult> {
    insert_text_with(
        text,
        paste_after_transcribe,
        target_app_bundle_id,
        copy_text_to_clipboard,
        trigger_platform_insert,
    )
}

fn insert_text_with<C, F>(
    text: &str,
    paste_after_transcribe: bool,
    target_app_bundle_id: Option<&str>,
    copy_fn: C,
    paste_fn: F,
) -> AppResult<InsertResult>
where
    C: Fn(&str) -> AppResult<()>,
    F: Fn(&str, Option<&str>) -> AppResult<InsertResult>,
{
    if text.trim().is_empty() {
        return Err(AppError::Clipboard(
            "Transcript is empty; nothing to insert".to_string(),
        ));
    }

    copy_fn(text)?;

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
        Ok(result) => Ok(result),
        Err(error) => Ok(InsertResult {
            status: InsertStatus::ClipboardOnly,
            message: format!("Paste automation failed; transcript copied to clipboard. {error}"),
        }),
    }
}

fn copy_text_to_clipboard(text: &str) -> AppResult<()> {
    let mut clipboard = Clipboard::new()
        .map_err(|error| AppError::Clipboard(format!("Failed to access clipboard: {error}")))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|error| AppError::Clipboard(format!("Failed to copy transcript: {error}")))?;
    Ok(())
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
fn trigger_platform_insert(
    text: &str,
    target_app_bundle_id: Option<&str>,
) -> AppResult<InsertResult> {
    // Keep insertion behavior predictable: write the transcript to the clipboard,
    // then try to restore the target app and insert it back into the cursor.
    trigger_platform_paste(text, target_app_bundle_id)
}

#[cfg(target_os = "macos")]
fn trigger_platform_paste(text: &str, target_app_bundle_id: Option<&str>) -> AppResult<InsertResult> {
    let mut diagnostics = Vec::new();

    if let Some(bundle_id) = target_app_bundle_id {
        match activate_app(bundle_id) {
            Ok(()) => {
                diagnostics.push(format!("refocus ok: {bundle_id}"));
                thread::sleep(Duration::from_millis(220));
            }
            Err(error) => {
                diagnostics.push(format!("refocus failed: {error}"));
                warn!("{error}");
            }
        }
    } else {
        diagnostics.push("refocus skipped: no target app bundle id".to_string());
    }

    thread::sleep(Duration::from_millis(120));

    match trigger_applescript_paste() {
        Ok(()) => {
            diagnostics.push("applescript paste ok".to_string());
            return Ok(InsertResult {
                status: InsertStatus::Pasted,
                message: with_diagnostics("Transcript pasted at the active cursor.", &diagnostics),
            })
        }
        Err(applescript_error) => {
            diagnostics.push(format!("applescript paste failed: {applescript_error}"));
            warn!("{applescript_error}");
        }
    }

    match trigger_native_paste() {
        Ok(()) => {
            diagnostics.push("native paste ok".to_string());
            return Ok(InsertResult {
                status: InsertStatus::Pasted,
                message: with_diagnostics(
                    "Transcript pasted at the active cursor with native keyboard events.",
                    &diagnostics,
                ),
            })
        }
        Err(native_error) => {
            diagnostics.push(format!("native paste failed: {native_error}"));
            warn!("{native_error}");
        }
    }

    match trigger_native_typing(text) {
        Ok(()) => {
            diagnostics.push("native typing fallback ok".to_string());
            return Ok(InsertResult {
                status: InsertStatus::Typed,
                message: with_diagnostics(
                    "Paste automation failed, so the transcript was typed at the active cursor with native keyboard events.",
                    &diagnostics,
                ),
            });
        }
        Err(native_typing_error) => {
            diagnostics.push(format!(
                "native typing fallback failed: {native_typing_error}"
            ));
            warn!("{native_typing_error}");
        }
    }

    match trigger_applescript_typing(text) {
        Ok(()) => {
            diagnostics.push("applescript typing fallback ok".to_string());
            return Ok(InsertResult {
                status: InsertStatus::Typed,
                message: with_diagnostics(
                    "Paste automation failed, so the transcript was typed at the active cursor.",
                    &diagnostics,
                ),
            });
        }
        Err(typing_error) => {
            diagnostics.push(format!("applescript typing fallback failed: {typing_error}"));
            return Err(AppError::Clipboard(with_diagnostics(
                "All insertion methods failed.",
                &diagnostics,
            )));
        }
    }
}

#[cfg(target_os = "macos")]
fn trigger_native_paste() -> AppResult<()> {
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

#[cfg(target_os = "macos")]
fn activate_app(bundle_id: &str) -> AppResult<()> {
    run_osascript(&[&format!("tell application id \"{bundle_id}\" to activate")]).or_else(
        |applescript_error| {
            warn!("{applescript_error}");
            let output = Command::new("open")
                .arg("-b")
                .arg(bundle_id)
                .output()
                .map_err(|error| {
                    AppError::Clipboard(format!(
                        "Failed to ask macOS to refocus the target app: {error}"
                    ))
                })?;

            if output.status.success() {
                Ok(())
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
                Err(AppError::Clipboard(format!(
                    "macOS could not refocus the target app before paste{}",
                    if stderr.is_empty() {
                        String::new()
                    } else {
                        format!(": {stderr}")
                    }
                )))
            }
        },
    )
}

#[cfg(not(target_os = "macos"))]
fn trigger_platform_insert(
    _text: &str,
    _target_app_bundle_id: Option<&str>,
) -> AppResult<InsertResult> {
    Err(AppError::Clipboard(
        "Paste automation is only configured for macOS".to_string(),
    ))
}

#[cfg(target_os = "macos")]
fn trigger_applescript_paste() -> AppResult<()> {
    run_osascript(&[
        "tell application \"System Events\"",
        "keystroke \"v\" using {command down}",
        "end tell",
    ])
}

#[cfg(target_os = "macos")]
fn trigger_applescript_typing(text: &str) -> AppResult<()> {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();

    for (index, line) in lines.iter().enumerate() {
        if !line.is_empty() {
            run_osascript_with_args(
                &[
                    "on run argv",
                    "tell application \"System Events\"",
                    "keystroke item 1 of argv",
                    "end tell",
                    "end run",
                ],
                &[*line],
            )?;
        }

        if index + 1 < lines.len() {
            run_osascript(&[
                "tell application \"System Events\"",
                "key code 36",
                "end tell",
            ])?;
        }
    }

    Ok(())
}

#[cfg(target_os = "macos")]
fn trigger_native_typing(text: &str) -> AppResult<()> {
    let source = CGEventSource::new(CGEventSourceStateID::HIDSystemState).map_err(|_| {
        AppError::Clipboard(
            "Failed to create native macOS HID event source for typing".to_string(),
        )
    })?;

    let key_down = CGEvent::new_keyboard_event(source.clone(), KeyCode::SPACE, true).map_err(|_| {
        AppError::Clipboard("Failed to create native macOS typing key-down event".to_string())
    })?;
    key_down.set_string(text);

    let key_up = CGEvent::new_keyboard_event(source, KeyCode::SPACE, false).map_err(|_| {
        AppError::Clipboard("Failed to create native macOS typing key-up event".to_string())
    })?;
    key_up.set_string(text);

    key_down.post(CGEventTapLocation::HID);
    thread::sleep(Duration::from_millis(12));
    key_up.post(CGEventTapLocation::HID);

    Ok(())
}

#[cfg(target_os = "macos")]
fn run_osascript(lines: &[&str]) -> AppResult<()> {
    run_osascript_with_args(lines, &[])
}

#[cfg(target_os = "macos")]
fn run_osascript_with_args(lines: &[&str], args: &[&str]) -> AppResult<()> {
    let mut command = Command::new("osascript");
    for line in lines {
        command.arg("-e").arg(line);
    }
    command.args(args);

    let output = command.output().map_err(|error| {
        AppError::Clipboard(format!("Failed to run macOS automation script: {error}"))
    })?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "unknown AppleScript error".to_string()
    };

    Err(AppError::Clipboard(format!(
        "macOS automation script failed: {detail}"
    )))
}

#[cfg(test)]
mod tests {
    use super::{insert_text_with, InsertResult, InsertStatus};
    use crate::error::AppError;

    #[test]
    fn copy_only_mode_skips_paste() {
        let result = insert_text_with(
            "hello",
            false,
            None,
            |_| Ok(()),
            |_, _| panic!("paste should not run"),
        )
        .expect("copy only should succeed");

        assert!(matches!(result.status, InsertStatus::ClipboardOnly));
    }

    #[test]
    fn paste_failure_falls_back_to_clipboard_only() {
        let result = insert_text_with(
            "hello",
            true,
            Some("com.apple.TextEdit"),
            |_| Ok(()),
            |_, _| Err(AppError::Clipboard("no automation permission".to_string())),
        )
        .expect("clipboard fallback should succeed");

        assert!(matches!(result.status, InsertStatus::ClipboardOnly));
        assert!(result.message.contains("Paste automation failed"));
    }

    #[test]
    fn typed_fallback_result_is_returned() {
        let result = insert_text_with(
            "hello",
            true,
            Some("com.apple.TextEdit"),
            |_| Ok(()),
            |_, _| {
                Ok(InsertResult {
                    status: InsertStatus::Typed,
                    message: "Transcript was typed".to_string(),
                })
            },
        )
        .expect("typed fallback should succeed");

        assert!(matches!(result.status, InsertStatus::Typed));
    }
}
