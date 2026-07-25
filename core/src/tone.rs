use std::f32::consts::PI;

use crate::protocol::{AMPLITUDE, ENCODE_SR};

/// Generate a pure sine wave tone burst at a given frequency.
/// Returns f32 samples at `ENCODE_SR` (48 kHz).
pub fn generate(duration_secs: f32, frequency: f32) -> Vec<f32> {
    let num_samples = (duration_secs * ENCODE_SR as f32).round() as usize;
    let ramp = (num_samples / 32).max(1);
    let phase_step = 2.0 * PI * frequency / ENCODE_SR as f32;
    let mut phase = 0.0f32;
    let mut samples = Vec::with_capacity(num_samples);
    for i in 0..num_samples {
        let mut envelope = 1.0f32;
        if i < ramp {
            envelope = 0.5 * (1.0 - (PI * (ramp - i) as f32 / ramp as f32).cos());
        } else if i >= num_samples - ramp {
            let edge = i - (num_samples - ramp);
            envelope = 0.5 * (1.0 + (PI * (ramp - edge) as f32 / ramp as f32).cos());
        }
        samples.push(phase.sin() * AMPLITUDE * envelope);
        phase += phase_step;
        if phase > 2.0 * PI {
            phase -= 2.0 * PI;
        }
    }
    samples
}

/// Generate PCM16 bytes for a tone burst.
pub fn generate_pcm16(duration_secs: f32, frequency: f32) -> Vec<u8> {
    let samples = generate(duration_secs, frequency);
    let mut bytes = Vec::with_capacity(samples.len() * 2);
    for &sample in &samples {
        let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes
}
