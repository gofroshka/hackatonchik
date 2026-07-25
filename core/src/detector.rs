use std::f32::consts::PI;

pub(crate) const COLS_PER_SYM: usize = 16;

pub(crate) fn goertzel(samples: &[f32], frequency: f32, sample_rate: u32) -> f32 {
    if samples.is_empty() {
        return 0.0;
    }
    let angle = 2.0 * PI * frequency / sample_rate as f32;
    let coefficient = 2.0 * angle.cos();
    let (mut previous, mut previous_two) = (0.0f32, 0.0f32);
    for &sample in samples {
        let current = sample + coefficient * previous - previous_two;
        previous_two = previous;
        previous = current;
    }
    let power =
        previous * previous + previous_two * previous_two - coefficient * previous * previous_two;
    power.max(0.0).sqrt() / samples.len() as f32
}

#[inline]
pub(crate) fn data_argmax(energies: &[f32]) -> (u8, f32, f32) {
    debug_assert!(energies.len() >= 16);
    let mut peak = 0.0f32;
    let mut peak_index = 0usize;
    let mut sum = 0.0f32;
    for (index, &value) in energies.iter().take(16).enumerate() {
        sum += value;
        if value > peak {
            peak = value;
            peak_index = index;
        }
    }
    (peak_index as u8, peak, sum - peak)
}
