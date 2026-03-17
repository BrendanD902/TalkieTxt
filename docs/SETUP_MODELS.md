# Model Setup

## Supported model path

v1 expects a local GGML Whisper model file:

- `ggml-tiny.en.bin`
- `ggml-base.en.bin`

Use the app settings panel to select the file path directly.

## Recommended defaults for Intel Mac

- Default preset: `tiny.en`
- Accuracy upgrade: `base.en`

`tiny.en` is the right default for low-latency dictation on Intel Mac because it keeps CPU load and turnaround time lower.

## Suggested storage

Any stable local path is fine. Common options:

- `~/Models/whisper/ggml-tiny.en.bin`
- `~/Library/Application Support/WalkieTalkie/models/ggml-base.en.bin`
- a project-local `models/` folder while developing

The app does not bundle or download models automatically in this version.

## Switching presets

The preset setting changes the UI recommendation and the expected default tier. It does not auto-download or auto-switch the file for you. To move from `tiny.en` to `base.en`, pick the new local file path in settings and save.

## Build prerequisite

`whisper-rs` builds Whisper locally as part of the Rust build, so `cmake` must be installed on the development machine.
