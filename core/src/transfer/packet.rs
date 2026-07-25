use crate::MAX_PAYLOAD;

use super::geometry::{shard_geometry, SHARD_SIZE};
use super::metadata::safe_file_name;
use super::types::{Compression, TransferError, TransferMetadata};

const MAGIC: &[u8; 4] = b"ASND";
const VERSION: u8 = 1;
const TYPE_MANIFEST: u8 = 1;
const TYPE_SHARD: u8 = 2;
const TYPE_END: u8 = 3;
const TYPE_INLINE: u8 = 4;
pub const TYPE_HANDSHAKE_REQ: u8 = 5;
pub const TYPE_HANDSHAKE_ACK: u8 = 6;
pub const TYPE_END_ACK: u8 = 7;
const COMMON_HEADER: usize = 4 + 1 + 1 + 8;
pub(super) const MANIFEST_FIXED: usize = COMMON_HEADER + 1 + 8 + 8 + 32 + 2 + 1 + 1 + 4 + 1 + 1;
pub(super) const INLINE_FIXED: usize = COMMON_HEADER + 1 + 8 + 8 + 32 + 1 + 1;

#[derive(Debug)]
pub(super) enum Packet {
    Manifest(TransferMetadata),
    Shard {
        id: u64,
        group: u32,
        index: u8,
        data: Vec<u8>,
    },
    End {
        id: u64,
        sha256: [u8; 32],
    },
    Inline {
        metadata: TransferMetadata,
        encoded: Vec<u8>,
    },
    HandshakeReq {
        id: u64,
    },
    HandshakeAck {
        id: u64,
    },
    EndAck {
        id: u64,
    },
}

fn encode_common(kind: u8, id: u64, output: &mut Vec<u8>) {
    output.extend_from_slice(MAGIC);
    output.push(VERSION);
    output.push(kind);
    output.extend_from_slice(&id.to_le_bytes());
}

pub(super) fn encode_manifest(metadata: &TransferMetadata) -> Result<Vec<u8>, TransferError> {
    let name = metadata.name.as_bytes();
    let content_type = metadata.content_type.as_bytes();
    if name.len() > u8::MAX as usize
        || content_type.len() > u8::MAX as usize
        || MANIFEST_FIXED + name.len() + content_type.len() > MAX_PAYLOAD
    {
        return Err(TransferError::MetadataTooLarge);
    }
    let mut output = Vec::with_capacity(MANIFEST_FIXED + name.len() + content_type.len());
    encode_common(TYPE_MANIFEST, metadata.id, &mut output);
    output.push(metadata.compression.to_byte());
    output.extend_from_slice(&metadata.original_size.to_le_bytes());
    output.extend_from_slice(&metadata.encoded_size.to_le_bytes());
    output.extend_from_slice(&metadata.sha256);
    output.extend_from_slice(&(SHARD_SIZE as u16).to_le_bytes());
    let (data_shards, parity_shards) = shard_geometry(metadata.encoded_size);
    output.push(data_shards as u8);
    output.push(parity_shards as u8);
    output.extend_from_slice(&metadata.group_count.to_le_bytes());
    output.push(name.len() as u8);
    output.push(content_type.len() as u8);
    output.extend_from_slice(name);
    output.extend_from_slice(content_type);
    Ok(output)
}

pub(super) fn encode_shard(id: u64, group: u32, index: u8, data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(COMMON_HEADER + 4 + 1 + SHARD_SIZE);
    encode_common(TYPE_SHARD, id, &mut output);
    output.extend_from_slice(&group.to_le_bytes());
    output.push(index);
    output.extend_from_slice(data);
    output
}

pub(super) fn encode_end(metadata: &TransferMetadata) -> Vec<u8> {
    let mut output = Vec::with_capacity(COMMON_HEADER + 32);
    encode_common(TYPE_END, metadata.id, &mut output);
    output.extend_from_slice(&metadata.sha256);
    output
}

pub fn encode_handshake_req(id: u64) -> Vec<u8> {
    let mut output = Vec::with_capacity(COMMON_HEADER);
    encode_common(TYPE_HANDSHAKE_REQ, id, &mut output);
    output
}

pub fn encode_handshake_ack(id: u64) -> Vec<u8> {
    let mut output = Vec::with_capacity(COMMON_HEADER);
    encode_common(TYPE_HANDSHAKE_ACK, id, &mut output);
    output
}

pub fn encode_end_ack(id: u64) -> Vec<u8> {
    let mut output = Vec::with_capacity(COMMON_HEADER);
    encode_common(TYPE_END_ACK, id, &mut output);
    output
}

pub(super) fn encode_inline(
    metadata: &TransferMetadata,
    encoded: &[u8],
) -> Result<Vec<u8>, TransferError> {
    let name = metadata.name.as_bytes();
    let content_type = metadata.content_type.as_bytes();
    let capacity = INLINE_FIXED + name.len() + content_type.len() + encoded.len();
    if capacity > MAX_PAYLOAD {
        return Err(TransferError::MetadataTooLarge);
    }
    let mut output = Vec::with_capacity(capacity);
    encode_common(TYPE_INLINE, metadata.id, &mut output);
    output.push(metadata.compression.to_byte());
    output.extend_from_slice(&metadata.original_size.to_le_bytes());
    output.extend_from_slice(&metadata.encoded_size.to_le_bytes());
    output.extend_from_slice(&metadata.sha256);
    output.push(name.len() as u8);
    output.push(content_type.len() as u8);
    output.extend_from_slice(name);
    output.extend_from_slice(content_type);
    output.extend_from_slice(encoded);
    Ok(output)
}

pub(super) fn decode_packet(data: &[u8]) -> Result<Packet, TransferError> {
    let mut cursor = SliceCursor::new(data);
    if cursor.take(4)? != MAGIC {
        return Err(TransferError::Malformed("bad magic"));
    }
    if cursor.u8()? != VERSION {
        return Err(TransferError::Malformed("unsupported version"));
    }
    let kind = cursor.u8()?;
    let id = cursor.u64()?;
    match kind {
        TYPE_MANIFEST => {
            let compression = Compression::from_byte(cursor.u8()?)?;
            let original_size = cursor.u64()?;
            let encoded_size = cursor.u64()?;
            let sha256 = cursor
                .take(32)?
                .try_into()
                .map_err(|_| TransferError::Malformed("bad digest"))?;
            let shard_size = cursor.u16()? as usize;
            let data_shards = cursor.u8()? as usize;
            let parity_shards = cursor.u8()? as usize;
            if shard_size != SHARD_SIZE
                || (data_shards, parity_shards) != shard_geometry(encoded_size)
            {
                return Err(TransferError::Malformed("unsupported shard geometry"));
            }
            let group_count = cursor.u32()?;
            let expected_groups = encoded_size.div_ceil(data_shards as u64 * SHARD_SIZE as u64);
            if group_count == 0 || u64::from(group_count) != expected_groups {
                return Err(TransferError::Malformed("invalid group count"));
            }
            let name_len = cursor.u8()? as usize;
            let content_type_len = cursor.u8()? as usize;
            let name = std::str::from_utf8(cursor.take(name_len)?)
                .map_err(|_| TransferError::Malformed("file name is not UTF-8"))?;
            let content_type = std::str::from_utf8(cursor.take(content_type_len)?)
                .map_err(|_| TransferError::Malformed("content type is not UTF-8"))?;
            cursor.finish()?;
            Ok(Packet::Manifest(TransferMetadata {
                id,
                name: safe_file_name(name),
                content_type: content_type.to_owned(),
                original_size,
                encoded_size,
                sha256,
                compression,
                group_count,
            }))
        }
        TYPE_SHARD => {
            let group = cursor.u32()?;
            let index = cursor.u8()?;
            let shard = cursor.take(SHARD_SIZE)?.to_vec();
            cursor.finish()?;
            Ok(Packet::Shard {
                id,
                group,
                index,
                data: shard,
            })
        }
        TYPE_END => {
            let sha256 = cursor
                .take(32)?
                .try_into()
                .map_err(|_| TransferError::Malformed("bad digest"))?;
            cursor.finish()?;
            Ok(Packet::End { id, sha256 })
        }
            TYPE_INLINE => {
                let compression = Compression::from_byte(cursor.u8()?)?;
                let original_size = cursor.u64()?;
                let encoded_size = cursor.u64()?;
                let sha256 = cursor
                    .take(32)?
                    .try_into()
                    .map_err(|_| TransferError::Malformed("bad digest"))?;
                let name_len = cursor.u8()? as usize;
                let content_type_len = cursor.u8()? as usize;
                let name = std::str::from_utf8(cursor.take(name_len)?)
                    .map_err(|_| TransferError::Malformed("file name is not UTF-8"))?;
                let content_type = std::str::from_utf8(cursor.take(content_type_len)?)
                    .map_err(|_| TransferError::Malformed("content type is not UTF-8"))?;
                let encoded_len = usize::try_from(encoded_size)
                    .map_err(|_| TransferError::Malformed("encoded size overflow"))?;
                let encoded = cursor.take(encoded_len)?.to_vec();
                cursor.finish()?;
                Ok(Packet::Inline {
                    metadata: TransferMetadata {
                        id,
                        name: safe_file_name(name),
                        content_type: content_type.to_owned(),
                        original_size,
                        encoded_size,
                        sha256,
                        compression,
                        group_count: 0,
                    },
                    encoded,
                })
            }
            TYPE_HANDSHAKE_REQ => {
                cursor.finish()?;
                Ok(Packet::HandshakeReq { id })
            }
            TYPE_HANDSHAKE_ACK => {
                cursor.finish()?;
                Ok(Packet::HandshakeAck { id })
            }
            TYPE_END_ACK => {
                cursor.finish()?;
                Ok(Packet::EndAck { id })
            }
            _ => Err(TransferError::Malformed("unknown packet type")),
    }
}

struct SliceCursor<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> SliceCursor<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, offset: 0 }
    }

    fn take(&mut self, length: usize) -> Result<&'a [u8], TransferError> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(TransferError::Malformed("length overflow"))?;
        let result = self
            .data
            .get(self.offset..end)
            .ok_or(TransferError::Malformed("truncated packet"))?;
        self.offset = end;
        Ok(result)
    }

    fn u8(&mut self) -> Result<u8, TransferError> {
        Ok(self.take(1)?[0])
    }

    fn u16(&mut self) -> Result<u16, TransferError> {
        Ok(u16::from_le_bytes(
            self.take(2)?.try_into().expect("two-byte slice"),
        ))
    }

    fn u32(&mut self) -> Result<u32, TransferError> {
        Ok(u32::from_le_bytes(
            self.take(4)?.try_into().expect("four-byte slice"),
        ))
    }

    fn u64(&mut self) -> Result<u64, TransferError> {
        Ok(u64::from_le_bytes(
            self.take(8)?.try_into().expect("eight-byte slice"),
        ))
    }

    fn finish(&self) -> Result<(), TransferError> {
        if self.offset == self.data.len() {
            Ok(())
        } else {
            Err(TransferError::Malformed("trailing bytes"))
        }
    }
}
