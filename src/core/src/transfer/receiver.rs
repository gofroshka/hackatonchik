use std::collections::HashMap;
use std::io::Cursor;

use reed_solomon_erasure::galois_8::ReedSolomon;
use sha2::{Digest, Sha256};

use super::geometry::{shard_geometry, SHARD_SIZE};
use super::packet::{decode_packet, Packet};
use super::types::{Compression, TransferEvent, TransferMetadata};

const MAX_PENDING_TRANSFERS: usize = 16;
const MAX_PENDING_SHARDS_PER_TRANSFER: usize = 96;

struct PendingShard {
    group: u32,
    index: u8,
    data: Vec<u8>,
}

#[derive(Debug)]
struct IncomingTransfer {
    metadata: TransferMetadata,
    groups: HashMap<u32, Vec<Option<Vec<u8>>>>,
    completed: HashMap<u32, Vec<u8>>,
}

#[derive(Default)]
pub struct TransferReceiver {
    incoming: HashMap<u64, IncomingTransfer>,
    pending: HashMap<u64, Vec<PendingShard>>,
}

impl TransferReceiver {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn ingest(&mut self, payload: &[u8]) -> Vec<TransferEvent> {
        let packet = match decode_packet(payload) {
            Ok(packet) => packet,
            Err(_) => return Vec::new(),
        };
        match packet {
            Packet::Manifest(metadata) => self.ingest_manifest(metadata),
            Packet::Shard {
                id,
                group,
                index,
                data,
            } => self.ingest_shard(id, group, index, data),
            Packet::End { id, sha256 } => self.ingest_end(id, sha256),
            Packet::Inline { metadata, encoded } => {
                let id = metadata.id;
                match decode_content(&metadata, encoded) {
                    Ok(data) => vec![
                        TransferEvent::Started(metadata.clone()),
                        TransferEvent::Completed { metadata, data },
                    ],
                    Err(reason) => vec![TransferEvent::Failed { id, reason }],
                }
            }
        }
    }

    fn ingest_manifest(&mut self, metadata: TransferMetadata) -> Vec<TransferEvent> {
        if let Some(existing) = self.incoming.get(&metadata.id) {
            if existing.metadata == metadata {
                return Vec::new();
            }
            self.incoming.remove(&metadata.id);
        }
        self.incoming.insert(
            metadata.id,
            IncomingTransfer {
                metadata: metadata.clone(),
                groups: HashMap::new(),
                completed: HashMap::new(),
            },
        );
        let id = metadata.id;
        let mut events = vec![TransferEvent::Started(metadata)];
        if let Some(pending) = self.pending.remove(&id) {
            for shard in pending {
                events.extend(self.ingest_shard(id, shard.group, shard.index, shard.data));
            }
        }
        events
    }

    fn ingest_shard(
        &mut self,
        id: u64,
        group: u32,
        index: u8,
        data: Vec<u8>,
    ) -> Vec<TransferEvent> {
        let Some(transfer) = self.incoming.get_mut(&id) else {
            if self.pending.contains_key(&id) || self.pending.len() < MAX_PENDING_TRANSFERS {
                let pending = self.pending.entry(id).or_default();
                if pending.len() < MAX_PENDING_SHARDS_PER_TRANSFER
                    && !pending
                        .iter()
                        .any(|shard| shard.group == group && shard.index == index)
                {
                    pending.push(PendingShard { group, index, data });
                }
            }
            return Vec::new();
        };
        let (data_shards, parity_shards) = shard_geometry(transfer.metadata.encoded_size);
        if group >= transfer.metadata.group_count || index as usize >= data_shards + parity_shards {
            return Vec::new();
        }
        if transfer.completed.contains_key(&group) {
            return Vec::new();
        }

        let shards = transfer
            .groups
            .entry(group)
            .or_insert_with(|| vec![None; data_shards + parity_shards]);
        if shards[index as usize].is_some() {
            return Vec::new();
        }
        shards[index as usize] = Some(data);
        if shards.iter().flatten().count() < data_shards {
            return Vec::new();
        }

        let fec = match ReedSolomon::new(data_shards, parity_shards) {
            Ok(fec) => fec,
            Err(_) => return Vec::new(),
        };
        if fec.reconstruct(shards).is_err() {
            return Vec::new();
        }
        let mut group_data = Vec::with_capacity(data_shards * SHARD_SIZE);
        for shard in shards.iter().take(data_shards) {
            let Some(shard) = shard else {
                return Vec::new();
            };
            group_data.extend_from_slice(shard);
        }
        transfer.groups.remove(&group);
        transfer.completed.insert(group, group_data);
        let completed_groups = transfer.completed.len() as u32;
        let total_groups = transfer.metadata.group_count;
        let mut events = vec![TransferEvent::Progress {
            id,
            completed_groups,
            total_groups,
        }];
        if completed_groups == total_groups {
            events.extend(self.finish(id));
        }
        events
    }

    fn ingest_end(&mut self, id: u64, sha256: [u8; 32]) -> Vec<TransferEvent> {
        let Some(transfer) = self.incoming.get_mut(&id) else {
            return Vec::new();
        };
        if transfer.metadata.sha256 != sha256 {
            return vec![TransferEvent::Failed {
                id,
                reason: "end digest does not match manifest".to_owned(),
            }];
        }
        if transfer.completed.len() as u32 == transfer.metadata.group_count {
            self.finish(id)
        } else {
            Vec::new()
        }
    }

    fn finish(&mut self, id: u64) -> Vec<TransferEvent> {
        let Some(mut transfer) = self.incoming.remove(&id) else {
            return Vec::new();
        };
        let mut encoded = Vec::with_capacity(transfer.metadata.encoded_size as usize);
        for group in 0..transfer.metadata.group_count {
            let Some(data) = transfer.completed.remove(&group) else {
                self.incoming.insert(id, transfer);
                return Vec::new();
            };
            encoded.extend_from_slice(&data);
        }
        let encoded_size = match usize::try_from(transfer.metadata.encoded_size) {
            Ok(size) if size <= encoded.len() => size,
            _ => {
                return vec![TransferEvent::Failed {
                    id,
                    reason: "encoded size is invalid".to_owned(),
                }];
            }
        };
        encoded.truncate(encoded_size);
        match decode_content(&transfer.metadata, encoded) {
            Ok(data) => vec![TransferEvent::Completed {
                metadata: transfer.metadata,
                data,
            }],
            Err(reason) => vec![TransferEvent::Failed { id, reason }],
        }
    }
}

fn decode_content(metadata: &TransferMetadata, encoded: Vec<u8>) -> Result<Vec<u8>, String> {
    let data = match metadata.compression {
        Compression::None => encoded,
        Compression::Zstd => zstd::stream::decode_all(Cursor::new(encoded))
            .map_err(|error| format!("zstd decompression failed: {error}"))?,
    };
    if data.len() as u64 != metadata.original_size
        || <[u8; 32]>::from(Sha256::digest(&data)) != metadata.sha256
    {
        return Err("SHA-256 verification failed".to_owned());
    }
    Ok(data)
}
