//! Reed-Solomon block coding used by the acoustic frame protocol.

use reed_solomon::{Decoder, Encoder};

pub const ECC_BYTES: usize = 32;
pub const MAX_DATA_BYTES: usize = 255 - ECC_BYTES;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError;

/// Encode arbitrary data as independent RS(255, 223) blocks.
pub fn encode_blocks(data: &[u8]) -> Vec<u8> {
    let encoder = Encoder::new(ECC_BYTES);
    let blocks = data.len().div_ceil(MAX_DATA_BYTES);
    let encoded_blocks: Vec<Vec<u8>> = data
        .chunks(MAX_DATA_BYTES)
        .map(|chunk| encoder.encode(chunk).to_vec())
        .collect();
    let max_len = encoded_blocks.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = Vec::with_capacity(data.len() + blocks * ECC_BYTES);

    // Interleave blocks so a contiguous acoustic noise burst is distributed
    // across codewords instead of overwhelming one block's correction budget.
    for index in 0..max_len {
        for block in &encoded_blocks {
            if let Some(&byte) = block.get(index) {
                out.push(byte);
            }
        }
    }
    out
}

/// Decode RS blocks whose combined original data length is `data_len`.
/// Returns corrected data and the number of corrected bytes.
pub fn decode_blocks(encoded: &[u8], data_len: usize) -> Result<(Vec<u8>, usize), DecodeError> {
    let decoder = Decoder::new(ECC_BYTES);
    let block_data_lengths: Vec<usize> = (0..data_len.div_ceil(MAX_DATA_BYTES))
        .map(|index| (data_len - index * MAX_DATA_BYTES).min(MAX_DATA_BYTES))
        .collect();
    let block_lengths: Vec<usize> = block_data_lengths
        .iter()
        .map(|&len| len + ECC_BYTES)
        .collect();
    let mut blocks: Vec<Vec<u8>> = block_lengths
        .iter()
        .map(|&len| Vec::with_capacity(len))
        .collect();
    let max_len = block_lengths.iter().copied().max().unwrap_or(0);
    let mut input = encoded.iter().copied();
    for index in 0..max_len {
        for (block, &length) in blocks.iter_mut().zip(&block_lengths) {
            if index < length {
                block.push(input.next().ok_or(DecodeError)?);
            }
        }
    }
    if input.next().is_some() {
        return Err(DecodeError);
    }

    let mut corrected_count = 0usize;
    let mut out = Vec::with_capacity(data_len);

    for (block, &block_data_len) in blocks.iter().zip(&block_data_lengths) {
        let (corrected, count) = decoder
            .correct_err_count(block, None)
            .map_err(|_| DecodeError)?;
        out.extend_from_slice(corrected.data());
        corrected_count += count;
        debug_assert_eq!(corrected.data().len(), block_data_len);
    }
    Ok((out, corrected_count))
}

pub fn encoded_len(data_len: usize) -> usize {
    if data_len == 0 {
        0
    } else {
        data_len + data_len.div_ceil(MAX_DATA_BYTES) * ECC_BYTES
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrects_sixteen_errors_per_block() {
        let data: Vec<u8> = (0..255u16).map(|x| (x * 7 + 3) as u8).collect();
        let mut encoded = encode_blocks(&data);

        // A contiguous 32-byte burst becomes 16 errors in each interleaved
        // block, exactly the correction limit.
        for (k, byte) in encoded.iter_mut().take(32).enumerate() {
            *byte ^= 0xA5 ^ k as u8;
        }

        let (decoded, corrected) = decode_blocks(&encoded, data.len()).expect("correctable");
        assert_eq!(decoded, data);
        assert_eq!(corrected, 32);
    }

    #[test]
    fn rejects_uncorrectable_block() {
        let data = vec![0x42; 100];
        let mut encoded = encode_blocks(&data);
        for k in 0..20 {
            encoded[k * 5] ^= 0x3C ^ k as u8;
        }
        assert!(decode_blocks(&encoded, data.len()).is_err());
    }
}
