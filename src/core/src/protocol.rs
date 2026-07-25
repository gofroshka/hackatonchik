/// Sample rate used when synthesising audio.
pub const ENCODE_SR: u32 = 48_000;
/// Default fast profile tone length.
pub const TONE_SAMPLES: usize = 512;
/// Default fast profile guard interval.
pub const GUARD_SAMPLES: usize = 512;
/// Full symbol period (tone + guard).
pub const SYMBOL_SAMPLES: usize = TONE_SAMPLES + GUARD_SAMPLES;
/// Symbol period in seconds (sample-rate independent).
pub const SYMBOL_DURATION: f32 = SYMBOL_SAMPLES as f32 / ENCODE_SR as f32;
/// Tone-burst duration in seconds.
pub const TONE_DURATION: f32 = TONE_SAMPLES as f32 / ENCODE_SR as f32;
/// Bin spacing for the exact tone grid.
pub const DF: f32 = ENCODE_SR as f32 / TONE_SAMPLES as f32;
/// First data tone in fast lane 0 (bin 16, 1500 Hz).
pub const F_DATA0: f32 = 16.0 * DF;
/// Bin spacing between adjacent data tones.
pub const TONE_STEP: usize = 1;
/// Marker/preamble tone above the data band.
pub const F_MARKER: f32 = 56.0 * DF;
pub const PREAMBLE_SYMBOLS: usize = 10;
pub const SYNC_BYTE: u8 = 0xA5;
pub const AMPLITUDE: f32 = 0.6;
pub const MAX_PAYLOAD: usize = 255;

pub(crate) const SYNC_REPEATS: usize = 5;
pub(crate) const LEN_REPEATS: usize = 5;

pub const MAX_LANES: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhyMode {
    Fast,
    Robust,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AcousticProfile {
    mode: PhyMode,
    lane: u8,
}

impl AcousticProfile {
    pub const fn fast(lane: u8) -> Self {
        assert!(lane < MAX_LANES, "acoustic lane out of range");
        Self {
            mode: PhyMode::Fast,
            lane,
        }
    }

    pub const fn robust(lane: u8) -> Self {
        assert!(lane < MAX_LANES, "acoustic lane out of range");
        Self {
            mode: PhyMode::Robust,
            lane,
        }
    }

    pub const fn mode(self) -> PhyMode {
        self.mode
    }

    pub const fn lane(self) -> u8 {
        self.lane
    }

    pub const fn tone_samples(self) -> usize {
        match self.mode {
            PhyMode::Fast => 512,
            PhyMode::Robust => 1024,
        }
    }

    pub const fn guard_samples(self) -> usize {
        match self.mode {
            PhyMode::Fast => 512,
            PhyMode::Robust => 1024,
        }
    }

    pub const fn symbol_samples(self) -> usize {
        self.tone_samples() + self.guard_samples()
    }

    pub const fn preamble_symbols(self) -> usize {
        match self.mode {
            PhyMode::Fast => 10,
            PhyMode::Robust => 16,
        }
    }

    pub const fn sync_repeats(self) -> usize {
        match self.mode {
            PhyMode::Fast => SYNC_REPEATS,
            PhyMode::Robust => 5,
        }
    }

    pub const fn len_repeats(self) -> usize {
        match self.mode {
            PhyMode::Fast => LEN_REPEATS,
            PhyMode::Robust => 5,
        }
    }

    pub const fn is_dual_tone(self) -> bool {
        matches!(self.mode, PhyMode::Fast)
    }

    pub const fn frame_symbols_per_byte(self) -> usize {
        if self.is_dual_tone() {
            1
        } else {
            2
        }
    }

    fn bin_spacing(self) -> f32 {
        ENCODE_SR as f32 / self.tone_samples() as f32
    }

    fn data_start_bin(self) -> usize {
        match self.mode {
            PhyMode::Fast => 16 + self.lane as usize * 48,
            PhyMode::Robust => 40 + self.lane as usize * 48,
        }
    }

    pub fn data_freq(self, value: u8) -> f32 {
        let offset = if self.is_dual_tone() {
            value as usize * 2
        } else {
            value as usize
        };
        (self.data_start_bin() + offset) as f32 * self.bin_spacing()
    }

    pub fn high_data_freq(self, value: u8) -> f32 {
        (self.data_start_bin() + value as usize * 2 + 1) as f32 * self.bin_spacing()
    }

    pub fn marker_freq(self) -> f32 {
        let offset = if self.is_dual_tone() { 40 } else { 24 };
        (self.data_start_bin() + offset) as f32 * self.bin_spacing()
    }

    pub const fn marker_index(self) -> usize {
        if self.is_dual_tone() {
            32
        } else {
            16
        }
    }

    pub const fn detector_tone_count(self) -> usize {
        self.marker_index() + 1
    }

    pub fn symbol_len(self, sample_rate: u32) -> usize {
        ((self.symbol_samples() as f32 / ENCODE_SR as f32 * sample_rate as f32).round() as usize)
            .max(1)
    }

    pub fn tone_len(self, sample_rate: u32) -> usize {
        ((self.tone_samples() as f32 / ENCODE_SR as f32 * sample_rate as f32).round() as usize)
            .max(1)
    }
}

impl Default for AcousticProfile {
    fn default() -> Self {
        Self::fast(0)
    }
}

/// Frequency of the data tone for a nibble value in `0..16`.
#[inline]
pub fn data_freq(value: u8) -> f32 {
    AcousticProfile::default().data_freq(value)
}

/// Samples per symbol period for a given sample rate.
pub fn symbol_len(sample_rate: u32) -> usize {
    AcousticProfile::default().symbol_len(sample_rate)
}

/// Samples in the tone burst for a given sample rate.
pub fn tone_len(sample_rate: u32) -> usize {
    AcousticProfile::default().tone_len(sample_rate)
}

pub(crate) fn crc8(data: &[u8]) -> u8 {
    let mut checksum = 0u8;
    for &byte in data {
        checksum ^= byte;
        for _ in 0..8 {
            checksum = if checksum & 0x80 != 0 {
                (checksum << 1) ^ 0x07
            } else {
                checksum << 1
            };
        }
    }
    checksum
}

pub(crate) fn majority(values: &[u8]) -> Option<u8> {
    values.iter().copied().find(|candidate| {
        values.iter().filter(|&&value| value == *candidate).count() > values.len() / 2
    })
}
