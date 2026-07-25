/// Sample rate used when synthesising audio.
pub const ENCODE_SR: u32 = 48_000;
/// Length of the tone burst within a symbol.
pub const TONE_SAMPLES: usize = 1024;
/// Silent guard interval after each tone.
pub const GUARD_SAMPLES: usize = 1024;
/// Full symbol period (tone + guard).
pub const SYMBOL_SAMPLES: usize = TONE_SAMPLES + GUARD_SAMPLES;
/// Symbol period in seconds (sample-rate independent).
pub const SYMBOL_DURATION: f32 = SYMBOL_SAMPLES as f32 / ENCODE_SR as f32;
/// Tone-burst duration in seconds.
pub const TONE_DURATION: f32 = TONE_SAMPLES as f32 / ENCODE_SR as f32;
/// Bin spacing for the exact tone grid.
pub const DF: f32 = ENCODE_SR as f32 / TONE_SAMPLES as f32;
/// First data tone (bin 40, 1875 Hz).
pub const F_DATA0: f32 = 40.0 * DF;
/// Bin spacing between adjacent data tones.
pub const TONE_STEP: usize = 1;
/// Marker/preamble tone above the data band.
pub const F_MARKER: f32 = 64.0 * DF;
pub const PREAMBLE_SYMBOLS: usize = 16;
pub const SYNC_BYTE: u8 = 0xA5;
pub const AMPLITUDE: f32 = 0.6;
pub const MAX_PAYLOAD: usize = 255;

pub(crate) const SYNC_REPEATS: usize = 5;
pub(crate) const LEN_REPEATS: usize = 5;

/// Frequency of the data tone for a nibble value in `0..16`.
#[inline]
pub fn data_freq(value: u8) -> f32 {
    F_DATA0 + (value as usize * TONE_STEP) as f32 * DF
}

/// Samples per symbol period for a given sample rate.
pub fn symbol_len(sample_rate: u32) -> usize {
    ((SYMBOL_DURATION * sample_rate as f32).round() as usize).max(1)
}

/// Samples in the tone burst for a given sample rate.
pub fn tone_len(sample_rate: u32) -> usize {
    ((TONE_DURATION * sample_rate as f32).round() as usize).max(1)
}

pub(crate) fn crc8(data: &[u8]) -> u8 {
    let mut checksum = 0u8;
    for &byte in data {
        checksum ^= byte;
        for _ in 0..8 {
            checksum = if checksum & 0x80 != 0 {
                (checksum << 1) ^ 0x07
            } else {
                checksum << 1
            };
        }
    }
    checksum
}

pub(crate) fn majority_byte<const N: usize>(values: &[u8; N]) -> Option<u8> {
    values
        .iter()
        .copied()
        .find(|candidate| values.iter().filter(|&&value| value == *candidate).count() > N / 2)
}
