//! Reed-Solomon block coding used by the acoustic frame protocol.

use reed_solomon::{Decoder, Encoder};

pub const ECC_BYTES: usize = 32;
pub const MAX_DATA_BYTES: usize = 255 - ECC_BYTES;
/// Stronger code for the OFDM body: corrects up to 32 byte errors per block,
/// which is needed to survive a wide notch in the speaker/microphone response.
pub const OFDM_ECC_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeError;

fn max_data(ecc: usize) -> usize {
    255 - ecc
}

/// Encode arbitrary data as independent RS(255, 255-ecc) blocks.
pub fn encode_blocks_ecc(data: &[u8], ecc: usize) -> Vec<u8> {
    let max_data = max_data(ecc);
    let encoder = Encoder::new(ecc);
    let blocks = data.len().div_ceil(max_data);
    let encoded_blocks: Vec<Vec<u8>> = data
        .chunks(max_data)
        .map(|chunk| encoder.encode(chunk).to_vec())
        .collect();
    let max_len = encoded_blocks.iter().map(Vec::len).max().unwrap_or(0);
    let mut out = Vec::with_capacity(data.len() + blocks * ecc);

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

/// Encode arbitrary data as independent RS(255, 223) blocks.
pub fn encode_blocks(data: &[u8]) -> Vec<u8> {
    encode_blocks_ecc(data, ECC_BYTES)
}

/// Decode RS blocks whose combined original data length is `data_len`.
/// Returns corrected data and the number of corrected bytes.
pub fn decode_blocks(encoded: &[u8], data_len: usize) -> Result<(Vec<u8>, usize), DecodeError> {
    decode_blocks_ecc(encoded, data_len, ECC_BYTES)
}

/// Decode RS(255, 255-ecc) blocks whose combined original length is `data_len`.
pub fn decode_blocks_ecc(
    encoded: &[u8],
    data_len: usize,
    ecc: usize,
) -> Result<(Vec<u8>, usize), DecodeError> {
    let max_data = max_data(ecc);
    let decoder = Decoder::new(ecc);
    let block_data_lengths: Vec<usize> = (0..data_len.div_ceil(max_data))
        .map(|index| (data_len - index * max_data).min(max_data))
        .collect();
    let block_lengths: Vec<usize> = block_data_lengths.iter().map(|&len| len + ecc).collect();
    let mut blocks: Vec<Vec<u8>> = block_lengths
        .iter()
        .map(|&len| Vec::with_capacity(len))
        .collect();
    let max_len = block_lengths.iter().copied().max().unwrap_or(0);
    let mut input = encoded.iter().copied();
    for byte_index in 0..max_len {
        for (block, &length) in blocks.iter_mut().zip(&block_lengths) {
            if byte_index < length {
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
    encoded_len_ecc(data_len, ECC_BYTES)
}

pub fn encoded_len_ecc(data_len: usize, ecc: usize) -> usize {
    if data_len == 0 {
        0
    } else {
        data_len + data_len.div_ceil(max_data(ecc)) * ecc
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
