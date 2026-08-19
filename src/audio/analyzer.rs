use rustfft::num_complex::Complex;
use rustfft::FftPlanner;
use crate::audio::decoder::DecodedAudio;
use crate::audio::{AudioAnalysis, AudioFeatures};

pub fn analyze_decoded_audio(decoded: &DecodedAudio, fps: u32) -> AudioAnalysis {
    let total_samples = decoded.samples.len();
    let sample_rate = decoded.sample_rate;
    let safe_fps = fps.max(1);

    let samples_per_frame = (sample_rate as f64 / safe_fps as f64).round() as usize;
    let total_frames = if total_samples == 0 || samples_per_frame == 0 {
        0
    } else {
        (total_samples + samples_per_frame - 1) / samples_per_frame
    };

    let fft_size = 2048.min(total_samples.next_power_of_two().max(256));
    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(fft_size);

    // Precalculate Hann window
    let hann_window: Vec<f32> = (0..fft_size)
        .map(|i| 0.5 * (1.0 - (2.0 * std::f32::consts::PI * i as f32 / (fft_size as f32 - 1.0)).cos()))
        .collect();

    let mut frames = Vec::with_capacity(total_frames);

    let bin_freq = sample_rate as f32 / fft_size as f32;

    for f in 0..total_frames {
        let center_sample = f * samples_per_frame;
        let start_sample = center_sample.saturating_sub(fft_size / 2);
        let end_sample = (start_sample + fft_size).min(total_samples);

        // 1. RMS Amplitude
        let rms_slice_start = f * samples_per_frame;
        let rms_slice_end = (rms_slice_start + samples_per_frame).min(total_samples);
        let rms_slice = if rms_slice_start < total_samples {
            &decoded.samples[rms_slice_start..rms_slice_end]
        } else {
            &[]
        };

        let amplitude = if !rms_slice.is_empty() {
            let sum_sq: f32 = rms_slice.iter().map(|s| s * s).sum();
            let rms = (sum_sq / rms_slice.len() as f32).sqrt();
            // Scale and clamp
            (rms * 2.5).min(1.0)
        } else {
            0.0
        };

        // 2. FFT Spectral Analysis
        let mut buffer: Vec<Complex<f32>> = Vec::with_capacity(fft_size);
        for i in 0..fft_size {
            let sample_idx = start_sample + i;
            let sample = if sample_idx < end_sample {
                decoded.samples[sample_idx] * hann_window[i]
            } else {
                0.0
            };
            buffer.push(Complex { re: sample, im: 0.0 });
        }

        fft.process(&mut buffer);

        let half_bins = fft_size / 2;
        let mut low_energy = 0.0f32;
        let mut mid_energy = 0.0f32;
        let mut high_energy = 0.0f32;

        let mut low_count = 0.0f32;
        let mut mid_count = 0.0f32;
        let mut high_count = 0.0f32;

        for k in 1..half_bins {
            let freq = k as f32 * bin_freq;
            let mag = buffer[k].norm();

            if (20.0..=250.0).contains(&freq) {
                low_energy += mag;
                low_count += 1.0;
            } else if (250.0..4000.0).contains(&freq) {
                mid_energy += mag;
                mid_count += 1.0;
            } else if (4000.0..=20000.0).contains(&freq) {
                high_energy += mag;
                high_count += 1.0;
            }
        }

        let low = if low_count > 0.0 {
            let avg = low_energy / low_count;
            (avg * 0.15).min(1.0)
        } else {
            0.0
        };

        let mid = if mid_count > 0.0 {
            let avg = mid_energy / mid_count;
            (avg * 0.25).min(1.0)
        } else {
            0.0
        };

        let high = if high_count > 0.0 {
            let avg = high_energy / high_count;
            (avg * 0.5).min(1.0)
        } else {
            0.0
        };

        frames.push(AudioFeatures {
            amplitude,
            low,
            mid,
            high,
        });
    }

    AudioAnalysis {
        fps: safe_fps,
        duration_ms: decoded.duration_ms,
        sample_rate: decoded.sample_rate,
        channels: decoded.channels,
        frames,
    }
}
