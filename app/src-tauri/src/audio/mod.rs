pub mod capture;
pub mod convert;
pub mod resample;

use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use crate::error::{AppError, AppResult};

pub use capture::{list_input_devices, AudioDeviceInfo, CapturedAudio, RecordingSession};
pub use convert::{prepare_for_whisper_audio, AudioLevelStats};

pub fn write_debug_wav(
    samples: &[f32],
    sample_rate: u32,
    file_stem_prefix: &str,
) -> AppResult<PathBuf> {
    let output_dir = std::env::temp_dir().join("walkie-talkie");
    fs::create_dir_all(&output_dir).map_err(|error| {
        AppError::Message(format!(
            "Failed to create debug audio directory {}: {error}",
            output_dir.display()
        ))
    })?;

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let output_path = output_dir.join(format!("{}-{}.wav", file_stem_prefix, timestamp));

    let mut writer = hound::WavWriter::create(
        &output_path,
        hound::WavSpec {
            channels: 1,
            sample_rate,
            bits_per_sample: 16,
            sample_format: hound::SampleFormat::Int,
        },
    )
    .map_err(|error| {
        AppError::Message(format!(
            "Failed to create debug WAV {}: {error}",
            output_path.display()
        ))
    })?;

    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let pcm = (clamped * i16::MAX as f32) as i16;
        writer.write_sample(pcm).map_err(|error| {
            AppError::Message(format!("Failed to write debug WAV sample: {error}"))
        })?;
    }

    writer
        .finalize()
        .map_err(|error| AppError::Message(format!("Failed to finalize debug WAV: {error}")))?;

    Ok(output_path)
}
