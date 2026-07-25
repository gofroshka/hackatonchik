use std::path::Path;

use sonic_share_core::output::save_received;
use sonic_share_core::transfer::{is_chat_content_type, TransferEvent, TransferReceiver};
use sonic_share_core::Decoder;

use super::types::MobileReceiveEvent;

#[flutter_rust_bridge::frb(opaque)]
pub struct RxSession {
    decoder: Decoder,
    transfers: TransferReceiver,
    output_dir: String,
}

impl RxSession {
    pub fn new(sample_rate: u32, output_dir: String) -> Self {
        Self {
            decoder: Decoder::new(sample_rate),
            transfers: TransferReceiver::new(),
            output_dir,
        }
    }

    pub fn push_pcm16(&mut self, pcm16_le: Vec<u8>) -> Vec<MobileReceiveEvent> {
        let samples: Vec<f32> = pcm16_le
            .chunks_exact(2)
            .map(|bytes| i16::from_le_bytes([bytes[0], bytes[1]]) as f32 / i16::MAX as f32)
            .collect();
        self.decoder.push(&samples);
        let mut events = Vec::new();
        while let Some(packet) = self.decoder.poll() {
            for event in self.transfers.ingest(&packet) {
                events.push(self.map_event(event));
            }
        }
        events
    }

    fn map_event(&self, event: TransferEvent) -> MobileReceiveEvent {
        match event {
            TransferEvent::Started(metadata) => MobileReceiveEvent {
                kind: "started".to_owned(),
                id: format!("{:016x}", metadata.id),
                name: Some(metadata.name),
                content_type: Some(metadata.content_type),
                path: None,
                text: None,
                message: None,
                original_size: Some(metadata.original_size),
                completed_groups: Some(0),
                total_groups: Some(metadata.group_count),
            },
            TransferEvent::Progress {
                id,
                completed_groups,
                total_groups,
            } => MobileReceiveEvent {
                kind: "progress".to_owned(),
                id: format!("{id:016x}"),
                name: None,
                content_type: None,
                path: None,
                text: None,
                message: None,
                original_size: None,
                completed_groups: Some(completed_groups),
                total_groups: Some(total_groups),
            },
            TransferEvent::Completed { metadata, data } => {
                let text = if is_text(&metadata.content_type) {
                    std::str::from_utf8(&data).ok().map(str::to_owned)
                } else {
                    None
                };
                if is_chat_content_type(&metadata.content_type) {
                    return MobileReceiveEvent {
                        kind: "completed".to_owned(),
                        id: format!("{:016x}", metadata.id),
                        name: Some(metadata.name),
                        content_type: Some(metadata.content_type),
                        path: None,
                        text,
                        message: Some("SHA-256 verified".to_owned()),
                        original_size: Some(metadata.original_size),
                        completed_groups: Some(metadata.group_count),
                        total_groups: Some(metadata.group_count),
                    };
                }
                let saved = save_received(Path::new(&self.output_dir), &metadata, &data);
                match saved {
                    Ok(path) => MobileReceiveEvent {
                        kind: "completed".to_owned(),
                        id: format!("{:016x}", metadata.id),
                        name: Some(metadata.name),
                        content_type: Some(metadata.content_type),
                        path: Some(path.to_string_lossy().into_owned()),
                        text,
                        message: Some("SHA-256 verified".to_owned()),
                        original_size: Some(metadata.original_size),
                        completed_groups: Some(metadata.group_count),
                        total_groups: Some(metadata.group_count),
                    },
                    Err(error) => failed_event(metadata.id, format!("cannot save file: {error}")),
                }
            }
            TransferEvent::Failed { id, reason } => failed_event(id, reason),
        }
    }
}

fn failed_event(id: u64, reason: String) -> MobileReceiveEvent {
    MobileReceiveEvent {
        kind: "failed".to_owned(),
        id: format!("{id:016x}"),
        name: None,
        content_type: None,
        path: None,
        text: None,
        message: Some(reason),
        original_size: None,
        completed_groups: None,
        total_groups: None,
    }
}

fn is_text(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || content_type.starts_with("application/json")
        || content_type.contains("+json")
        || content_type.starts_with("application/xml")
        || content_type.contains("+xml")
}
