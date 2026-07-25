use crate::detector::{data_argmax, goertzel, COLS_PER_SYM};
use crate::protocol::{
    crc8, data_freq, majority_byte, symbol_len, tone_len, F_MARKER, LEN_REPEATS, SYNC_BYTE,
    SYNC_REPEATS,
};
use crate::rs;

enum Attempt {
    Decoded(Vec<u8>, usize),
    NeedMore,
    Invalid,
}

enum Lock {
    Decoded(Vec<u8>),
    Wait,
    None,
}

/// Incremental acoustic frame decoder.
pub struct Decoder {
    sample_rate: u32,
    hop: usize,
    tone_len: usize,
    frequencies: Vec<f32>,
    buffer: Vec<f32>,
    columns: Vec<[f32; 17]>,
    scan_column: usize,
    pending: Option<usize>,
}

impl Decoder {
    pub fn new(sample_rate: u32) -> Self {
        let symbol_len = symbol_len(sample_rate);
        let mut frequencies: Vec<f32> = (0..16u8).map(data_freq).collect();
        frequencies.push(F_MARKER);
        Self {
            sample_rate,
            hop: (symbol_len / COLS_PER_SYM).max(1),
            tone_len: tone_len(sample_rate),
            frequencies,
            buffer: Vec::new(),
            columns: Vec::new(),
            scan_column: 0,
            pending: None,
        }
    }

    pub fn push(&mut self, samples: &[f32]) {
        self.buffer.extend_from_slice(samples);
    }

    fn extend_spectrogram(&mut self) {
        while self.columns.len() * self.hop + self.tone_len <= self.buffer.len() {
            let start = self.columns.len() * self.hop;
            let window = &self.buffer[start..start + self.tone_len];
            let mut energies = [0.0f32; 17];
            for (index, &frequency) in self.frequencies.iter().enumerate() {
                energies[index] = goertzel(window, frequency, self.sample_rate);
            }
            self.columns.push(energies);
        }
    }

    fn read_symbol(&self, cursor: f32) -> Option<(u8, f32)> {
        let column = cursor.round() as i64;
        if column < 0 || column as usize >= self.columns.len() {
            return None;
        }
        let (nibble, _, _) = data_argmax(&self.columns[column as usize]);
        Some((nibble, column as f32 + COLS_PER_SYM as f32))
    }

    fn read_byte(&self, cursor: &mut f32) -> Option<u8> {
        let (high, next) = self.read_symbol(*cursor)?;
        let (low, next) = self.read_symbol(next)?;
        *cursor = next;
        Some((high << 4) | low)
    }

    fn try_decode(&self, start_column: usize) -> Attempt {
        let mut cursor = start_column as f32;
        let mut syncs = [0u8; SYNC_REPEATS];
        for sync in &mut syncs {
            *sync = match self.read_byte(&mut cursor) {
                Some(byte) => byte,
                None => return Attempt::NeedMore,
            };
        }
        if majority_byte(&syncs) != Some(SYNC_BYTE) {
            return Attempt::Invalid;
        }

        let mut lengths = [0u8; LEN_REPEATS];
        for length in &mut lengths {
            *length = match self.read_byte(&mut cursor) {
                Some(byte) => byte,
                None => return Attempt::NeedMore,
            };
        }
        let Some(length_byte) = majority_byte(&lengths) else {
            return Attempt::Invalid;
        };
        let data_len = length_byte as usize + 2;
        let mut encoded = Vec::with_capacity(rs::encoded_len(data_len));
        for _ in 0..rs::encoded_len(data_len) {
            match self.read_byte(&mut cursor) {
                Some(byte) => encoded.push(byte),
                None => return Attempt::NeedMore,
            }
        }
        let Ok((inner, _)) = rs::decode_blocks(&encoded, data_len) else {
            return Attempt::Invalid;
        };
        if inner.len() != data_len || inner[0] != length_byte {
            return Attempt::Invalid;
        }
        let crc_index = inner.len() - 1;
        if crc8(&inner[..crc_index]) != inner[crc_index] {
            return Attempt::Invalid;
        }
        Attempt::Decoded(inner[1..crc_index].to_vec(), cursor.round() as usize)
    }

    fn lock_frame(&mut self, transition: usize) -> Lock {
        let low = transition.saturating_sub(COLS_PER_SYM);
        let wanted_high = transition + 2 * COLS_PER_SYM;
        let last = self.columns.len().saturating_sub(1);
        let high = wanted_high.min(last);
        let truncated = wanted_high > last;
        let mut wait = false;
        for start in low..=high {
            match self.try_decode(start) {
                Attempt::Decoded(message, end_column) => {
                    let end_sample = (end_column * self.hop).min(self.buffer.len());
                    self.buffer.drain(0..end_sample);
                    self.columns.clear();
                    self.scan_column = 0;
                    self.pending = None;
                    return Lock::Decoded(message);
                }
                Attempt::NeedMore => wait = true,
                Attempt::Invalid => {}
            }
        }
        if wait || truncated {
            self.pending = Some(transition);
            Lock::Wait
        } else {
            Lock::None
        }
    }

    /// Scan for and decode the next complete frame.
    pub fn poll(&mut self) -> Option<Vec<u8>> {
        self.extend_spectrogram();
        let floor = 2e-4;

        if let Some(transition) = self.pending.take() {
            match self.lock_frame(transition) {
                Lock::Decoded(message) => return Some(message),
                Lock::Wait => return None,
                Lock::None => self.scan_column = transition + COLS_PER_SYM,
            }
        }

        let column_count = self.columns.len();
        let mut column = self.scan_column;
        while column < column_count {
            let marker = self.columns[column][16];
            if marker > floor {
                let (_, data_peak, _) = data_argmax(&self.columns[column]);
                if marker > data_peak * 2.0 {
                    let mut transition = column;
                    while transition < column_count {
                        let marker = self.columns[transition][16];
                        let (_, data_peak, _) = data_argmax(&self.columns[transition]);
                        if marker > floor && marker > data_peak * 1.5 {
                            transition += 1;
                        } else {
                            break;
                        }
                    }
                    if transition >= column_count || column_count < transition + 4 * COLS_PER_SYM {
                        self.scan_column = column;
                        return None;
                    }
                    match self.lock_frame(transition) {
                        Lock::Decoded(message) => return Some(message),
                        Lock::Wait => return None,
                        Lock::None => {
                            self.scan_column = transition + COLS_PER_SYM;
                            column = self.scan_column;
                            continue;
                        }
                    }
                }
            }
            column += 1;
        }
        self.scan_column = column_count;

        if self.pending.is_none() {
            let max_keep = self.sample_rate as usize * 8;
            if self.buffer.len() > max_keep {
                let drop_columns = (self.buffer.len() - max_keep) / self.hop;
                if drop_columns > 0 {
                    self.buffer.drain(0..drop_columns * self.hop);
                    self.columns.drain(0..drop_columns.min(self.columns.len()));
                    self.scan_column = self.scan_column.saturating_sub(drop_columns);
                }
            }
        }
        None
    }
}

/// Decode every frame contained in a finished buffer.
pub fn decode_all(sample_rate: u32, samples: &[f32]) -> Vec<Vec<u8>> {
    let mut decoder = Decoder::new(sample_rate);
    decoder.push(samples);
    let mut messages = Vec::new();
    while let Some(message) = decoder.poll() {
        messages.push(message);
    }
    messages
}
