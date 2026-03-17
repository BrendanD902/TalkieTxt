# Dictation Pipeline Test Plan

## Automated checks

- Run `cargo test`
- Run `cargo check`
- Run `npm run build`

## Manual scenarios

### Model readiness

- Start the app with no saved model path and confirm status is `missing`.
- Save an invalid model path and confirm status moves to `failed` with a clear message.
- Save a valid local model path and confirm status moves `loading -> ready`.

### Input coverage

- Record with a 44.1 kHz input device and confirm transcription completes.
- Record with a 48 kHz input device and confirm transcription completes.
- Record with a mono input device and confirm transcription completes.
- Record with a stereo input device and confirm transcription completes.
- Select a preferred device, relaunch, and confirm it is used when present.
- Select a preferred device that is unavailable, relaunch, and confirm fallback to the default input device.

### Utterance coverage

- Speak a short utterance and confirm low-latency transcript insertion.
- Speak a longer utterance and confirm the app stays responsive while processing.
- Perform rapid repeated push-to-talk cycles and confirm duplicate starts/stops are handled safely.

### Paste behavior

- Keep `Paste after transcribe` enabled and confirm text is inserted into the active app.
- Disable `Paste after transcribe` and confirm the transcript is copied without paste automation.
- Trigger a paste failure path and confirm clipboard-only fallback is surfaced clearly.

### Debug path

- Enable `Save debug WAV` and confirm a debug WAV is written outside the normal hot path.
- Disable `Save debug WAV` and confirm normal dictation does not write WAV files.
