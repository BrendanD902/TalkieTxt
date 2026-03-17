# Local Voice Typing App PRD

## Product goal
Enable users to press one global hotkey, speak naturally, and insert accurate text into any macOS app with low latency and no required cloud services.

## Non-goals (initial)
- Real-time streaming subtitles.
- Multi-speaker diarization.
- Cloud account features, sync, or analytics backend.
- Full voice command automation beyond text insertion.

## Target users
- Developers who need quick dictation in code and chat tools.
- Knowledge workers writing email, docs, and notes.
- Users who require offline processing for privacy.

## Core user jobs
- Start/stop dictation from anywhere using one hotkey.
- See clear status feedback while recording/transcribing.
- Insert transcript reliably at current cursor focus.
- Choose output formatting mode for context (Code, Email, Notes).

## Platform and constraints
- Primary platform: macOS (Intel first).
- Runtime: Tauri 2 + Rust.
- STT: whisper.cpp local models.
- Optional cleanup: local llama.cpp model.
- Offline-first: default operation without network calls.

## User flows

### Flow A: First run onboarding
1. User launches app.
2. App shows setup checklist for Microphone and Accessibility/Input Monitoring (if needed for paste/typing).
3. User completes permissions and confirms hotkey.
4. App shows status as Ready.

### Flow B: Standard dictation
1. User presses global hotkey to start recording.
2. Overlay switches to Listening and shows timer/waveform.
3. User presses hotkey again (or silence timeout triggers stop).
4. Overlay switches to Transcribing.
5. Transcript returns and app inserts text at cursor.
6. Overlay shows brief Pasted confirmation.

### Flow C: Recovery path
1. Model missing, permission denied, or insertion fails.
2. App shows error with one clear action (Open Settings, Retry, Choose Model).
3. User retries without restarting app.

## Functional requirements
- Global hotkey configurable by user.
- Menu bar app with persistent status.
- Floating overlay with four states: Idle, Listening, Transcribing, Pasted.
- Local microphone recording and segmentation.
- whisper.cpp transcription with selectable models.
- Insertion method: clipboard paste and direct typing fallback.
- Settings for hotkey, mic, mode, model, cleanup toggle, privacy/offline indicator.
- Optional local cleanup pass with mode-specific formatting.

## Non-functional requirements
- Usable on Intel MacBook hardware without GPU dependency.
- Fast perceived response with explicit progress states.
- No required network for core dictation path.
- Deterministic cancel/retry behavior under repeated hotkey toggles.

## Privacy and offline stance
- Process audio locally by default.
- Make any network action opt-in and visible.
- Show an explicit "No network requests" status/toggle.
- Keep transcript handling local unless user explicitly exports data.

## Acceptance criteria by phase

### Phase 1 (MVP)
- Given app is running, when user presses hotkey, recording starts.
- Given recording stops, when whisper.cpp model is available, transcript is produced locally.
- Given transcript exists and cursor is focused, text is inserted successfully.
- Given model is missing, user sees clear remediation.

### Phase 2 (UX polish)
- Menu bar controls are available at all times.
- Overlay state transitions are visible and correct.
- Settings persist across restarts.
- Permission onboarding can recover from denied permissions.

### Phase 3 (advanced quality)
- Cleanup toggle can be enabled/disabled per preference.
- Modes (Auto, Code, Email, Notes) alter formatting predictably.
- Per-app mode rule can auto-apply Code mode for Cursor.

### Phase 4 (Intel performance)
- Quantized model selection is available.
- Model warm-load reduces first transcription delay.
- Silence detection tuning reduces clipped starts/ends.
- Performance regressions are caught by repeatable checks.

## Edge cases
- Hotkey already registered by another app.
- Microphone unavailable or revoked mid-session.
- Accessibility permission missing for insertion method.
- User switches focused app during transcription.
- Empty/near-silent input returns no text.
- Repeated start/stop quickly causes stale state race.

## Success metrics
- End-to-end success rate for dictation insertion.
- Median time from stop-recording to text insertion.
- Permission-onboarding completion rate on clean install.
- Crash-free sessions for repeated dictation loops.
> Historical product note written before the in-process `whisper-rs` refactor. For the current implementation, use `docs/ARCHITECTURE.md`, `docs/SETUP_MODELS.md`, and `app/README.md`.
