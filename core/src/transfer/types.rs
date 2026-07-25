use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    None,
    Zstd,
}

impl Compression {
    pub(super) fn to_byte(self) -> u8 {
        match self {
            Self::None => 0,
            Self::Zstd => 1,
        }
    }

    pub(super) fn from_byte(value: u8) -> Result<Self, TransferError> {
        match value {
            0 => Ok(Self::None),
            1 => Ok(Self::Zstd),
            _ => Err(TransferError::Malformed("unknown compression")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferMetadata {
    pub id: u64,
    pub name: String,
    pub content_type: String,
    pub original_size: u64,
    pub encoded_size: u64,
    pub sha256: [u8; 32],
    pub compression: Compression,
    pub group_count: u32,
}

#[derive(Debug)]
pub struct TransferPlan {
    pub metadata: TransferMetadata,
    pub packets: Vec<Vec<u8>>,
}

#[derive(Debug)]
pub enum TransferEvent {
    Started(TransferMetadata),
    Progress {
        id: u64,
        completed_groups: u32,
        total_groups: u32,
    },
    Completed {
        metadata: TransferMetadata,
        data: Vec<u8>,
    },
    Failed {
        id: u64,
        reason: String,
    },
}

#[derive(Debug)]
pub enum TransferError {
    TooLarge,
    MetadataTooLarge,
    Malformed(&'static str),
    Compression(std::io::Error),
    Fec,
}

impl fmt::Display for TransferError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooLarge => write!(formatter, "transfer is too large"),
            Self::MetadataTooLarge => {
                write!(formatter, "file name or content type is too long")
            }
            Self::Malformed(reason) => write!(formatter, "malformed transfer packet: {reason}"),
            Self::Compression(error) => write!(formatter, "compression error: {error}"),
            Self::Fec => write!(formatter, "outer FEC operation failed"),
        }
    }
}

impl std::error::Error for TransferError {}
