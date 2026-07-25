//! OFDM data plane for the acoustic transfer path.
//!
//! Everything is differential across time (per subcarrier), which cancels the
//! speaker/microphone channel, a fixed sample-timing offset and a constant
//! carrier rotation without any coherent channel estimate. Both the header and
//! the body repeat every coded cell on several well-separated carriers, so a
//! notch in the handset response cannot kill a bit. This is what lets it survive
//! a real laptop-speaker path.

use std::f32::consts::{FRAC_PI_2, PI};
use std::sync::Arc;

use rustfft::{num_complex::Complex, num_traits::Zero, Fft, FftPlanner};

use crate::{protocol::crc8, rs, MAX_LANES, MAX_PAYLOAD};

pub const OFDM_FFT_SIZE: usize = 1024;
pub const OFDM_CP_SAMPLES: usize = 384;
pub const OFDM_SYMBOL_SAMPLES: usize = OFDM_FFT_SIZE + OFDM_CP_SAMPLES;

const EDGE_SILENCE: usize = 128;
const ACTIVE_CARRIERS: usize = 72;
const DATA_CARRIERS: usize = 64;
const PILOT_STRIDE: usize = 9;
const HEADER_BYTES: usize = 8;
const HEADER_BITS: usize = HEADER_BYTES * 8;
/// Header always uses 4x frequency diversity (16 cells) and enough symbols for
/// 2x time redundancy of its 64 bits, then a CRC-8 validates it.
const HEADER_REPETITION: usize = 4;
const HEADER_CELLS: usize = DATA_CARRIERS / HEADER_REPETITION;
const HEADER_SYMBOLS: usize = 8;
const PREAMBLE_SYMBOLS: usize = 1;
/// Symbols before the body: the preamble/reference plus the header.
const FIXED_SYMBOLS: usize = PREAMBLE_SYMBOLS + HEADER_SYMBOLS;
const REFERENCE_INDEX: usize = 0;
/// FFT window start inside the cyclic prefix. A large guard before the window
/// absorbs channel delay spread (speaker/room reverb) without inter-symbol
/// interference; it must stay <= CP so the window ends inside the symbol.
const WINDOW_OFFSET: usize = 256;
const CORRELATION_STEP: usize = 4;
const CORRELATION_THRESHOLD: f32 = 0.6;
const OUTPUT_SCALE: f32 = 0.018;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OfdmModulation {
    Qpsk,
    Qam16,
}

impl OfdmModulation {
    const fn wire_value(self) -> u8 {
        match self {
            Self::Qpsk => 1,
            Self::Qam16 => 2,
        }
    }

    /// Differential PSK order used by the body: DBPSK for the reliable default,
    /// DQPSK for the faster mode.
    const fn bits_per_carrier(self) -> usize {
        match self {
            Self::Qpsk => 1,
            Self::Qam16 => 2,
        }
    }

    /// How many carriers carry each coded cell. The reliable default repeats
    /// every cell on four carriers spread across the band (stride = cells), so a
    /// localized notch cannot hit every copy.
    const fn repetition(self) -> usize {
        match self {
            Self::Qpsk => 4,
            Self::Qam16 => 2,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OfdmProfile {
    modulation: OfdmModulation,
    lane: u8,
}

impl OfdmProfile {
    pub const fn qpsk(lane: u8) -> Self {
        assert!(lane < MAX_LANES, "acoustic lane out of range");
        Self {
            modulation: OfdmModulation::Qpsk,
            lane,
        }
    }

    pub const fn qam16(lane: u8) -> Self {
        assert!(lane < MAX_LANES, "acoustic lane out of range");
        Self {
            modulation: OfdmModulation::Qam16,
            lane,
        }
    }

    pub const fn modulation(self) -> OfdmModulation {
        self.modulation
    }

    pub const fn lane(self) -> u8 {
        self.lane
    }

    const fn first_active_bin(self) -> usize {
        36 + self.lane as usize * 96
    }

    const fn cells(self) -> usize {
        DATA_CARRIERS / self.modulation.repetition()
    }

    const fn body_bits_per_symbol(self) -> usize {
        self.cells() * self.modulation.bits_per_carrier()
    }
}

impl Default for OfdmProfile {
    fn default() -> Self {
        Self::qpsk(0)
    }
}

fn body_symbol_count(payload_len: usize, profile: OfdmProfile) -> usize {
    let protected_len = rs::encoded_len(payload_len + 2);
    (protected_len * 8).div_ceil(profile.body_bits_per_symbol())
}

/// Coded-cell index for a data carrier: carriers `k`, `k + cells`, ... share a
/// cell so repeated copies land on well-separated frequencies.
fn carrier_cell(carrier: usize, cells: usize) -> usize {
    carrier % cells
}

pub fn encoded_ofdm_sample_count(payload_len: usize, profile: OfdmProfile) -> usize {
    assert!(payload_len <= MAX_PAYLOAD, "payload too large");
    let body_symbols = body_symbol_count(payload_len, profile);
    EDGE_SILENCE * 2 + (FIXED_SYMBOLS + body_symbols) * OFDM_SYMBOL_SAMPLES
}

fn header_bytes(payload_len: usize, profile: OfdmProfile) -> [u8; HEADER_BYTES] {
    let mut header = [0u8; HEADER_BYTES];
    header[0] = b'O';
    header[1] = b'F';
    header[2] = payload_len as u8;
    header[3] = profile.modulation.wire_value();
    header[4] = profile.lane;
    header[5] = crc8(&header[..5]);
    header[6] = 0xA5;
    header[7] = 0x5A;
    header
}

pub fn encode_ofdm(payload: &[u8], profile: OfdmProfile) -> Vec<f32> {
    assert!(payload.len() <= MAX_PAYLOAD, "payload too large");
    let mut planner = FftPlanner::<f32>::new();
    let inverse = planner.plan_fft_inverse(OFDM_FFT_SIZE);

    let mut protected = Vec::with_capacity(payload.len() + 2);
    protected.extend_from_slice(payload);
    protected.extend_from_slice(&crc16(payload).to_le_bytes());
    let encoded = rs::encode_blocks(&protected);
    let body_bits = bytes_to_bits(&encoded);
    let header_bits = bytes_to_bits(&header_bytes(payload.len(), profile));

    let bits_per_carrier = profile.modulation.bits_per_carrier();
    let body_cells = profile.cells();
    let body_symbols = body_symbol_count(payload.len(), profile);

    let mut output = Vec::with_capacity(encoded_ofdm_sample_count(payload.len(), profile));
    output.resize(EDGE_SILENCE, 0.0);

    // Differential reference / preamble: every active carrier at phase zero.
    let mut phase = vec![0.0f32; DATA_CARRIERS];
    append_body_symbol(&mut output, profile, &phase, &inverse);

    // Header: differential DBPSK, 4x frequency diversity, 2x time redundancy.
    for symbol in 0..HEADER_SYMBOLS {
        for (carrier, slot) in phase.iter_mut().enumerate() {
            let cell = carrier_cell(carrier, HEADER_CELLS);
            let bit = header_bits[(symbol * HEADER_CELLS + cell) % HEADER_BITS];
            *slot += dbpsk_phase(bit);
        }
        append_body_symbol(&mut output, profile, &phase, &inverse);
    }

    // Body: carrier-major interleaving with the profile's frequency diversity.
    for symbol in 0..body_symbols {
        for (carrier, slot) in phase.iter_mut().enumerate() {
            let cell = carrier_cell(carrier, body_cells);
            let base = (cell * body_symbols + symbol) * bits_per_carrier;
            let b0 = body_bits.get(base).copied().unwrap_or(false);
            *slot += if bits_per_carrier == 1 {
                dbpsk_phase(b0)
            } else {
                dqpsk_phase(b0, body_bits.get(base + 1).copied().unwrap_or(false))
            };
        }
        append_body_symbol(&mut output, profile, &phase, &inverse);
    }

    output.resize(output.len() + EDGE_SILENCE, 0.0);
    debug_assert_eq!(
        output.len(),
        encoded_ofdm_sample_count(payload.len(), profile)
    );
    output
}

pub struct OfdmDecoder {
    profile: OfdmProfile,
    forward: Arc<dyn Fft<f32>>,
    buffer: Vec<f32>,
    scan_pos: usize,
}

impl OfdmDecoder {
    pub fn new(profile: OfdmProfile) -> Self {
        let mut planner = FftPlanner::<f32>::new();
        let forward = planner.plan_fft_forward(OFDM_FFT_SIZE);
        Self {
            profile,
            forward,
            buffer: Vec::new(),
            scan_pos: 0,
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);
    }

    pub fn poll(&mut self) -> Option<Vec<u8>> {
        loop {
            let frame_start = self.find_preamble()?;
            let fixed_end = frame_start + FIXED_SYMBOLS * OFDM_SYMBOL_SAMPLES;
            if self.buffer.len() < fixed_end {
                self.scan_pos = frame_start;
                return None;
            }

            let Some(payload_len) = self.decode_header(frame_start) else {
                // Every OFDM symbol has a cyclic prefix, so the CP metric is high
                // throughout a frame. Skip a whole symbol on a header miss instead
                // of crawling sample-by-sample, which would fall behind real time.
                self.scan_pos = frame_start + OFDM_SYMBOL_SAMPLES;
                continue;
            };
            let body_symbols = body_symbol_count(payload_len, self.profile);
            let frame_end = frame_start + (FIXED_SYMBOLS + body_symbols) * OFDM_SYMBOL_SAMPLES;
            if self.buffer.len() < frame_end {
                self.scan_pos = frame_start;
                return None;
            }
            if ofdm_debug() {
                eprintln!("[ofdm] frame at {frame_start}: payload={payload_len}");
            }

            let payload = self.decode_body(frame_start, payload_len, body_symbols);
            let consume = (frame_end + EDGE_SILENCE).min(self.buffer.len());
            self.buffer.drain(..consume);
            self.scan_pos = 0;
            if let Some(payload) = payload {
                return Some(payload);
            }
            if ofdm_debug() {
                eprintln!("[ofdm] frame at {frame_start}: body rejected");
            }
        }
    }

    fn find_preamble(&mut self) -> Option<usize> {
        let symbol_len = OFDM_SYMBOL_SAMPLES;
        while self.scan_pos + symbol_len <= self.buffer.len() {
            let score =
                cyclic_prefix_correlation(&self.buffer[self.scan_pos..self.scan_pos + symbol_len]);
            if score >= CORRELATION_THRESHOLD {
                let coarse = self.scan_pos;
                let end = (coarse + CORRELATION_STEP).min(self.buffer.len() - symbol_len);
                let mut best = (score, coarse);
                for candidate in coarse..=end {
                    let candidate_score =
                        cyclic_prefix_correlation(&self.buffer[candidate..candidate + symbol_len]);
                    if candidate_score > best.0 {
                        best = (candidate_score, candidate);
                    }
                }
                return Some(best.1);
            }
            self.scan_pos += CORRELATION_STEP;
        }
        None
    }

    fn decode_header(&self, frame_start: usize) -> Option<usize> {
        let reference_pos = frame_start + REFERENCE_INDEX * OFDM_SYMBOL_SAMPLES;
        let mut previous = self.active_spectrum(reference_pos)?;
        let mut metric = [0.0f32; HEADER_BITS];
        for symbol in 0..HEADER_SYMBOLS {
            let pos = frame_start + (PREAMBLE_SYMBOLS + symbol) * OFDM_SYMBOL_SAMPLES;
            let current = self.active_spectrum(pos)?;
            let cells = self.differential_cells(&current, &previous, HEADER_CELLS);
            for (cell, value) in cells.iter().enumerate() {
                metric[(symbol * HEADER_CELLS + cell) % HEADER_BITS] += value.re;
            }
            previous = current;
        }
        let bits = metric.iter().map(|&m| m < 0.0).collect::<Vec<_>>();
        let header = bits_to_bytes(&bits, HEADER_BYTES);
        if header.len() != HEADER_BYTES
            || header[0..2] != *b"OF"
            || header[3] != self.profile.modulation.wire_value()
            || header[4] != self.profile.lane
            || header[5] != crc8(&header[..5])
            || header[6] != 0xA5
            || header[7] != 0x5A
        {
            return None;
        }
        Some(header[2] as usize)
    }

    fn decode_body(
        &self,
        frame_start: usize,
        payload_len: usize,
        body_symbols: usize,
    ) -> Option<Vec<u8>> {
        let bits_per_carrier = self.profile.modulation.bits_per_carrier();
        let cells = self.profile.cells();
        // The first body symbol references the last header symbol.
        let reference_pos = frame_start + (FIXED_SYMBOLS - 1) * OFDM_SYMBOL_SAMPLES;
        let mut previous = self.active_spectrum(reference_pos)?;
        let mut bits = vec![false; body_symbols * cells * bits_per_carrier];
        for symbol in 0..body_symbols {
            let pos = frame_start + (FIXED_SYMBOLS + symbol) * OFDM_SYMBOL_SAMPLES;
            let current = self.active_spectrum(pos)?;
            let combined = self.differential_cells(&current, &previous, cells);
            for (cell, value) in combined.iter().enumerate() {
                let base = (cell * body_symbols + symbol) * bits_per_carrier;
                if bits_per_carrier == 1 {
                    bits[base] = value.re < 0.0;
                } else {
                    let (b0, b1) = dqpsk_demap(*value);
                    bits[base] = b0;
                    bits[base + 1] = b1;
                }
            }
            previous = current;
        }
        let encoded_len = rs::encoded_len(payload_len + 2);
        let encoded = bits_to_bytes(&bits, encoded_len);
        let (decoded, corrected) = rs::decode_blocks(&encoded, payload_len + 2).ok()?;
        let (payload, checksum) = decoded.split_at(payload_len);
        if checksum != crc16(payload).to_le_bytes() {
            if ofdm_debug() {
                eprintln!("[ofdm] CRC failed after correcting {corrected} bytes");
            }
            return None;
        }
        Some(payload.to_vec())
    }

    /// Differential of one symbol vs the previous, common-rotation corrected with
    /// pilots, then soft-combined across the repeated carriers of each cell.
    fn differential_cells(
        &self,
        current: &[Complex<f32>],
        previous: &[Complex<f32>],
        cells: usize,
    ) -> Vec<Complex<f32>> {
        let mut rotation = Complex::<f32>::zero();
        let mut differentials = Vec::with_capacity(ACTIVE_CARRIERS);
        for (active_index, (cur, prev)) in current.iter().zip(previous).enumerate() {
            let differential = cur * prev.conj();
            if is_pilot(active_index) {
                rotation += differential;
            }
            differentials.push(differential);
        }
        let derotate = if rotation.norm_sqr() > 1e-12 {
            rotation.conj() / rotation.norm()
        } else {
            Complex::new(1.0, 0.0)
        };
        let mut combined = vec![Complex::<f32>::zero(); cells];
        let mut carrier = 0usize;
        for (active_index, differential) in differentials.iter().enumerate() {
            if is_pilot(active_index) {
                continue;
            }
            combined[carrier_cell(carrier, cells)] += differential * derotate;
            carrier += 1;
        }
        combined
    }

    fn fft_at(&self, pos: usize) -> Option<Vec<Complex<f32>>> {
        let start = pos + WINDOW_OFFSET;
        if start + OFDM_FFT_SIZE > self.buffer.len() {
            return None;
        }
        let mut values = self.buffer[start..start + OFDM_FFT_SIZE]
            .iter()
            .map(|&sample| Complex::new(sample, 0.0))
            .collect::<Vec<_>>();
        self.forward.process(&mut values);
        Some(values)
    }

    fn active_spectrum(&self, pos: usize) -> Option<Vec<Complex<f32>>> {
        let spectrum = self.fft_at(pos)?;
        Some(active_bins(self.profile).map(|bin| spectrum[bin]).collect())
    }
}

pub fn decode_all_ofdm(samples: &[f32], profile: OfdmProfile) -> Vec<Vec<u8>> {
    let mut decoder = OfdmDecoder::new(profile);
    decoder.push(samples);
    let mut payloads = Vec::new();
    while let Some(payload) = decoder.poll() {
        payloads.push(payload);
    }
    payloads
}

/// Raw demapped body bits of one detected OFDM frame, before RS/CRC. Diagnostic.
#[derive(Debug, Clone)]
pub struct OfdmFrameProbe {
    pub payload_len: usize,
    pub body_bits: Vec<bool>,
}

/// Exact body bits that `encode_ofdm` transmits for a payload. Diagnostic.
pub fn ofdm_expected_body_bits(payload: &[u8], _profile: OfdmProfile) -> Vec<bool> {
    let mut protected = payload.to_vec();
    protected.extend_from_slice(&crc16(payload).to_le_bytes());
    bytes_to_bits(&rs::encode_blocks(&protected))
}

/// Decode raw body bits for every detected frame without RS correction. Diagnostic.
pub fn ofdm_probe_frames(samples: &[f32], profile: OfdmProfile) -> Vec<OfdmFrameProbe> {
    let mut decoder = OfdmDecoder::new(profile);
    decoder.push(samples);
    let mut frames = Vec::new();
    while let Some(frame_start) = decoder.find_preamble() {
        let fixed_end = frame_start + FIXED_SYMBOLS * OFDM_SYMBOL_SAMPLES;
        if decoder.buffer.len() < fixed_end {
            break;
        }
        let Some(payload_len) = decoder.decode_header(frame_start) else {
            decoder.scan_pos = frame_start + OFDM_SYMBOL_SAMPLES;
            continue;
        };
        let body_symbols = body_symbol_count(payload_len, profile);
        let frame_end = frame_start + (FIXED_SYMBOLS + body_symbols) * OFDM_SYMBOL_SAMPLES;
        if decoder.buffer.len() < frame_end {
            break;
        }
        if let Some(body_bits) = decoder.raw_body_bits(frame_start, payload_len, body_symbols) {
            frames.push(OfdmFrameProbe {
                payload_len,
                body_bits,
            });
        }
        let consume = (frame_end + EDGE_SILENCE).min(decoder.buffer.len());
        decoder.buffer.drain(..consume);
        decoder.scan_pos = 0;
    }
    frames
}

impl OfdmDecoder {
    fn raw_body_bits(
        &self,
        frame_start: usize,
        payload_len: usize,
        body_symbols: usize,
    ) -> Option<Vec<bool>> {
        let bits_per_carrier = self.profile.modulation.bits_per_carrier();
        let cells = self.profile.cells();
        let reference_pos = frame_start + (FIXED_SYMBOLS - 1) * OFDM_SYMBOL_SAMPLES;
        let mut previous = self.active_spectrum(reference_pos)?;
        let mut bits = vec![false; body_symbols * cells * bits_per_carrier];
        for symbol in 0..body_symbols {
            let pos = frame_start + (FIXED_SYMBOLS + symbol) * OFDM_SYMBOL_SAMPLES;
            let current = self.active_spectrum(pos)?;
            let combined = self.differential_cells(&current, &previous, cells);
            for (cell, value) in combined.iter().enumerate() {
                let base = (cell * body_symbols + symbol) * bits_per_carrier;
                if bits_per_carrier == 1 {
                    bits[base] = value.re < 0.0;
                } else {
                    let (b0, b1) = dqpsk_demap(*value);
                    bits[base] = b0;
                    bits[base + 1] = b1;
                }
            }
            previous = current;
        }
        let _ = payload_len;
        Some(bits)
    }
}

fn active_bins(profile: OfdmProfile) -> impl Iterator<Item = usize> {
    profile.first_active_bin()..profile.first_active_bin() + ACTIVE_CARRIERS
}

fn is_pilot(active_index: usize) -> bool {
    active_index.is_multiple_of(PILOT_STRIDE)
}

/// Differential body symbol: constant pilots and unit-magnitude data carriers.
fn append_body_symbol(
    output: &mut Vec<f32>,
    profile: OfdmProfile,
    data_phase: &[f32],
    inverse: &Arc<dyn Fft<f32>>,
) {
    let mut spectrum = vec![Complex::zero(); OFDM_FFT_SIZE];
    let mut phases = data_phase.iter().copied();
    for (active_index, bin) in active_bins(profile).enumerate() {
        let value = if is_pilot(active_index) {
            Complex::new(1.0, 0.0)
        } else {
            Complex::from_polar(1.0, phases.next().unwrap_or(0.0))
        };
        spectrum[bin] = value;
        spectrum[OFDM_FFT_SIZE - bin] = value.conj();
    }
    inverse.process(&mut spectrum);
    let useful = spectrum
        .iter()
        .map(|value| value.re * OUTPUT_SCALE)
        .collect::<Vec<_>>();
    output.extend_from_slice(&useful[OFDM_FFT_SIZE - OFDM_CP_SAMPLES..]);
    output.extend_from_slice(&useful);
}

fn dbpsk_phase(bit: bool) -> f32 {
    if bit {
        PI
    } else {
        0.0
    }
}

fn dqpsk_phase(b0: bool, b1: bool) -> f32 {
    match (b0, b1) {
        (false, false) => 0.0,
        (false, true) => FRAC_PI_2,
        (true, true) => PI,
        (true, false) => -FRAC_PI_2,
    }
}

fn dqpsk_demap(value: Complex<f32>) -> (bool, bool) {
    let index = (value.arg() / FRAC_PI_2).round() as i32;
    match index.rem_euclid(4) {
        0 => (false, false),
        1 => (false, true),
        2 => (true, true),
        _ => (true, false),
    }
}

fn bytes_to_bits(bytes: &[u8]) -> Vec<bool> {
    bytes
        .iter()
        .flat_map(|byte| (0..8).rev().map(move |shift| byte & (1 << shift) != 0))
        .collect()
}

fn bits_to_bytes(bits: &[bool], byte_len: usize) -> Vec<u8> {
    bits.chunks(8)
        .take(byte_len)
        .map(|chunk| {
            chunk.iter().enumerate().fold(0u8, |byte, (index, bit)| {
                byte | ((*bit as u8) << (7 - index))
            })
        })
        .collect()
}

fn cyclic_prefix_correlation(samples: &[f32]) -> f32 {
    let mut dot = 0.0;
    let mut prefix_energy = 0.0;
    let mut suffix_energy = 0.0;
    for index in 0..OFDM_CP_SAMPLES {
        let prefix = samples[index];
        let suffix = samples[OFDM_FFT_SIZE + index];
        dot += prefix * suffix;
        prefix_energy += prefix * prefix;
        suffix_energy += suffix * suffix;
    }
    if prefix_energy <= 1e-12 || suffix_energy <= 1e-12 {
        0.0
    } else {
        dot.abs() / (prefix_energy * suffix_energy).sqrt()
    }
}

fn crc16(data: &[u8]) -> u16 {
    let mut checksum = 0xFFFFu16;
    for &byte in data {
        checksum ^= u16::from(byte) << 8;
        for _ in 0..8 {
            checksum = if checksum & 0x8000 != 0 {
                (checksum << 1) ^ 0x1021
            } else {
                checksum << 1
            };
        }
    }
    checksum
}

fn ofdm_debug() -> bool {
    std::env::var_os("OFDM_DEBUG").is_some()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_roundtrip_for_both_modulations_and_lanes() {
        let payload = (0..211)
            .map(|value| (value * 73 + 19) as u8)
            .collect::<Vec<_>>();
        for profile in [
            OfdmProfile::qpsk(0),
            OfdmProfile::qpsk(1),
            OfdmProfile::qam16(0),
            OfdmProfile::qam16(1),
        ] {
            let waveform = encode_ofdm(&payload, profile);
            assert_eq!(
                waveform.len(),
                encoded_ofdm_sample_count(payload.len(), profile)
            );
            assert_eq!(decode_all_ofdm(&waveform, profile), vec![payload.clone()]);
        }
    }

    #[test]
    fn streaming_decoder_handles_uneven_chunks() {
        let payload = b"streamed OFDM payload";
        let profile = OfdmProfile::qpsk(0);
        let waveform = encode_ofdm(payload, profile);
        let mut decoder = OfdmDecoder::new(profile);
        let mut decoded = None;
        for chunk in waveform.chunks(997) {
            decoder.push(chunk);
            decoded = decoded.or_else(|| decoder.poll());
        }
        assert_eq!(decoded.as_deref(), Some(payload.as_slice()));
    }

    #[test]
    fn lane_decoder_rejects_other_lane() {
        let waveform = encode_ofdm(b"lane one", OfdmProfile::qpsk(1));
        assert!(decode_all_ofdm(&waveform, OfdmProfile::qpsk(0)).is_empty());
    }
}
