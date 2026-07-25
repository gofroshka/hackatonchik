use std::f32::consts::PI;

use crate::protocol::{
    crc8, data_freq, AMPLITUDE, ENCODE_SR, F_MARKER, GUARD_SAMPLES, LEN_REPEATS, MAX_PAYLOAD,
    PREAMBLE_SYMBOLS, SYMBOL_SAMPLES, SYNC_BYTE, SYNC_REPEATS, TONE_SAMPLES,
};
use crate::rs;

pub(crate) fn frame_bytes(payload: &[u8]) -> Vec<u8> {
    let mut inner = Vec::with_capacity(payload.len() + 2);
    inner.push(payload.len() as u8);
    inner.extend_from_slice(payload);
    inner.push(crc8(&inner));
    let encoded = rs::encode_blocks(&inner);

    let mut bytes = Vec::with_capacity(SYNC_REPEATS + LEN_REPEATS + encoded.len());
    bytes.extend(std::iter::repeat_n(SYNC_BYTE, SYNC_REPEATS));
    bytes.extend(std::iter::repeat_n(payload.len() as u8, LEN_REPEATS));
    bytes.extend_from_slice(&encoded);
    bytes
}

fn frame_nibbles(payload: &[u8]) -> Vec<u8> {
    let mut nibbles = Vec::new();
    for byte in frame_bytes(payload) {
        nibbles.push(byte >> 4);
        nibbles.push(byte & 0x0f);
    }
    nibbles
}

/// Turn a payload into a mono f32 waveform sampled at `ENCODE_SR`.
pub fn encode(payload: &[u8]) -> Vec<f32> {
    assert!(payload.len() <= MAX_PAYLOAD, "payload too large");
    let period = SYMBOL_SAMPLES;
    let mut tones = Vec::with_capacity(PREAMBLE_SYMBOLS + 64);
    tones.extend(std::iter::repeat_n(F_MARKER, PREAMBLE_SYMBOLS));
    tones.extend(frame_nibbles(payload).into_iter().map(data_freq));

    let ramp = (TONE_SAMPLES / 16).max(1);
    let mut output = Vec::with_capacity((tones.len() + 2) * period);
    output.extend(std::iter::repeat_n(0.0, period));
    for frequency in tones {
        let phase_step = 2.0 * PI * frequency / ENCODE_SR as f32;
        let mut phase = 0.0f32;
        for index in 0..TONE_SAMPLES {
            let mut envelope = 1.0f32;
            if index < ramp {
                envelope = 0.5 * (1.0 - (PI * (ramp - index) as f32 / ramp as f32).cos());
            }
            if index >= TONE_SAMPLES - ramp {
                let edge_index = index - (TONE_SAMPLES - ramp);
                envelope = 0.5 * (1.0 + (PI * (ramp - edge_index) as f32 / ramp as f32).cos());
            }
            output.push(phase.sin() * AMPLITUDE * envelope);
            phase += phase_step;
            if phase > 2.0 * PI {
                phase -= 2.0 * PI;
            }
        }
        output.extend(std::iter::repeat_n(0.0, GUARD_SAMPLES));
    }
    output.extend(std::iter::repeat_n(0.0, period));
    output
}

/// Exact number of 48 kHz mono samples generated for a payload of this size.
pub fn encoded_sample_count(payload_len: usize) -> usize {
    let protected_data_len = payload_len + 2;
    let frame_bytes = SYNC_REPEATS + LEN_REPEATS + rs::encoded_len(protected_data_len);
    (2 + PREAMBLE_SYMBOLS + frame_bytes * 2) * SYMBOL_SAMPLES
}

/// Encode the payload repeatedly as independently decodable frames.
pub fn encode_repeated(payload: &[u8], repeats: usize) -> Vec<f32> {
    let one = encode(payload);
    let mut output = Vec::with_capacity(one.len() * repeats.max(1));
    for _ in 0..repeats.max(1) {
        output.extend_from_slice(&one);
    }
    output
}
