use serde::{Deserialize, Serialize};
use crate::audio::AudioFeatures;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DspSettings {
    pub master_gain: f32,
    pub low_gain: f32,
    pub mid_gain: f32,
    pub high_gain: f32,
    pub threshold: f32,
    pub attack_decay: f32,
    pub auto_gain: bool,
}

impl Default for DspSettings {
    fn default() -> Self {
        Self {
            master_gain: 1.3,
            low_gain: 1.5,
            mid_gain: 1.2,
            high_gain: 1.6,
            threshold: 0.02,
            attack_decay: 0.30,
            auto_gain: true,
        }
    }
}

pub fn process_features(
    raw: AudioFeatures,
    dsp: &DspSettings,
    peak_max: Option<AudioFeatures>,
    prev: Option<AudioFeatures>,
) -> AudioFeatures {
    // 1. Auto-Gain / Peak Normalization
    let (mut amp, mut low, mut mid, mut high) = if dsp.auto_gain {
        if let Some(peak) = peak_max {
            let norm_amp = if peak.amplitude > 0.001 { raw.amplitude / peak.amplitude } else { raw.amplitude };
            let norm_low = if peak.low > 0.001 { raw.low / peak.low } else { raw.low };
            let norm_mid = if peak.mid > 0.001 { raw.mid / peak.mid } else { raw.mid };
            let norm_high = if peak.high > 0.001 { raw.high / peak.high } else { raw.high };
            (norm_amp, norm_low, norm_mid, norm_high)
        } else {
            (raw.amplitude, raw.low, raw.mid, raw.high)
        }
    } else {
        (raw.amplitude, raw.low, raw.mid, raw.high)
    };

    // 2. Threshold / Noise Gate
    amp = apply_gate(amp, dsp.threshold);
    low = apply_gate(low, dsp.threshold);
    mid = apply_gate(mid, dsp.threshold);
    high = apply_gate(high, dsp.threshold);

    // 3. User Gain adjustments
    amp = (amp * dsp.master_gain).min(1.0);
    low = (low * dsp.low_gain * dsp.master_gain).min(1.0);
    mid = (mid * dsp.mid_gain * dsp.master_gain).min(1.0);
    high = (high * dsp.high_gain * dsp.master_gain).min(1.0);

    // 4. Attack / Decay smoothing
    if let Some(p) = prev {
        let decay = dsp.attack_decay.clamp(0.0, 0.95);
        amp = apply_envelope(amp, p.amplitude, decay);
        low = apply_envelope(low, p.low, decay);
        mid = apply_envelope(mid, p.mid, decay);
        high = apply_envelope(high, p.high, decay);
    }

    AudioFeatures {
        amplitude: amp.clamp(0.0, 1.0),
        low: low.clamp(0.0, 1.0),
        mid: mid.clamp(0.0, 1.0),
        high: high.clamp(0.0, 1.0),
    }
}

fn apply_gate(val: f32, threshold: f32) -> f32 {
    if val <= threshold {
        0.0
    } else if threshold >= 0.999 {
        val
    } else {
        (val - threshold) / (1.0 - threshold)
    }
}

fn apply_envelope(current: f32, previous: f32, decay: f32) -> f32 {
    if current >= previous {
        // Fast attack for punchy transients
        current
    } else {
        // Smooth musical decay
        previous * decay + current * (1.0 - decay)
    }
}
