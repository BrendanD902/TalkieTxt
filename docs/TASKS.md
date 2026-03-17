# Voice Typing Build Tasks (Phased)

## Sequencing rule
Complete planning artifacts before implementation.
Order of work: PRD -> UX -> ARCH -> implementation phases.

> Historical planning note written before the in-process `whisper-rs` refactor. For the current implementation, use `docs/ARCHITECTURE.md` and `docs/SETUP_MODELS.md`.

## Phase 1: MVP (Hotkey -> Record -> Transcribe -> Paste)

### Goals
- Deliver end-to-end local dictation with one hotkey.

### Tasks
- Initialize Tauri 2 app shell and tray baseline.
- Implement global hotkey register/start-stop flow.
- Implement microphone capture service.
- Integrate whisper.cpp local transcription path.
- Implement insertion via clipboard paste fallback path.
- Add minimal settings for hotkey, mic, and model path.

### Definition of done
- Dictation works end-to-end in at least one external app.
- Missing model and denied permission states are recoverable.

## Phase 2: UX polish (Professional experience)

### Goals
- Make status, setup, and controls production-usable.

### Tasks
- Build overlay with Idle/Listening/Transcribing/Pasted states.
- Finalize menu bar interactions and quick actions.
- Build settings sections for hotkey, mic, model, paste method.
- Add first-run onboarding checklist for permissions.
- Persist settings and restore on launch.

### Definition of done
- First-run setup succeeds on clean macOS profile.
- Overlay transitions are stable and understandable.

## Phase 3: Advanced quality (Wispr-like behaviors)

### Goals
- Improve output quality and context control.

### Tasks
- Add optional local cleanup pipeline using llama.cpp.
- Add output modes: Auto, Code, Email, Notes.
- Add per-app mode rule engine (default Cursor -> Code).
- Add non-destructive preview/fallback to original transcript.

### Definition of done
- Cleanup can be toggled safely.
- Mode behavior is measurable and predictable.

## Phase 4: Intel performance and hardening

### Goals
- Optimize latency and reliability on Intel Macs.

### Tasks
- Add quantized model support and speed/quality labels.
- Implement model warm-load and cache strategy.
- Tune silence detection thresholds with fixtures.
- Add repeatable benchmarks for stop-to-insert latency.
- Run regression suite across multiple target apps.

### Definition of done
- Performance targets meet agreed thresholds on Intel hardware.
- No major regressions across dictation loops and permission states.

## QA and release checklist
- Verify microphone and accessibility permission flows.
- Verify insertion in editor, browser, and notes apps.
- Verify offline behavior with network disabled.
- Verify packaging/signing/notarization prerequisites.
- Publish rollback plan and known issues for first release.
