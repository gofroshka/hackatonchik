use std::path::Path;

use sonic_share_core::transfer::{build_transfer, detect_content_type};
use sonic_share_core::tone;
use sonic_share_core::{
    encode, encoded_sample_count, ENCODE_SR, F_END_ACK, F_HANDSHAKE_ACK, F_HANDSHAKE_REQ,
    HANDSHAKE_TONE_SECS,
};

use super::types::{TxChunk, TxInfo};

#[flutter_rust_bridge::frb(opaque)]
pub struct TxSession {
    packets: Vec<Vec<u8>>,
    packet_index: usize,
    current_wave: Vec<f32>,
    current_offset: usize,
    emitted_samples: usize,
    total_samples: usize,
    cancelled: bool,
    info: TxInfo,
}

impl TxSession {
    pub fn from_file(path: String, content_type: Option<String>) -> Result<Self, String> {
        let data = std::fs::read(&path).map_err(|error| format!("cannot read file: {error}"))?;
        let name = Path::new(&path)
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or("file.bin")
            .to_owned();
        Self::from_data(name, content_type, data)
    }

    pub fn from_data(
        name: String,
        content_type: Option<String>,
        data: Vec<u8>,
    ) -> Result<Self, String> {
        let content_type = content_type.unwrap_or_else(|| detect_content_type(&name, &data));
        let plan = build_transfer(&name, &content_type, &data, true)
            .map_err(|error| format!("cannot create transfer: {error}"))?;
        let total_samples = plan
            .packets
            .iter()
            .map(|packet| encoded_sample_count(packet.len()))
            .sum::<usize>();
        let info = TxInfo {
            id: format!("{:016x}", plan.metadata.id),
            name: plan.metadata.name.clone(),
            content_type: plan.metadata.content_type.clone(),
            original_size: plan.metadata.original_size,
            encoded_size: plan.metadata.encoded_size,
            packet_count: plan.packets.len() as u32,
            estimated_seconds: total_samples as f64 / ENCODE_SR as f64,
        };
        Ok(Self {
            packets: plan.packets,
            packet_index: 0,
            current_wave: Vec::new(),
            current_offset: 0,
            emitted_samples: 0,
            total_samples,
            cancelled: false,
            info,
        })
    }

    pub fn info(&self) -> TxInfo {
        self.info.clone()
    }

    pub fn next_pcm_chunk(&mut self, max_samples: u32) -> TxChunk {
        let max_samples = (max_samples as usize).clamp(256, 65_536);
        let mut bytes = Vec::with_capacity(max_samples * 2);
        while bytes.len() < max_samples * 2 && !self.cancelled {
            if self.current_offset >= self.current_wave.len() {
                if self.packet_index >= self.packets.len() {
                    break;
                }
                self.current_wave = encode(&self.packets[self.packet_index]);
                self.current_offset = 0;
                self.packet_index += 1;
            }
            let remaining_samples = max_samples - bytes.len() / 2;
            let available = self.current_wave.len() - self.current_offset;
            let count = remaining_samples.min(available);
            for &sample in &self.current_wave[self.current_offset..self.current_offset + count] {
                let value = (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16;
                bytes.extend_from_slice(&value.to_le_bytes());
            }
            self.current_offset += count;
            self.emitted_samples += count;
        }
        let done = self.cancelled
            || (self.packet_index >= self.packets.len()
                && self.current_offset >= self.current_wave.len());
        TxChunk {
            pcm16_le: bytes,
            progress: if self.total_samples == 0 {
                1.0
            } else {
                self.emitted_samples as f64 / self.total_samples as f64
            }
            .clamp(0.0, 1.0),
            packet_index: self.packet_index as u32,
            packet_count: self.packets.len() as u32,
            done,
        }
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
        self.current_wave.clear();
    }
}

pub fn handshake_request_pcm(_id: u64) -> Vec<u8> {
    tone::generate_pcm16(HANDSHAKE_TONE_SECS, F_HANDSHAKE_REQ)
}

pub fn handshake_ack_pcm(_id: u64) -> Vec<u8> {
    tone::generate_pcm16(HANDSHAKE_TONE_SECS, F_HANDSHAKE_ACK)
}

pub fn end_ack_pcm(_id: u64) -> Vec<u8> {
    tone::generate_pcm16(HANDSHAKE_TONE_SECS, F_END_ACK)
}
