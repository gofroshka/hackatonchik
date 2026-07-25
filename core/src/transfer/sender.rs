use std::io::Cursor;

use reed_solomon_erasure::galois_8::ReedSolomon;
use sha2::{Digest, Sha256};

use crate::MAX_PAYLOAD;

use super::geometry::{shard_geometry, SHARD_SIZE};
use super::metadata::{safe_file_name, transfer_id, truncate_utf8};
use super::packet::{
    encode_end, encode_inline, encode_manifest, encode_shard, INLINE_FIXED, MANIFEST_FIXED,
};
use super::types::{Compression, TransferError, TransferMetadata, TransferPlan};

const MANIFEST_REPEATS: usize = 2;
const END_REPEATS: usize = 1;

pub fn build_transfer(
    name: &str,
    content_type: &str,
    data: &[u8],
    allow_compression: bool,
) -> Result<TransferPlan, TransferError> {
    let name = safe_file_name(name);
    let content_type = truncate_utf8(content_type, 52);
    if MANIFEST_FIXED + name.len() + content_type.len() > MAX_PAYLOAD {
        return Err(TransferError::MetadataTooLarge);
    }

    let compressed = if allow_compression && data.len() >= 128 {
        Some(zstd::stream::encode_all(Cursor::new(data), 3).map_err(TransferError::Compression)?)
    } else {
        None
    };
    let (encoded_data, compression) = match compressed {
        Some(value) if value.len() + 32 < data.len() => (value, Compression::Zstd),
        _ => (data.to_vec(), Compression::None),
    };

    let sha256: [u8; 32] = Sha256::digest(data).into();
    let id = transfer_id(&sha256);
    let mut metadata = TransferMetadata {
        id,
        name,
        content_type,
        original_size: data.len() as u64,
        encoded_size: encoded_data.len() as u64,
        sha256,
        compression,
        group_count: 0,
    };

    if INLINE_FIXED + metadata.name.len() + metadata.content_type.len() + encoded_data.len()
        <= MAX_PAYLOAD
    {
        let packet = encode_inline(&metadata, &encoded_data)?;
        return Ok(TransferPlan {
            metadata,
            packets: vec![packet],
        });
    }

    let (data_shards, parity_shards) = shard_geometry(encoded_data.len() as u64);
    let group_bytes = data_shards * SHARD_SIZE;
    let groups = encoded_data.len().div_ceil(group_bytes);
    metadata.group_count = u32::try_from(groups).map_err(|_| TransferError::TooLarge)?;

    let manifest = encode_manifest(&metadata)?;
    let end = encode_end(&metadata);
    let fec = ReedSolomon::new(data_shards, parity_shards).map_err(|_| TransferError::Fec)?;
    let total_shards = data_shards + parity_shards;
    let mut packets = Vec::with_capacity(MANIFEST_REPEATS + groups * total_shards + END_REPEATS);
    packets.extend(std::iter::repeat_n(manifest.clone(), MANIFEST_REPEATS));

    for group in 0..groups {
        let mut shards = vec![vec![0u8; SHARD_SIZE]; total_shards];
        let group_start = group * group_bytes;
        for (index, shard) in shards.iter_mut().take(data_shards).enumerate() {
            let start = group_start + index * SHARD_SIZE;
            if start >= encoded_data.len() {
                break;
            }
            let end = (start + SHARD_SIZE).min(encoded_data.len());
            shard[..end - start].copy_from_slice(&encoded_data[start..end]);
        }
        fec.encode(&mut shards).map_err(|_| TransferError::Fec)?;
        for (index, shard) in shards.iter().enumerate() {
            packets.push(encode_shard(id, group as u32, index as u8, shard));
        }
        if (group + 1) % 8 == 0 {
            packets.push(manifest.clone());
        }
    }
    packets.extend(std::iter::repeat_n(end, END_REPEATS));
    debug_assert!(packets.iter().all(|packet| packet.len() <= MAX_PAYLOAD));
    Ok(TransferPlan { metadata, packets })
}
