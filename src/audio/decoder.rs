use std::fs::File;
use std::path::Path;
use anyhow::{bail, Context, Result};
use symphonia::core::codecs::audio::{AudioDecoderOptions, CODEC_ID_NULL_AUDIO};
use symphonia::core::codecs::CodecParameters;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;

#[derive(Debug, Clone)]
pub struct DecodedAudio {
    /// Normalized mono audio samples (-1.0 .. 1.0)
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub channels: u16,
    pub duration_ms: u64,
}

pub fn decode_audio_file(path: &Path) -> Result<DecodedAudio> {
    let file = File::open(path)
        .with_context(|| format!("Failed to open audio file: {}", path.display()))?;

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        hint.with_extension(ext);
    }

    let mss = MediaSourceStream::new(Box::new(file), Default::default());
    let meta_opts: MetadataOptions = Default::default();
    let fmt_opts: FormatOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .probe(&hint, mss, fmt_opts, meta_opts)
        .with_context(|| format!("Unsupported audio format for: {}", path.display()))?;

    let mut format = probed;

    // Find the default audio track
    let track = format
        .tracks()
        .iter()
        .find(|t| {
            if let Some(CodecParameters::Audio(ref audio_params)) = t.codec_params {
                audio_params.codec != CODEC_ID_NULL_AUDIO
            } else {
                false
            }
        })
        .ok_or_else(|| anyhow::anyhow!("No audio track found in file"))?;

    let track_id = track.id;
    let audio_params = match &track.codec_params {
        Some(CodecParameters::Audio(params)) => params.clone(),
        _ => bail!("Track is not an audio track"),
    };

    let sample_rate = audio_params
        .sample_rate
        .ok_or_else(|| anyhow::anyhow!("Audio sample rate missing"))?;

    let channels = audio_params
        .channels
        .as_ref()
        .map(|c| c.count() as u16)
        .unwrap_or(1);

    let dec_opts: AudioDecoderOptions = Default::default();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&audio_params, &dec_opts)
        .with_context(|| "Failed to create audio decoder")?;

    let mut mono_samples = Vec::new();
    let mut interleaved_buf = Vec::new();

    loop {
        let packet = match format.next_packet() {
            Ok(Some(packet)) => packet,
            Ok(None) => break, // EOF reached
            Err(SymphoniaError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(SymphoniaError::IoError(ref e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::IoError(_)) => {
                break;
            }
            Err(err) => {
                bail!("Error while decoding packet: {:?}", err);
            }
        };

        if packet.track_id != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            Err(SymphoniaError::DecodeError(err)) => {
                eprintln!("Warning: skipping un-decodable audio frame: {}", err);
                continue;
            }
            Err(err) => {
                bail!("Error decoding audio packet: {:?}", err);
            }
        };

        interleaved_buf.clear();
        decoded.copy_to_vec_interleaved::<f32>(&mut interleaved_buf);

        let n_channels = channels.max(1) as usize;
        for frame in interleaved_buf.chunks(n_channels) {
            let sum: f32 = frame.iter().sum();
            mono_samples.push(sum / frame.len() as f32);
        }
    }

    if mono_samples.is_empty() {
        bail!("Decoded audio file contains 0 samples");
    }

    let duration_ms = (mono_samples.len() as u64 * 1000) / (sample_rate as u64);

    Ok(DecodedAudio {
        samples: mono_samples,
        sample_rate,
        channels,
        duration_ms,
    })
}
