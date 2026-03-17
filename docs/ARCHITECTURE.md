# Dictation Architecture

## Old bottleneck

The previous path was:

`cpal capture -> temp WAV -> spawn whisper.cpp CLI -> wait for process -> read transcript file -> clipboard paste`

That design paid startup and disk I/O costs on every utterance:

- audio was written to disk before transcription
- Whisper was loaded by an external process for every utterance
- the app waited on transcript file creation before insertion
- the pipeline lived mostly inside one large backend file

## Current path

The default path is now:

`global shortcut -> in-memory capture -> mono downmix -> optional 16 kHz resample -> whisper-rs -> clipboard paste`

## Backend layout

- `src/lib.rs`: Tauri setup, tray, shortcut registration, plugin wiring
- `src/commands.rs`: commands, status events, recording lifecycle, async transcription job
- `src/state.rs`: runtime status, managed settings state, recording manager
- `src/settings.rs`: persisted settings and migration from legacy whisper CLI settings
- `src/clipboard.rs`: clipboard copy + macOS Cmd+V insertion logic
- `src/audio/`: capture, conversion, resampling, optional debug WAV export
- `src/transcription/`: model selection and warm `whisper-rs` engine

## Runtime behavior

- The app loads persisted settings on startup.
- If a model path is configured, model loading begins immediately and status is emitted as `loading`.
- If the model is missing or invalid, the app stays responsive and surfaces a clear UI error state.
- Recording is only allowed when the model is `ready`.
- Stopping a recording moves the app to `processing`, performs conversion in memory, then runs transcription on a blocking worker thread.
- Text insertion is isolated from transcription and can be disabled so the result is copied without paste automation.

## Future-ready boundaries

The current refactor is push-to-talk first. The module boundaries are set up so VAD or streaming partials can be added later without rewriting capture, conversion, or the warm model manager.
