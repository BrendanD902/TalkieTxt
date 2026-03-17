use std::{
    sync::{mpsc, Arc, Mutex},
    thread,
    time::Duration,
};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use serde::Serialize;
use tracing::{error, warn};

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceInfo {
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone)]
pub struct CaptureStartInfo {
    pub device_name: String,
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone)]
pub struct CapturedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub device_name: String,
    pub paste_target_bundle_id: Option<String>,
}

struct CaptureReady {
    device_name: String,
    sample_rate: u32,
    channels: u16,
}

pub struct RecordingSession {
    stop_tx: mpsc::Sender<()>,
    join_handle: thread::JoinHandle<AppResult<CapturedAudio>>,
    start_info: CaptureStartInfo,
    paste_target_bundle_id: Option<String>,
}

impl RecordingSession {
    pub fn start_info(&self) -> &CaptureStartInfo {
        &self.start_info
    }

    pub fn stop(self) -> AppResult<CapturedAudio> {
        let _ = self.stop_tx.send(());
        let mut captured = self
            .join_handle
            .join()
            .map_err(|_| AppError::InvalidState("Audio capture thread panicked".to_string()))??;
        captured.paste_target_bundle_id = self.paste_target_bundle_id;
        Ok(captured)
    }
}

pub fn list_input_devices() -> AppResult<Vec<AudioDeviceInfo>> {
    let host = cpal::default_host();
    let default_name = host
        .default_input_device()
        .and_then(|device| device.name().ok());

    let devices = host.input_devices().map_err(|error| {
        AppError::AudioStream(format!("Failed to enumerate audio devices: {error}"))
    })?;

    let mut result = Vec::new();
    for device in devices {
        let name = device
            .name()
            .unwrap_or_else(|_| "Unknown input device".to_string());
        let is_default = default_name.as_deref() == Some(name.as_str());
        result.push(AudioDeviceInfo { name, is_default });
    }

    Ok(result)
}

pub fn start_recording(
    preferred_device_name: Option<&str>,
    paste_target_bundle_id: Option<String>,
) -> AppResult<RecordingSession> {
    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let (ready_tx, ready_rx) = mpsc::channel::<AppResult<CaptureReady>>();
    let sample_buffer = Arc::new(Mutex::new(Vec::new()));
    let capture_buffer = Arc::clone(&sample_buffer);
    let preferred_device_name = preferred_device_name.map(str::to_string);

    let join_handle = thread::spawn(move || {
        run_capture_until_stopped(
            preferred_device_name.as_deref(),
            stop_rx,
            ready_tx,
            capture_buffer,
        )
    });

    match ready_rx.recv_timeout(Duration::from_secs(5)) {
        Ok(Ok(ready)) => Ok(RecordingSession {
            stop_tx,
            join_handle,
            start_info: CaptureStartInfo {
                device_name: ready.device_name,
                sample_rate: ready.sample_rate,
                channels: ready.channels,
            },
            paste_target_bundle_id,
        }),
        Ok(Err(error)) => {
            let _ = stop_tx.send(());
            let _ = join_handle.join();
            Err(error)
        }
        Err(_) => {
            let _ = stop_tx.send(());
            let _ = join_handle.join();
            Err(AppError::AudioStream(
                "Microphone device did not initialize in time".to_string(),
            ))
        }
    }
}

fn run_capture_until_stopped(
    preferred_device_name: Option<&str>,
    stop_rx: mpsc::Receiver<()>,
    ready_tx: mpsc::Sender<AppResult<CaptureReady>>,
    sample_buffer: Arc<Mutex<Vec<f32>>>,
) -> AppResult<CapturedAudio> {
    let host = cpal::default_host();
    let device = resolve_input_device(&host, preferred_device_name)?;
    let device_name = device
        .name()
        .unwrap_or_else(|_| "Unknown input device".to_string());

    let config = device
        .default_input_config()
        .map_err(|error| AppError::AudioStream(format!("Failed to read input config: {error}")))?;

    let sample_rate = config.sample_rate().0;
    let channels = config.channels();
    let stream_config = config.config();

    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => build_input_stream::<f32, _>(
            &device,
            &stream_config,
            Arc::clone(&sample_buffer),
            |sample| sample,
        )?,
        cpal::SampleFormat::I16 => build_input_stream::<i16, _>(
            &device,
            &stream_config,
            Arc::clone(&sample_buffer),
            |sample| sample as f32 / i16::MAX as f32,
        )?,
        cpal::SampleFormat::U16 => build_input_stream::<u16, _>(
            &device,
            &stream_config,
            Arc::clone(&sample_buffer),
            |sample| {
                let normalized = sample as f32 / u16::MAX as f32;
                (normalized * 2.0) - 1.0
            },
        )?,
        sample_format => {
            return Err(AppError::UnsupportedSampleFormat(format!(
                "{sample_format:?}"
            )));
        }
    };

    stream
        .play()
        .map_err(|error| AppError::AudioStream(format!("Failed to start input stream: {error}")))?;

    let _ = ready_tx.send(Ok(CaptureReady {
        device_name: device_name.clone(),
        sample_rate,
        channels,
    }));

    let _ = stop_rx.recv();
    drop(stream);

    let samples = sample_buffer
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();

    if samples.is_empty() {
        return Err(AppError::AudioStream(
            "No audio was captured. Check microphone permissions and input level.".to_string(),
        ));
    }

    Ok(CapturedAudio {
        samples,
        sample_rate,
        channels,
        device_name,
        paste_target_bundle_id: None,
    })
}

fn resolve_input_device(
    host: &cpal::Host,
    preferred_device_name: Option<&str>,
) -> AppResult<cpal::Device> {
    if let Some(preferred_name) = preferred_device_name {
        match host.input_devices() {
            Ok(devices) => {
                for device in devices {
                    let device_name = device.name().unwrap_or_default();
                    if device_name == preferred_name {
                        return Ok(device);
                    }
                }
                warn!(
                    preferred_device_name = preferred_name,
                    "Preferred input device not found, falling back to default"
                );
            }
            Err(error) => {
                warn!(
                    "Failed to enumerate input devices while resolving preferred device: {error}"
                );
            }
        }
    }

    host.default_input_device().ok_or(AppError::NoInputDevice)
}

fn build_input_stream<T, F>(
    device: &cpal::Device,
    stream_config: &cpal::StreamConfig,
    sample_buffer: Arc<Mutex<Vec<f32>>>,
    convert: F,
) -> AppResult<cpal::Stream>
where
    T: cpal::SizedSample,
    F: Fn(T) -> f32 + Send + 'static + Copy,
{
    device
        .build_input_stream(
            stream_config,
            move |data: &[T], _| {
                let mut guard = sample_buffer
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner);
                guard.extend(data.iter().copied().map(convert));
            },
            on_audio_error,
            None,
        )
        .map_err(|error| AppError::AudioStream(format!("Failed to build input stream: {error}")))
}

fn on_audio_error(error: cpal::StreamError) {
    error!("Audio input stream error: {error}");
}
