# Voice Typing UX Specification

## UX principles
- Stay available everywhere with minimal visual intrusion.
- Always show current state clearly.
- Optimize for keyboard-only workflow.
- Make failure recovery one-step whenever possible.
- Keep privacy and offline behavior explicit.

## Primary surfaces

### 1) Menu bar app
- Show current status icon (Ready, Listening, Busy, Error).
- Provide quick actions: Start/Stop, Settings, Quit.
- Show selected mode and model in compact form.

### 2) Floating overlay (near cursor)
- Small panel anchored near active input area when possible.
- Auto-hide quickly after completion.
- Never steal keyboard focus.

### 3) Settings window
- Single-column, minimal controls grouped by task.
- Immediate save with visible confirmation.

### 4) First-run onboarding
- Checklist UI with direct links to macOS settings pages.
- Re-check permission status after user returns.

## Overlay states and copy

### Idle
- Label: `Ready to Dictate`
- Hint: `Press {Hotkey} to start`

### Listening
- Label: `Listening...`
- Secondary: live timer `00:12`
- Visual: simple waveform or level meter

### Transcribing
- Label: `Transcribing...`
- Secondary action: `Cancel`

### Pasted
- Label: `Inserted`
- Duration: ~900ms then fade out

## State transitions
- Idle -> Listening: hotkey press when app ready.
- Listening -> Transcribing: hotkey press, silence timeout, or max duration reached.
- Transcribing -> Pasted: successful insertion.
- Transcribing -> Error: model or insertion failure.
- Any state -> Idle: cancel or completion timeout.

## Settings information architecture

### Input
- Hotkey picker.
- Microphone device selector.

### Output
- Mode selector: Auto, Code, Email, Notes.
- Paste method: Clipboard Paste, Direct Typing.

### Models
- STT model selector with speed/quality labels.
- Model location and download status.

### Optional cleanup
- Cleanup toggle (local LLM).
- Cleanup model selector if enabled.

### Privacy
- `No network requests` toggle/status.
- Last network activity indicator (should remain none in offline mode).

## Onboarding checklist
- Step 1: Grant Microphone access.
- Step 2: Grant Accessibility/Input Monitoring (if paste method requires it).
- Step 3: Confirm hotkey is available.
- Step 4: Run 5-second test dictation.

## Error UX
- Missing model: `Model not found` + `Open Model Manager`.
- Permission denied: `Permission required` + `Open macOS Settings`.
- Insert failed: `Could not insert text` + `Copy to Clipboard` fallback.
- Busy/race condition: `Still processing previous clip` + `Cancel and Retry`.

## Per-app mode behavior
- Default mode: Auto.
- Allow per-app override rules.
- Preconfigure Cursor as Code mode when recognized.

## Accessibility and motion
- Keep sufficient contrast and readable type at small sizes.
- Respect reduced motion preference by minimizing animation.
- Ensure all settings are keyboard navigable.
