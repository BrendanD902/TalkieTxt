# Troubleshooting

## Model status stays `missing`

- Open settings and choose a local `.bin` GGML model file.
- Save settings and wait for the status to move from `loading` to `ready`.

## Model status becomes `failed`

- Confirm the path points to a real file.
- Confirm the file is a Whisper GGML model such as `ggml-tiny.en.bin` or `ggml-base.en.bin`.
- Re-save the settings to trigger a reload.

## Rust build fails on `whisper-rs-sys`

- Install `cmake`.
- Re-run `cargo check` or `npm run tauri dev`.

## Recording starts but no transcript appears

- Confirm the model status is `ready` before recording.
- Check microphone permissions in macOS System Settings.
- Try a different preferred input device in the settings panel.
- Verify the microphone sample rate/device path from the status area.

## Paste fails but the transcript exists

- The app falls back to clipboard-only behavior if macOS paste automation fails.
- Confirm the app has the accessibility permissions needed for simulated Cmd+V.
- The transcript should still be available in the clipboard and inside the app UI.

## Rapid hotkey use feels stuck

- The app allows one active recording or one active transcription at a time.
- Wait for the state to return to `idle` before starting another utterance.

## Current limitations

- No VAD in v1
- No streaming partial transcript path in v1
- English-first defaults
- Clipboard insertion only, no direct typing path
