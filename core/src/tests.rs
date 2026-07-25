use std::f32::consts::PI;

use crate::encoder::frame_bytes;
use crate::protocol::{LEN_REPEATS, SYNC_REPEATS};
use crate::*;

fn overwrite_frame_byte(wave: &mut [f32], byte_index: usize, value: u8) {
    let ramp = (TONE_SAMPLES / 16).max(1);
    for (nibble_index, nibble) in [value >> 4, value & 0x0f].into_iter().enumerate() {
        let period_index = 1 + PREAMBLE_SYMBOLS + byte_index * 2 + nibble_index;
        let start = period_index * SYMBOL_SAMPLES;
        let phase_step = 2.0 * PI * data_freq(nibble) / ENCODE_SR as f32;
        let mut phase = 0.0f32;
        for index in 0..TONE_SAMPLES {
            let edge = index.min(TONE_SAMPLES - 1 - index);
            let envelope = if edge < ramp {
                0.5 * (1.0 - (PI * edge as f32 / ramp as f32).cos())
            } else {
                1.0
            };
            wave[start + index] = phase.sin() * AMPLITUDE * envelope;
            phase += phase_step;
        }
    }
}

#[test]
fn roundtrip() {
    let message = b"Hello, acoustic world!";
    assert_eq!(
        decode_all(ENCODE_SR, &encode(message)),
        vec![message.to_vec()]
    );
}

#[test]
fn roundtrip_with_noise_and_offset() {
    let message = b"data over sound";
    let mut wave = vec![0.0f32; 1234];
    wave.extend(encode(message));
    let mut seed = 42u32;
    for sample in &mut wave {
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        *sample += ((seed >> 16) as f32 / 65535.0 - 0.5) * 0.05;
    }
    assert_eq!(decode_all(ENCODE_SR, &wave), vec![message.to_vec()]);
}

#[test]
fn fec_corrects_header_and_sixteen_byte_errors_in_audio() {
    let message = b"Reed-Solomon survives a noisy acoustic channel";
    let mut wave = encode(message);
    let frame = frame_bytes(message);
    overwrite_frame_byte(&mut wave, 0, frame[0].wrapping_add(3));
    overwrite_frame_byte(&mut wave, 1, frame[1].wrapping_add(5));
    overwrite_frame_byte(&mut wave, SYNC_REPEATS, frame[SYNC_REPEATS].wrapping_add(7));
    overwrite_frame_byte(
        &mut wave,
        SYNC_REPEATS + 1,
        frame[SYNC_REPEATS + 1].wrapping_add(11),
    );
    for index in 0..16 {
        let byte_index = SYNC_REPEATS + LEN_REPEATS + index;
        overwrite_frame_byte(
            &mut wave,
            byte_index,
            frame[byte_index] ^ (0x31 + index as u8),
        );
    }
    assert_eq!(decode_all(ENCODE_SR, &wave), vec![message.to_vec()]);
}

#[test]
fn roundtrip_streaming_small_chunks() {
    let message = b"hello over sound";
    let mut wave = vec![0.0f32; 777];
    wave.extend(encode(message));
    wave.extend(vec![0.0f32; 2000]);
    let mut decoder = Decoder::new(ENCODE_SR);
    let mut decoded = Vec::new();
    for chunk in wave.chunks(371) {
        decoder.push(chunk);
        while let Some(message) = decoder.poll() {
            decoded.push(message);
        }
    }
    assert_eq!(decoded, vec![message.to_vec()]);
}

#[test]
fn roundtrip_resampled_44100() {
    let message = b"cross rate";
    let wave_48k = encode(message);
    let ratio = 44_100.0 / 48_000.0;
    let output_len = (wave_48k.len() as f32 * ratio) as usize;
    let mut wave_44k = Vec::with_capacity(output_len);
    for index in 0..output_len {
        let source = index as f32 / ratio;
        let source_index = source.floor() as usize;
        let fraction = source - source_index as f32;
        let first = wave_48k[source_index.min(wave_48k.len() - 1)];
        let second = wave_48k[(source_index + 1).min(wave_48k.len() - 1)];
        wave_44k.push(first + (second - first) * fraction);
    }
    assert_eq!(decode_all(44_100, &wave_44k), vec![message.to_vec()]);
}
