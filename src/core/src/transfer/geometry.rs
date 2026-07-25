pub const SHARD_SIZE: usize = 192;
pub const MAX_DATA_SHARDS: usize = 8;
pub const MAX_PARITY_SHARDS: usize = 4;

pub(super) fn shard_geometry(encoded_size: u64) -> (usize, usize) {
    match encoded_size.div_ceil(SHARD_SIZE as u64) {
        0 | 1 => (1, 2),
        2 => (2, 2),
        3 | 4 => (4, 3),
        _ => (MAX_DATA_SHARDS, MAX_PARITY_SHARDS),
    }
}
