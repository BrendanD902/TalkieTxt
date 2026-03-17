import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

type DictationState = "idle" | "listening" | "processing" | "error";
type ModelStatus = "loading" | "ready" | "missing" | "failed";
type HotkeyMode = "toggle" | "holdToTalk";
type DefaultModelPreset = "tinyEn" | "baseEn";
type LogLevel = "error" | "warn" | "info" | "debug" | "trace";

type StatusPayload = {
  state: DictationState;
  source: string;
  modelStatus: ModelStatus;
  modelMessage: string | null;
  hotkeyHint: string;
  hotkeyMode: HotkeyMode;
  modelPath: string | null;
  defaultModelPreset: DefaultModelPreset;
  pasteAfterTranscribe: boolean;
  preferredInputDevice: string | null;
  currentInputDevice: string | null;
  lastCaptureSampleRate: number | null;
  lastCaptureChannels: number | null;
};

type AppSettings = {
  modelPath: string | null;
  defaultModelPreset: DefaultModelPreset;
  pasteAfterTranscribe: boolean;
  saveDebugWav: boolean;
  logLevel: LogLevel;
  preferredInputDevice: string | null;
  hotkeyMode: HotkeyMode;
  hasCompletedOnboarding: boolean;
};

type AudioDeviceInfo = {
  name: string;
  isDefault: boolean;
};

type TranscriptPayload = {
  text: string;
  engine: string;
};

type InsertResultPayload = {
  status: "pasted" | "clipboardOnly" | "skipped";
  message: string;
};

type ErrorPayload = {
  message: string;
  source: string;
};

const stateLabel = document.querySelector<HTMLElement>("#state-label");
const stateSource = document.querySelector<HTMLElement>("#state-source");
const modelStatusLabel = document.querySelector<HTMLElement>("#model-status");
const modelMessage = document.querySelector<HTMLElement>("#model-message");
const stateChip = document.querySelector<HTMLElement>("#state-chip");
const modelChip = document.querySelector<HTMLElement>("#model-chip");
const hotkeyHint = document.querySelector<HTMLElement>("#hotkey-hint");
const hotkeyActionCopy = document.querySelector<HTMLElement>("#hotkey-action-copy");
const deviceHint = document.querySelector<HTMLElement>("#device-hint");
const captureMeta = document.querySelector<HTMLElement>("#capture-meta");
const toggleButton = document.querySelector<HTMLButtonElement>("#toggle-record");
const resetButton = document.querySelector<HTMLButtonElement>("#reset-state");
const transcriptBody = document.querySelector<HTMLElement>("#transcript-body");
const transcriptEngine = document.querySelector<HTMLElement>("#transcript-engine");
const insertStatus = document.querySelector<HTMLElement>("#insert-status");
const insertMessage = document.querySelector<HTMLElement>("#insert-message");
const errorBanner = document.querySelector<HTMLElement>("#error-banner");

const modelPathInput = document.querySelector<HTMLInputElement>("#model-path");
const presetSelect = document.querySelector<HTMLSelectElement>("#model-preset");
const hotkeyModeSelect = document.querySelector<HTMLSelectElement>("#hotkey-mode");
const pasteCheckbox = document.querySelector<HTMLInputElement>("#paste-after-transcribe");
const debugWavCheckbox = document.querySelector<HTMLInputElement>("#save-debug-wav");
const logLevelSelect = document.querySelector<HTMLSelectElement>("#log-level");
const inputDeviceSelect = document.querySelector<HTMLSelectElement>("#preferred-input-device");
const browseModelButton = document.querySelector<HTMLButtonElement>("#browse-model");
const refreshDevicesButton = document.querySelector<HTMLButtonElement>("#refresh-devices");
const saveSettingsButton = document.querySelector<HTMLButtonElement>("#save-settings");
const saveFeedback = document.querySelector<HTMLElement>("#save-feedback");

const onboarding = document.querySelector<HTMLElement>("#onboarding");
const onboardingModelHeading = document.querySelector<HTMLElement>("#onboarding-model-heading");
const onboardingModelCopy = document.querySelector<HTMLElement>("#onboarding-model-copy");
const onboardingModelChip = document.querySelector<HTMLElement>("#onboarding-model-chip");
const onboardingModelMessage = document.querySelector<HTMLElement>("#onboarding-model-message");
const onboardingHotkey = document.querySelector<HTMLElement>("#onboarding-hotkey");
const onboardingModeCopy = document.querySelector<HTMLElement>("#onboarding-mode-copy");
const onboardingReadyMessage = document.querySelector<HTMLElement>("#onboarding-ready-message");
const onboardingBrowseModelButton =
  document.querySelector<HTMLButtonElement>("#onboarding-browse-model");
const onboardingSaveModelButton =
  document.querySelector<HTMLButtonElement>("#onboarding-save-model");
const openMicSettingsButton =
  document.querySelector<HTMLButtonElement>("#open-mic-settings");
const openAccessibilitySettingsButton = document.querySelector<HTMLButtonElement>(
  "#open-accessibility-settings",
);
const finishOnboardingButton =
  document.querySelector<HTMLButtonElement>("#finish-onboarding");
const dismissOnboardingButton =
  document.querySelector<HTMLButtonElement>("#dismiss-onboarding");
const quickstartStepOne = document.querySelector<HTMLElement>("#quickstart-step-1");
const quickstartStepTwo = document.querySelector<HTMLElement>("#quickstart-step-2");

let currentStatus: StatusPayload | null = null;
let currentSettings: AppSettings | null = null;
let saveFeedbackTimeout: number | null = null;
let onboardingDismissedForSession = false;

function showError(message: string) {
  if (!errorBanner) return;
  errorBanner.textContent = message;
  errorBanner.hidden = false;
}

function clearError() {
  if (!errorBanner) return;
  errorBanner.textContent = "";
  errorBanner.hidden = true;
}

function setSaveFeedback(message: string) {
  if (!saveFeedback) return;
  saveFeedback.textContent = message;

  if (saveFeedbackTimeout) {
    window.clearTimeout(saveFeedbackTimeout);
  }

  saveFeedbackTimeout = window.setTimeout(() => {
    if (saveFeedback) {
      saveFeedback.textContent = "Settings are saved locally.";
    }
  }, 2500);
}

function setChipState(element: HTMLElement | null, value: string, prefix: "chip") {
  if (!element) return;
  element.textContent = value;
  element.className = `${prefix} chip-${value}`;
}

function hasModelPath() {
  return Boolean(modelPathInput?.value.trim());
}

function transcriptPlaceholder(status: StatusPayload | null) {
  if (status?.hotkeyMode === "toggle") {
    return "No speech detected yet. Press Option+Space once to start, speak, then press it again to transcribe.";
  }

  return "No speech detected yet. Hold Option+Space while speaking, then release to transcribe.";
}

function renderTranscript(payload?: TranscriptPayload) {
  const text = payload?.text.trim() ?? "";

  if (transcriptBody) {
    transcriptBody.textContent = text || transcriptPlaceholder(currentStatus);
  }

  if (transcriptEngine) {
    transcriptEngine.textContent = text
      ? `Transcribed with ${payload?.engine ?? "whisper-rs"}`
      : "No speech detected from the last recording";
  }
}

function syncHotkeyCopy(status: StatusPayload) {
  const isHoldToTalk = status.hotkeyMode === "holdToTalk";

  if (hotkeyActionCopy) {
    hotkeyActionCopy.textContent = isHoldToTalk
      ? status.state === "listening"
        ? "while you speak. Release to transcribe."
        : "and hold while you speak, then release to transcribe."
      : status.state === "listening"
        ? "again to stop and transcribe."
        : "once to start dictation, then press it again to transcribe.";
  }

  if (onboardingModeCopy) {
    onboardingModeCopy.textContent = isHoldToTalk ? "Hold to talk" : "Toggle";
  }

  if (quickstartStepOne) {
    quickstartStepOne.innerHTML = isHoldToTalk
      ? "Press and hold <strong>Option+Space</strong> to dictate."
      : "Press <strong>Option+Space</strong> once to start dictating.";
  }

  if (quickstartStepTwo) {
    quickstartStepTwo.textContent = isHoldToTalk
      ? "Release to transcribe and paste into the active app."
      : "Press Option+Space again to transcribe and paste into the active app.";
  }
}

function usesManagedModelPath(path: string | null | undefined) {
  if (!path) return false;
  return path.includes("/Application Support/com.brendandalziel.walkietalkie/models/");
}

function syncOnboarding() {
  if (!onboarding || !currentSettings || !currentStatus) return;

  const shouldShow =
    !currentSettings.hasCompletedOnboarding && !onboardingDismissedForSession;
  onboarding.hidden = !shouldShow;
  document.body.classList.toggle("onboarding-active", shouldShow);

  const managedModel = usesManagedModelPath(currentSettings.modelPath);
  if (onboardingModelHeading) {
    onboardingModelHeading.textContent = managedModel
      ? "Model found automatically"
      : "Connect your local Whisper model";
  }
  if (onboardingModelCopy) {
    onboardingModelCopy.textContent = managedModel
      ? "TalkieTxt found a compatible model in its managed models folder automatically. You can keep using it, or switch to a different file any time."
      : "TalkieTxt checks its managed models folder automatically first. If nothing is there yet, you can choose a different local GGML model below.";
  }
  setChipState(onboardingModelChip, currentStatus.modelStatus, "chip");
  if (onboardingModelMessage) {
    onboardingModelMessage.textContent =
      currentStatus.modelMessage ??
      (hasModelPath() ? "Model path connected." : "No local model is connected yet.");
  }
  if (onboardingHotkey) {
    onboardingHotkey.textContent = currentStatus.hotkeyHint;
  }
  if (onboardingReadyMessage) {
    onboardingReadyMessage.textContent =
      currentStatus.modelStatus === "ready"
        ? "Model ready. Finish setup and start dictating."
        : hasModelPath()
          ? "Model connected. Wait for the model status to become ready, then finish setup."
          : "If the app does not find a model automatically, choose one here.";
  }
  if (finishOnboardingButton) {
    finishOnboardingButton.disabled = currentStatus.modelStatus !== "ready";
  }
  if (onboardingBrowseModelButton) {
    onboardingBrowseModelButton.textContent = managedModel ? "Change model" : "Choose model";
  }
  if (onboardingSaveModelButton) {
    onboardingSaveModelButton.hidden = managedModel && currentStatus.modelStatus === "ready";
  }
}

function updateToggleButton(status: StatusPayload) {
  if (!toggleButton) return;

  if (status.state === "listening") {
    toggleButton.textContent = "Stop and transcribe";
    toggleButton.disabled = false;
    return;
  }

  if (status.state === "processing") {
    toggleButton.textContent = "Processing...";
    toggleButton.disabled = true;
    return;
  }

  toggleButton.textContent = "Start dictation";
  toggleButton.disabled = status.modelStatus !== "ready";
}

function renderStatus(status: StatusPayload) {
  currentStatus = status;
  document.body.dataset.state = status.state;

  if (stateLabel) stateLabel.textContent = status.state;
  if (stateSource) stateSource.textContent = status.source;
  if (modelStatusLabel) modelStatusLabel.textContent = status.modelStatus;
  if (modelMessage) modelMessage.textContent = status.modelMessage ?? "No model message";
  setChipState(stateChip, status.state, "chip");
  setChipState(modelChip, status.modelStatus, "chip");
  if (hotkeyHint) hotkeyHint.textContent = status.hotkeyHint;
  syncHotkeyCopy(status);
  if (deviceHint) {
    deviceHint.textContent =
      status.currentInputDevice ?? status.preferredInputDevice ?? "Default microphone";
  }
  if (captureMeta) {
    captureMeta.textContent =
      status.lastCaptureSampleRate && status.lastCaptureChannels
        ? `${status.lastCaptureSampleRate} Hz • ${status.lastCaptureChannels} ch`
        : "No capture yet";
  }

  if (status.state !== "error") {
    clearError();
  }

  updateToggleButton(status);
  syncOnboarding();
}

function applySettings(settings: AppSettings) {
  currentSettings = settings;
  if (modelPathInput) modelPathInput.value = settings.modelPath ?? "";
  if (presetSelect) presetSelect.value = settings.defaultModelPreset;
  if (hotkeyModeSelect) hotkeyModeSelect.value = settings.hotkeyMode;
  if (pasteCheckbox) pasteCheckbox.checked = settings.pasteAfterTranscribe;
  if (debugWavCheckbox) debugWavCheckbox.checked = settings.saveDebugWav;
  if (logLevelSelect) logLevelSelect.value = settings.logLevel;
  if (inputDeviceSelect) inputDeviceSelect.value = settings.preferredInputDevice ?? "";
  syncOnboarding();
}

function populateInputDevices(devices: AudioDeviceInfo[]) {
  if (!inputDeviceSelect) return;

  const currentValue = inputDeviceSelect.value;
  inputDeviceSelect.innerHTML = "";

  const defaultOption = document.createElement("option");
  defaultOption.value = "";
  defaultOption.textContent = "Default microphone";
  inputDeviceSelect.append(defaultOption);

  for (const device of devices) {
    const option = document.createElement("option");
    option.value = device.name;
    option.textContent = device.isDefault ? `${device.name} (Default)` : device.name;
    inputDeviceSelect.append(option);
  }

  inputDeviceSelect.value = currentValue;
}

async function refreshDevices() {
  try {
    const devices = await invoke<AudioDeviceInfo[]>("list_audio_devices");
    populateInputDevices(devices);
    if (currentStatus?.preferredInputDevice && inputDeviceSelect) {
      inputDeviceSelect.value = currentStatus.preferredInputDevice;
    }
  } catch (error) {
    showError(String(error));
  }
}

async function bootstrap() {
  const [status, settings, devices] = await Promise.all([
    invoke<StatusPayload>("get_status"),
    invoke<AppSettings>("get_settings"),
    invoke<AudioDeviceInfo[]>("list_audio_devices"),
  ]);

  renderStatus(status);
  populateInputDevices(devices);
  applySettings(settings);
}

function buildSettingsPayload(overrides?: Partial<AppSettings>): AppSettings {
  return {
    modelPath: modelPathInput?.value.trim() || null,
    defaultModelPreset: (presetSelect?.value as DefaultModelPreset) ?? "tinyEn",
    pasteAfterTranscribe: pasteCheckbox?.checked ?? true,
    saveDebugWav: debugWavCheckbox?.checked ?? false,
    logLevel: (logLevelSelect?.value as LogLevel) ?? "info",
    preferredInputDevice: inputDeviceSelect?.value || null,
    hotkeyMode: (hotkeyModeSelect?.value as HotkeyMode) ?? "holdToTalk",
    hasCompletedOnboarding: currentSettings?.hasCompletedOnboarding ?? false,
    ...overrides,
  };
}

async function saveSettings(overrides?: Partial<AppSettings>) {
  try {
    const saved = await invoke<AppSettings>("save_settings", {
      settings: buildSettingsPayload(overrides),
    });
    applySettings(saved);
    setSaveFeedback("Settings saved. Model state will refresh automatically.");
  } catch (error) {
    showError(String(error));
  }
}

async function finishOnboarding() {
  if (currentStatus?.modelStatus !== "ready") {
    showError("Finish setup after the model status becomes ready.");
    return;
  }

  onboardingDismissedForSession = false;
  await saveSettings({ hasCompletedOnboarding: true });
  setSaveFeedback("Setup complete. You can now close the window and keep dictating from the tray.");
}

async function toggleRecording() {
  if (!currentStatus) return;

  try {
    if (currentStatus.state === "listening") {
      await invoke("stop_recording");
      return;
    }

    await invoke("start_recording");
  } catch (error) {
    showError(String(error));
  }
}

async function pickModel() {
  const selected = await open({
    multiple: false,
    directory: false,
    filters: [{ name: "Whisper model", extensions: ["bin"] }],
  });

  if (typeof selected === "string" && modelPathInput) {
    modelPathInput.value = selected;
    syncOnboarding();
  }
}

async function openMacPreferencePane(kind: "microphone" | "accessibility") {
  try {
    await invoke("open_macos_preference_pane", { kind });
  } catch (error) {
    showError(String(error));
  }
}

window.addEventListener("DOMContentLoaded", async () => {
  await bootstrap();
  renderTranscript();

  await listen<StatusPayload>("dictation://status", ({ payload }) => {
    renderStatus(payload);
  });

  await listen<TranscriptPayload>("dictation://transcript", ({ payload }) => {
    renderTranscript(payload);
  });

  await listen<InsertResultPayload>("dictation://insert-result", ({ payload }) => {
    if (insertStatus) insertStatus.textContent = payload.status;
    if (insertMessage) insertMessage.textContent = payload.message;
  });

  await listen<ErrorPayload>("dictation://error", ({ payload }) => {
    showError(`${payload.message} [${payload.source}]`);
  });

  toggleButton?.addEventListener("click", () => {
    void toggleRecording();
  });

  resetButton?.addEventListener("click", async () => {
    try {
      await invoke("reset_dictation_state");
      clearError();
    } catch (error) {
      showError(String(error));
    }
  });

  browseModelButton?.addEventListener("click", () => {
    void pickModel();
  });

  onboardingBrowseModelButton?.addEventListener("click", () => {
    void pickModel();
  });

  onboardingSaveModelButton?.addEventListener("click", () => {
    void saveSettings();
  });

  refreshDevicesButton?.addEventListener("click", () => {
    void refreshDevices();
  });

  saveSettingsButton?.addEventListener("click", () => {
    void saveSettings();
  });

  openMicSettingsButton?.addEventListener("click", () => {
    void openMacPreferencePane("microphone");
  });

  openAccessibilitySettingsButton?.addEventListener("click", () => {
    void openMacPreferencePane("accessibility");
  });

  finishOnboardingButton?.addEventListener("click", () => {
    void finishOnboarding();
  });

  dismissOnboardingButton?.addEventListener("click", () => {
    onboardingDismissedForSession = true;
    syncOnboarding();
  });
});
