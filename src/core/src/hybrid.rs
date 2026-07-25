use std::collections::VecDeque;

use crate::transfer::{classify_packet, TransferPacketKind};
use crate::{
    encode_ofdm, encode_with_profile, encoded_ofdm_sample_count, encoded_sample_count_with_profile,
    AcousticProfile, Decoder, OfdmDecoder, OfdmProfile, ENCODE_SR,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PacketPhy {
    Fsk,
    Ofdm,
}

pub fn hybrid_packet_routes(packets: &[Vec<u8>]) -> Vec<PacketPhy> {
    // OFDM (differential DBPSK with rep-2 diversity) is the reliable channel on
    // real speakers, so route every file packet through it. FSK is only used for
    // the short inline path (tiny chat messages fit in one frame).
    packets
        .iter()
        .map(|packet| match classify_packet(packet) {
            Some(TransferPacketKind::Inline) => PacketPhy::Fsk,
            _ => PacketPhy::Ofdm,
        })
        .collect()
}

pub fn encode_hybrid_packet(
    packet: &[u8],
    route: PacketPhy,
    fsk_profile: AcousticProfile,
    ofdm_profile: OfdmProfile,
) -> Vec<f32> {
    match route {
        PacketPhy::Fsk => encode_with_profile(packet, fsk_profile),
        PacketPhy::Ofdm => encode_ofdm(packet, ofdm_profile),
    }
}

pub fn hybrid_sample_count(
    packet_len: usize,
    route: PacketPhy,
    fsk_profile: AcousticProfile,
    ofdm_profile: OfdmProfile,
) -> usize {
    match route {
        PacketPhy::Fsk => encoded_sample_count_with_profile(packet_len, fsk_profile),
        PacketPhy::Ofdm => encoded_ofdm_sample_count(packet_len, ofdm_profile),
    }
}

pub struct HybridDecoder {
    fsk: Decoder,
    ofdm: OfdmDecoder,
    resampler: SampleRateConverter,
    ready: VecDeque<Vec<u8>>,
}

impl HybridDecoder {
    pub fn new(ofdm_profile: OfdmProfile) -> Self {
        Self::with_sample_rate(ENCODE_SR, ofdm_profile)
    }

    pub fn with_sample_rate(sample_rate: u32, ofdm_profile: OfdmProfile) -> Self {
        let lane = ofdm_profile.lane();
        Self {
            fsk: Decoder::with_profile(sample_rate, AcousticProfile::fast(lane)),
            ofdm: OfdmDecoder::new(ofdm_profile),
            resampler: SampleRateConverter::new(sample_rate, ENCODE_SR),
            ready: VecDeque::new(),
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.fsk.push(samples);
        let resampled = self.resampler.push(samples);
        self.ofdm.push(&resampled);
    }

    pub fn poll(&mut self) -> Option<Vec<u8>> {
        if let Some(packet) = self.ready.pop_front() {
            return Some(packet);
        }
        if let Some(packet) = self.fsk.poll() {
            self.ready.push_back(packet);
        }
        if let Some(packet) = self.ofdm.poll() {
            self.ready.push_back(packet);
        }
        self.ready.pop_front()
    }
}

struct SampleRateConverter {
    step: f64,
    position: f64,
    input: Vec<f32>,
}

impl SampleRateConverter {
    fn new(from: u32, to: u32) -> Self {
        Self {
            step: from as f64 / to as f64,
            position: 0.0,
            input: Vec::new(),
        }
    }

    fn push(&mut self, samples: &[f32]) -> Vec<f32> {
        self.input.extend_from_slice(samples);
        let mut output = Vec::with_capacity(
            ((samples.len() as f64 / self.step).ceil() as usize).saturating_add(1),
        );
        while self.position + 1.0 < self.input.len() as f64 {
            let index = self.position.floor() as usize;
            let fraction = (self.position - index as f64) as f32;
            output.push(self.input[index] + (self.input[index + 1] - self.input[index]) * fraction);
            self.position += self.step;
        }
        let consumed = self.position.floor() as usize;
        if consumed > 0 {
            self.input.drain(..consumed);
            self.position -= consumed as f64;
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use crate::transfer::{build_transfer, TransferEvent, TransferReceiver};

    use super::*;

    #[test]
    fn hybrid_transfer_routes_file_packets_through_ofdm() {
        let data = (0..4096)
            .map(|value| (value * 31) as u8)
            .collect::<Vec<_>>();
        let plan = build_transfer("payload.bin", "application/octet-stream", &data, false)
            .expect("transfer should build");
        let routes = hybrid_packet_routes(&plan.packets);
        assert!(routes.iter().all(|route| *route == PacketPhy::Ofdm));

        let fsk = AcousticProfile::fast(0);
        let ofdm = OfdmProfile::qpsk(0);
        let mut decoder = HybridDecoder::new(ofdm);
        let mut receiver = TransferReceiver::new();
        let mut completed = None;
        for (packet, route) in plan.packets.iter().zip(routes) {
            let waveform = encode_hybrid_packet(packet, route, fsk, ofdm);
            decoder.push(&waveform);
            while let Some(decoded) = decoder.poll() {
                for event in receiver.ingest(&decoded) {
                    if let TransferEvent::Completed { data, .. } = event {
                        completed = Some(data);
                    }
                }
            }
        }
        assert_eq!(completed.as_deref(), Some(data.as_slice()));
    }

    #[test]
    fn hybrid_decoder_resamples_streaming_input() {
        let profile = OfdmProfile::qpsk(0);
        let payload = vec![0x6D; 211];
        let waveform = encode_ofdm(&payload, profile);
        let mut at_44100 = Vec::new();
        let ratio = 44_100.0 / ENCODE_SR as f64;
        let output_len = (waveform.len() as f64 * ratio).floor() as usize;
        for index in 0..output_len {
            let source = index as f64 / ratio;
            let source_index = source.floor() as usize;
            let fraction = (source - source_index as f64) as f32;
            let first = waveform[source_index.min(waveform.len() - 1)];
            let second = waveform[(source_index + 1).min(waveform.len() - 1)];
            at_44100.push(first + (second - first) * fraction);
        }

        let mut decoder = HybridDecoder::with_sample_rate(44_100, profile);
        for chunk in at_44100.chunks(701) {
            decoder.push(chunk);
        }
        assert_eq!(decoder.poll(), Some(payload));
    }
}
