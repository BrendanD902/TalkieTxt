# Build Plan: Local macOS Voice Typing App (Personal Use)

## Objective
Ship a reliable offline dictation app you can use daily on your Intel Mac.

## Scope for personal-use V1
- Global hotkey start/stop.
- Record from selected microphone.
- Local transcription via whisper.cpp.
- Insert text at cursor via clipboard paste first.
- Lightweight menu bar status + overlay states.
- Basic settings and permission onboarding.

## Suggested timeline
- Session 1 (2-3h): Project bootstrap and hotkey wiring.
- Session 2 (2-3h): Audio capture + save WAV fixture.
- Session 3 (3-4h): whisper.cpp integration and transcript output.
- Session 4 (2-3h): Paste insertion flow + error recovery.
- Session 5 (2-3h): Overlay states and tray polish.
- Session 6 (2-3h): Settings + persistence + onboarding checks.
- Session 7 (2-3h): Intel tuning, regression pass, packaging draft.

## Phase plan with hard gates

### Phase A: Foundation
1. Create Tauri 2 app skeleton and menu bar/tray shell.
2. Add global shortcut registration and rebind-safe path.
3. Add app state machine skeleton (`Idle`, `Listening`, `Transcribing`, `Error`).

Gate A (must pass)
- Hotkey can toggle state without crash for 30 repeated presses.

### Phase B: Speech pipeline
1. Capture microphone input to PCM buffer.
2. Stop recording and export fixture WAV for debug.
3. Integrate whisper.cpp runner with configurable model path.
4. Return transcript + error object to UI.

Gate B (must pass)
- 10-second dictation produces local transcript with networking disabled.

### Phase C: Insert and recover
1. Implement clipboard-based insertion method first.
2. Add fallback copy-on-failure behavior.
3. Add user errors for missing model/permission denied.

Gate C (must pass)
- Transcript inserts into Cursor and Notes in 10/10 attempts each.

### Phase D: UX readiness
1. Implement 4 overlay states: Idle, Listening, Transcribing, Pasted.
2. Add tray status and quick actions.
3. Build settings: hotkey, mic, model, mode, paste method.
4. Build first-run onboarding checklist for permissions.

Gate D (must pass)
- Clean install path completes onboarding and first dictation without restart.

### Phase E: Personal productivity features
1. Add modes: Auto, Code, Email, Notes.
2. Add optional local cleanup pipeline (llama.cpp).
3. Add per-app mode override (Cursor => Code).

Gate E (must pass)
- Mode output differences are visible and reversible.

### Phase F: Intel optimization and release prep
1. Add quantized model choices and speed/quality labels.
2. Warm-load default model at app idle.
3. Tune silence detection and cancellation behavior.
4. Run final regression matrix and package.

Gate F (must pass)
- Stop-to-insert latency is consistently acceptable on your Intel machine.

## Technical decisions (fixed for now)
- Desktop shell: Tauri 2 + Rust.
- STT engine: whisper.cpp local.
- Initial insertion path: clipboard paste.
- Privacy mode: offline by default.

## Daily use success criteria
- You can trigger dictation while coding without breaking focus.
- Output quality is good enough that manual edits are occasional, not constant.
- App survives long sessions (50+ dictations) without restart.

## Test matrix (minimum)
- Apps: Cursor, Notes, browser text area.
- Inputs: short sentence, long paragraph, code snippet.
- Failures: mic disconnected, permission revoked, missing model.
- Repetition: rapid start/stop stress (20 cycles).

## Risks and mitigations
- Permission friction on macOS.
- Mitigation: first-run checklist + deep links to system settings.

- Hotkey conflicts.
- Mitigation: conflict detection and guided rebind.

- Intel latency with large models.
- Mitigation: default quantized small model + optional upgrade path.

## Start now (next concrete actions)
1. Bootstrap Tauri 2 project and confirm app launches.
2. Implement hotkey toggle and console state logging.
3. Implement microphone capture and write one WAV file.
4. Connect whisper.cpp transcription to print transcript.
5. Add clipboard insertion into active app.

## Definition of done for your personal V1
- Offline local dictation works end-to-end via hotkey.
- Permission onboarding is clear and repeatable.
- App is stable enough for daily use in your core apps.
> Historical planning note written before the in-process `whisper-rs` refactor. For the current implementation, use `docs/ARCHITECTURE.md` and `docs/SETUP_MODELS.md`.
