//! Typed, chunked transfers on top of the acoustic link frames.

mod geometry;
mod metadata;
mod packet;
mod receiver;
mod sender;
mod types;

pub use geometry::{MAX_DATA_SHARDS, MAX_PARITY_SHARDS, SHARD_SIZE};
pub use metadata::{detect_content_type, is_chat_content_type, safe_file_name};
pub use packet::{classify_packet, TransferPacketKind};
pub use receiver::TransferReceiver;
pub use sender::build_transfer;
pub use types::{Compression, TransferError, TransferEvent, TransferMetadata, TransferPlan};

#[cfg(test)]
mod tests;
