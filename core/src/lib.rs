//! Acoustic modem, ggwave-inspired.
//!
//! Frames use exact FFT-bin tones, a marker preamble, repeated synchronization
//! and length bytes, Reed-Solomon protection, and a CRC-8 checksum.

mod decoder;
mod detector;
mod diagnostics;
mod encoder;
mod protocol;

pub mod output;
pub mod rs;
pub mod tone;
pub mod transfer;

pub use decoder::{decode_all, Decoder};
pub use detector::detect_tone;
pub use diagnostics::diagnose;
pub use encoder::{encode, encode_repeated, encoded_sample_count};
pub use protocol::{
    data_freq, symbol_len, tone_len, AMPLITUDE, DF, ENCODE_SR, F_DATA0, F_HANDSHAKE_ACK,
    F_HANDSHAKE_REQ, F_END_ACK, F_MARKER, GUARD_SAMPLES, HANDSHAKE_TONE_SECS, MAX_PAYLOAD,
    PREAMBLE_SYMBOLS, SYMBOL_DURATION, SYMBOL_SAMPLES, SYNC_BYTE, TONE_DURATION, TONE_SAMPLES,
    TONE_STEP,
};

#[cfg(test)]
mod tests;
