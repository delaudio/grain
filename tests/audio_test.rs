use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use grain::audio::{self, AudioAnalysis};

fn create_test_wav(path: &Path, duration_sec: f32, sample_rate: u32, freq_hz: f32) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let num_samples = (duration_sec * sample_rate as f32) as usize;
    let mut file = File::create(path).expect("failed to create test wav file");

    let num_channels: u16 = 1;
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
    file.write_all(&16u32.to_le_bytes()).unwrap(); // Subchunk1Size (16 for PCM)
    file.write_all(&1u16.to_le_bytes()).unwrap();  // AudioFormat (1 = PCM)
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
        let sample = (t * freq_hz * 2.0 * std::f32::consts::PI).sin();
        let sample_i16 = (sample * 30000.0) as i16;
        file.write_all(&sample_i16.to_le_bytes()).unwrap();
    }
}

#[test]
fn test_decode_and_analyze_wav() {
    let fixture_path = Path::new("target/test_fixtures/sine_440hz.wav");
    create_test_wav(fixture_path, 1.0, 44100, 440.0);

    let analysis = audio::load_or_analyze(fixture_path, 60).expect("failed to analyze wav");

    assert_eq!(analysis.sample_rate, 44100);
    assert_eq!(analysis.channels, 1);
    assert_eq!(analysis.fps, 60);
    assert!(analysis.duration_ms >= 990 && analysis.duration_ms <= 1010);
    assert_eq!(analysis.total_frames(), 60);

    // Mid band should dominate for 440 Hz
    let frame_mid = analysis.get_features_at_frame(30);
    assert!(frame_mid.amplitude > 0.3);
    assert!(frame_mid.mid > 0.1);

    // Test querying by time
    let time_features = analysis.get_features_at_time(0.5);
    assert_eq!(time_features, frame_mid);
}

#[test]
fn test_analysis_determinism() {
    let fixture_path = Path::new("target/test_fixtures/determinism.wav");
    create_test_wav(fixture_path, 0.5, 44100, 100.0);

    let analysis_1 = audio::load_or_analyze(fixture_path, 60).unwrap();
    let analysis_2 = audio::load_or_analyze(fixture_path, 60).unwrap();

    assert_eq!(analysis_1, analysis_2);
}

#[test]
fn test_audio_caching() {
    let fixture_path = Path::new("target/test_fixtures/cache_test.wav");
    create_test_wav(fixture_path, 0.5, 44100, 200.0);

    let cache_path = audio::get_cache_path(fixture_path, 60).unwrap();
    if cache_path.exists() {
        let _ = fs::remove_file(&cache_path);
    }

    let analysis = audio::load_or_analyze(fixture_path, 60).unwrap();
    assert!(cache_path.exists());

    let loaded = AudioAnalysis::load_from_file(&cache_path).unwrap();
    assert_eq!(analysis, loaded);

    let _ = fs::remove_file(&cache_path);
}

#[test]
fn test_invalid_audio_path() {
    let invalid_path = Path::new("non_existent_file.wav");
    let result = audio::load_or_analyze(invalid_path, 60);
    assert!(result.is_err());
}
