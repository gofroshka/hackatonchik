use std::f32::consts::PI;

use crate::protocol::{crc8, AcousticProfile, AMPLITUDE, ENCODE_SR, MAX_PAYLOAD, SYNC_BYTE};
use crate::rs;

#[cfg(test)]
pub(crate) fn frame_bytes(payload: &[u8]) -> Vec<u8> {
    frame_bytes_with_profile(payload, AcousticProfile::default())
}

pub(crate) fn frame_bytes_with_profile(payload: &[u8], profile: AcousticProfile) -> Vec<u8> {
    let mut inner = Vec::with_capacity(payload.len() + 2);
    inner.push(payload.len() as u8);
    inner.extend_from_slice(payload);
    inner.push(crc8(&inner));
    let encoded = rs::encode_blocks(&inner);

    let mut bytes =
        Vec::with_capacity(profile.sync_repeats() + profile.len_repeats() + encoded.len());
    bytes.extend(std::iter::repeat_n(SYNC_BYTE, profile.sync_repeats()));
    bytes.extend(std::iter::repeat_n(
        payload.len() as u8,
        profile.len_repeats(),
    ));
    bytes.extend_from_slice(&encoded);
    bytes
}

#[derive(Clone, Copy)]
struct ToneSymbol {
    first: f32,
    second: Option<f32>,
}

fn frame_symbols(payload: &[u8], profile: AcousticProfile) -> Vec<ToneSymbol> {
    let bytes = frame_bytes_with_profile(payload, profile);
    let mut symbols = Vec::with_capacity(bytes.len() * profile.frame_symbols_per_byte());
    for byte in bytes {
        if profile.is_dual_tone() {
            symbols.push(ToneSymbol {
                first: profile.data_freq(byte >> 4),
                second: Some(profile.high_data_freq(byte & 0x0f)),
            });
        } else {
            symbols.push(ToneSymbol {
                first: profile.data_freq(byte >> 4),
                second: None,
            });
            symbols.push(ToneSymbol {
                first: profile.data_freq(byte & 0x0f),
                second: None,
            });
        }
    }
    symbols
}

/// Turn a payload into a mono f32 waveform sampled at `ENCODE_SR`.
pub fn encode(payload: &[u8]) -> Vec<f32> {
    encode_with_profile(payload, AcousticProfile::default())
}

/// Encode a payload using an explicit speed/reliability profile and frequency lane.
pub fn encode_with_profile(payload: &[u8], profile: AcousticProfile) -> Vec<f32> {
    assert!(payload.len() <= MAX_PAYLOAD, "payload too large");
    let period = profile.symbol_samples();
    let tone_samples = profile.tone_samples();
    let mut tones = Vec::with_capacity(profile.preamble_symbols() + 64);
    tones.extend(std::iter::repeat_n(
        ToneSymbol {
            first: profile.marker_freq(),
            second: None,
        },
        profile.preamble_symbols(),
    ));
    tones.extend(frame_symbols(payload, profile));

    let ramp = (tone_samples / 16).max(1);
    let mut output = Vec::with_capacity((tones.len() + 2) * period);
    output.extend(std::iter::repeat_n(0.0, period));
    for symbol in tones {
        let first_step = 2.0 * PI * symbol.first / ENCODE_SR as f32;
        let second_step = symbol
            .second
            .map(|frequency| 2.0 * PI * frequency / ENCODE_SR as f32);
        let mut first_phase = 0.0f32;
        let mut second_phase = 0.0f32;
        let tone_amplitude = AMPLITUDE;
        for index in 0..tone_samples {
            let mut envelope = 1.0f32;
            if index < ramp {
                envelope = 0.5 * (1.0 - (PI * (ramp - index) as f32 / ramp as f32).cos());
            }
            if index >= tone_samples - ramp {
                let edge_index = index - (tone_samples - ramp);
                envelope = 0.5 * (1.0 + (PI * (ramp - edge_index) as f32 / ramp as f32).cos());
            }
            let mut sample = first_phase.sin();
            first_phase += first_step;
            if first_phase > 2.0 * PI {
                first_phase -= 2.0 * PI;
            }
            if let Some(step) = second_step {
                sample += second_phase.sin();
                second_phase += step;
                if second_phase > 2.0 * PI {
                    second_phase -= 2.0 * PI;
                }
            }
            output.push(sample * tone_amplitude * envelope);
        }
        output.extend(std::iter::repeat_n(0.0, profile.guard_samples()));
    }
    output.extend(std::iter::repeat_n(0.0, period));
    output
}

/// Exact number of 48 kHz mono samples generated for a payload of this size.
pub fn encoded_sample_count(payload_len: usize) -> usize {
    encoded_sample_count_with_profile(payload_len, AcousticProfile::default())
}

pub fn encoded_sample_count_with_profile(payload_len: usize, profile: AcousticProfile) -> usize {
    let protected_data_len = payload_len + 2;
    let frame_bytes =
        profile.sync_repeats() + profile.len_repeats() + rs::encoded_len(protected_data_len);
    (2 + profile.preamble_symbols() + frame_bytes * profile.frame_symbols_per_byte())
        * profile.symbol_samples()
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
