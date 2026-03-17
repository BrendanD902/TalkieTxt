# TalkieTxt Local Dictation

Local macOS dictation app built with Tauri 2, Rust, `cpal`, `whisper-rs`, and a lightweight Vite + TypeScript UI.

## Dictation architecture

The default path is now fully local and in-process:

1. `Option+Space` starts push-to-talk recording.
2. `cpal` captures microphone audio into an in-memory `Vec<f32>`.
3. The backend downmixes to mono and resamples to 16 kHz in memory.
4. `whisper-rs` transcribes with a warm Whisper model already loaded in app state.
5. The transcript is inserted through the existing clipboard + Cmd+V macOS path.

The older `record -> WAV -> spawn whisper.cpp CLI -> wait -> read transcript file -> paste` bottleneck is removed from the default path. WAV export is only used when `Save debug WAV` is enabled.

## Current capabilities

- Warm in-process local transcription with `whisper-rs`
- Push-to-talk by default, with toggle mode as an option
- Local GGML model selection through the UI
- First-run onboarding for model setup, macOS permission guidance, and tray behavior
- In-memory downmix + resample pipeline for 44.1 kHz, 48 kHz, mono, and stereo microphones
- Clipboard paste or copy-only fallback on macOS
- Persistent settings for model path, preset, paste behavior, preferred input device, and logging level
- Status events for model loading, listening, processing, paste results, and errors

## Recommended models for Intel Mac

- `ggml-tiny.en.bin`: fastest default, best first choice for low latency
- `ggml-base.en.bin`: slower but usually more accurate

Place the model anywhere local on disk, then select it from the UI. The app does not download models automatically in v1.

## Run locally

```bash
cd app
npm install
cd src-tauri
cargo check
cd ..
npm run tauri dev
```

## Run as a normal Mac app

After building a release bundle, open the packaged app directly from Finder or Applications. The terminal is only required for `tauri dev`.

```bash
cd app
npm run tauri build
open "src-tauri/target/release/bundle/macos/TalkieTxt.app"
```

Closing the main window hides it, but the tray app and hotkey stay active. Use the tray menu to reopen the window or fully quit.

## Notes

- The app targets macOS Intel first.
- No cloud APIs are used.
- No `whisper.cpp` CLI binary is required in the default path.
- `cmake` is required locally because `whisper-rs` builds `whisper.cpp` from source during the Rust build.
