use crate::{
    audio::{capture::CapturedAudio, resample},
    error::{AppError, AppResult},
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AudioLevelStats {
    pub peak_abs: f32,
    pub rms: f32,
}

impl AudioLevelStats {
    pub fn is_effectively_silent(&self) -> bool {
        self.peak_abs < 0.003 && self.rms < 0.001
    }

    pub fn is_low_input(&self) -> bool {
        self.peak_abs < 0.08 && self.rms < 0.02
    }
}

#[derive(Debug, Clone)]
pub struct PreparedAudio {
    pub samples: Vec<f32>,
    pub stats_before_normalization: AudioLevelStats,
    pub stats_after_normalization: AudioLevelStats,
    pub applied_gain: f32,
}

pub fn prepare_for_whisper_audio(captured: &CapturedAudio) -> AppResult<PreparedAudio> {
    let mono = downmix_interleaved_to_mono(&captured.samples, captured.channels)?;
    let resampled = if captured.sample_rate == 16_000 {
        mono
    } else {
        resample::resample_mono(&mono, captured.sample_rate, 16_000)?
    };

    Ok(normalize_for_transcription(resampled))
}

pub fn downmix_interleaved_to_mono(samples: &[f32], channels: u16) -> AppResult<Vec<f32>> {
    if channels == 0 {
        return Err(AppError::Message(
            "Audio capture reported zero channels".to_string(),
        ));
    }

    if channels == 1 {
        return Ok(samples.to_vec());
    }

    let channels = channels as usize;
    let mut mono = Vec::with_capacity(samples.len() / channels + 1);

    for frame in samples.chunks(channels) {
        if frame.is_empty() {
            continue;
        }

        let sum: f32 = frame.iter().copied().sum();
        mono.push((sum / frame.len() as f32).clamp(-1.0, 1.0));
    }

    Ok(mono)
}

pub fn analyze_audio_levels(samples: &[f32]) -> AudioLevelStats {
    if samples.is_empty() {
        return AudioLevelStats {
            peak_abs: 0.0,
            rms: 0.0,
        };
    }

    let mut peak_abs = 0.0f32;
    let mut sum_squares = 0.0f64;

    for sample in samples {
        let abs = sample.abs();
        if abs > peak_abs {
            peak_abs = abs;
        }
        sum_squares += f64::from(*sample) * f64::from(*sample);
    }

    AudioLevelStats {
        peak_abs,
        rms: (sum_squares / samples.len() as f64).sqrt() as f32,
    }
}

fn normalize_for_transcription(mut samples: Vec<f32>) -> PreparedAudio {
    const TARGET_PEAK_ABS: f32 = 0.35;
    const MIN_GAIN_TRIGGER_PEAK: f32 = 0.12;
    const MAX_AUTO_GAIN: f32 = 8.0;

    let stats_before_normalization = analyze_audio_levels(&samples);
    let applied_gain = if stats_before_normalization.is_effectively_silent()
        || stats_before_normalization.peak_abs >= MIN_GAIN_TRIGGER_PEAK
    {
        1.0
    } else {
        (TARGET_PEAK_ABS / stats_before_normalization.peak_abs).clamp(1.0, MAX_AUTO_GAIN)
    };

    if applied_gain > 1.0 {
        for sample in &mut samples {
            *sample = (*sample * applied_gain).clamp(-1.0, 1.0);
        }
    }

    let stats_after_normalization = analyze_audio_levels(&samples);

    PreparedAudio {
        samples,
        stats_before_normalization,
        stats_after_normalization,
        applied_gain,
    }
}

#[cfg(test)]
mod tests {
    use super::{analyze_audio_levels, downmix_interleaved_to_mono, prepare_for_whisper_audio};
    use crate::audio::capture::CapturedAudio;

    #[test]
    fn keeps_mono_audio_unchanged() {
        let input = vec![0.1, -0.2, 0.3];
        let mono = downmix_interleaved_to_mono(&input, 1).expect("mono audio should pass through");
        assert_eq!(mono, input);
    }

    #[test]
    fn downmixes_stereo_audio() {
        let input = vec![0.5, -0.5, 0.25, 0.75];
        let mono = downmix_interleaved_to_mono(&input, 2).expect("stereo downmix should work");
        assert_eq!(mono, vec![0.0, 0.5]);
    }

    #[test]
    fn analyzes_audio_levels() {
        let stats = analyze_audio_levels(&[0.25, -0.5, 0.0]);
        assert!((stats.peak_abs - 0.5).abs() < 0.0001);
        assert!(stats.rms > 0.322 && stats.rms < 0.323);
    }

    #[test]
    fn normalizes_weak_audio_for_transcription() {
        let captured = CapturedAudio {
            samples: vec![0.02, -0.03, 0.04, -0.02],
            sample_rate: 16_000,
            channels: 1,
            device_name: "Test Mic".to_string(),
            paste_target_bundle_id: None,
        };

        let prepared = prepare_for_whisper_audio(&captured).expect("audio prep should succeed");
        assert!(prepared.applied_gain > 1.0);
        assert!(
            prepared.stats_after_normalization.peak_abs
                > prepared.stats_before_normalization.peak_abs
        );
        assert!(prepared.stats_after_normalization.peak_abs <= 0.35);
    }

    #[test]
    fn keeps_silent_audio_unchanged() {
        let captured = CapturedAudio {
            samples: vec![0.0; 32],
            sample_rate: 16_000,
            channels: 1,
            device_name: "Test Mic".to_string(),
            paste_target_bundle_id: None,
        };

        let prepared = prepare_for_whisper_audio(&captured).expect("audio prep should succeed");
        assert_eq!(prepared.applied_gain, 1.0);
        assert_eq!(
            prepared.stats_before_normalization,
            prepared.stats_after_normalization
        );
    }
}
