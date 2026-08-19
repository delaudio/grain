use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

pub fn generate_fixture_wav(path: &Path, duration_sec: f32) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let sample_rate: u32 = 44100;
    let num_samples = (duration_sec * sample_rate as f32) as usize;
    let mut file = File::create(path).expect("failed to create fixture wav file");

    let num_channels: u16 = 2;
    let bits_per_sample: u16 = 16;
    let byte_rate = sample_rate * num_channels as u32 * (bits_per_sample as u32 / 8);
    let block_align = num_channels * (bits_per_sample / 8);
    let data_len = num_samples as u32 * block_align as u32;
    let riff_chunk_size = 36 + data_len;

    // RIFF header
    file.write_all(b"RIFF").unwrap();
    file.write_all(&riff_chunk_size.to_le_bytes()).unwrap();
    file.write_all(b"WAVE").unwrap();

    // fmt subchunk
    file.write_all(b"fmt ").unwrap();
    file.write_all(&16u32.to_le_bytes()).unwrap();
    file.write_all(&1u16.to_le_bytes()).unwrap(); // PCM
    file.write_all(&num_channels.to_le_bytes()).unwrap();
    file.write_all(&sample_rate.to_le_bytes()).unwrap();
    file.write_all(&byte_rate.to_le_bytes()).unwrap();
    file.write_all(&block_align.to_le_bytes()).unwrap();
    file.write_all(&bits_per_sample.to_le_bytes()).unwrap();

    // data subchunk
    file.write_all(b"data").unwrap();
    file.write_all(&data_len.to_le_bytes()).unwrap();

    for i in 0..num_samples {
        let t = i as f32 / sample_rate as f32;
        // Bass kick pulses at 2 Hz
        let kick = (t * 2.0 * std::f32::consts::PI * 2.0).sin().powi(4) * (t * 60.0 * 2.0 * std::f32::consts::PI).sin();
        // Mid harmonic synth wave at 440 Hz
        let mid = (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.4;
        // Treble hi-hat sizzle
        let hi = (t * 8000.0 * 2.0 * std::f32::consts::PI).sin() * 0.15;

        let sample = (kick * 0.6 + mid + hi) * 0.7;
        let sample_i16 = (sample.clamp(-1.0, 1.0) * 30000.0) as i16;

        // Stereo (Left & Right)
        file.write_all(&sample_i16.to_le_bytes()).unwrap();
        file.write_all(&sample_i16.to_le_bytes()).unwrap();
    }
}

fn main() {
    generate_fixture_wav(Path::new("fixtures/demo.wav"), 2.5);
    println!("Generated fixtures/demo.wav successfully");
}
