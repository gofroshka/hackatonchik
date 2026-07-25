//! Acoustic modem with an OFDM data plane and FSK control/fallback frames.

mod decoder;
mod detector;
mod diagnostics;
mod encoder;
mod hybrid;
mod ofdm;
mod protocol;

pub mod output;
pub mod rs;
pub mod transfer;

pub use decoder::{decode_all, decode_all_with_profile, Decoder};
pub use diagnostics::diagnose;
pub use encoder::{
    encode, encode_repeated, encode_with_profile, encoded_sample_count,
    encoded_sample_count_with_profile,
};
pub use hybrid::{
    encode_hybrid_packet, hybrid_packet_routes, hybrid_sample_count, HybridDecoder, PacketPhy,
};
pub use ofdm::{
    decode_all_ofdm, encode_ofdm, encoded_ofdm_sample_count, ofdm_expected_body_bits,
    ofdm_probe_frames, OfdmDecoder, OfdmFrameProbe, OfdmModulation, OfdmProfile, OFDM_CP_SAMPLES,
    OFDM_FFT_SIZE, OFDM_SYMBOL_SAMPLES,
};
pub use protocol::{
    data_freq, symbol_len, tone_len, AcousticProfile, PhyMode, AMPLITUDE, DF, ENCODE_SR, F_DATA0,
    F_MARKER, GUARD_SAMPLES, MAX_LANES, MAX_PAYLOAD, PREAMBLE_SYMBOLS, SYMBOL_DURATION,
    SYMBOL_SAMPLES, SYNC_BYTE, TONE_DURATION, TONE_SAMPLES, TONE_STEP,
};

#[cfg(test)]
mod tests;
