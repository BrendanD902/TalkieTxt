use audioadapter_buffers::direct::InterleavedSlice;
use rubato::{Fft, FixedSync, Resampler};

use crate::error::{AppError, AppResult};

pub fn resample_mono(
    samples: &[f32],
    input_sample_rate: u32,
    output_sample_rate: u32,
) -> AppResult<Vec<f32>> {
    if input_sample_rate == output_sample_rate {
        return Ok(samples.to_vec());
    }

    let chunk_size = 1024usize.min(samples.len().max(32));
    let mut resampler = Fft::<f32>::new(
        input_sample_rate as usize,
        output_sample_rate as usize,
        chunk_size,
        1,
        1,
        FixedSync::Both,
    )
    .map_err(|error| AppError::Message(format!("Failed to create resampler: {error}")))?;

    let input_adapter = InterleavedSlice::new(samples, 1, samples.len()).map_err(|error| {
        AppError::Message(format!("Failed to prepare resampler input: {error}"))
    })?;
    let needed_len = resampler.process_all_needed_output_len(samples.len());
    let mut output = vec![0.0f32; needed_len];
    let mut output_adapter =
        InterleavedSlice::new_mut(&mut output, 1, needed_len).map_err(|error| {
            AppError::Message(format!("Failed to prepare resampler output: {error}"))
        })?;

    let (_, output_frames) = resampler
        .process_all_into_buffer(&input_adapter, &mut output_adapter, samples.len(), None)
        .map_err(|error| AppError::Message(format!("Failed to resample audio: {error}")))?;

    output.truncate(output_frames);
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::resample_mono;

    #[test]
    fn resamples_48k_to_16k() {
        let input = vec![0.25f32; 48_000];
        let output = resample_mono(&input, 48_000, 16_000).expect("48k resample should work");
        assert!((15_500..=16_500).contains(&output.len()));
    }

    #[test]
    fn resamples_44k_to_16k() {
        let input = vec![0.25f32; 44_100];
        let output = resample_mono(&input, 44_100, 16_000).expect("44.1k resample should work");
        assert!((15_500..=16_500).contains(&output.len()));
    }
}
