use crate::detector::{goertzel, COLS_PER_SYM};
use crate::protocol::{data_freq, symbol_len, tone_len, F_MARKER};

/// Print per-tone energy, preamble detections, and raw nibbles for a recording.
pub fn diagnose(sample_rate: u32, samples: &[f32]) {
    let symbol_len = symbol_len(sample_rate);
    let tone_len = tone_len(sample_rate);
    let hop = (symbol_len / COLS_PER_SYM).max(1);
    let mut frequencies: Vec<f32> = (0..16u8).map(data_freq).collect();
    frequencies.push(F_MARKER);

    let mut columns = Vec::new();
    let mut start = 0;
    while start + tone_len <= samples.len() {
        let window = &samples[start..start + tone_len];
        let mut energies = [0.0f32; 17];
        for (index, &frequency) in frequencies.iter().enumerate() {
            energies[index] = goertzel(window, frequency, sample_rate);
        }
        columns.push(energies);
        start += hop;
    }
    println!("cols={} (hop={hop}, sl={symbol_len})", columns.len());

    let floor = 2e-4;
    let mut preamble_runs = Vec::new();
    let mut run_start = None;
    for (column, energies) in columns.iter().enumerate() {
        let marker = energies[16];
        let data_max = energies[..16].iter().copied().fold(0.0, f32::max);
        let is_preamble = marker > floor && marker > data_max * 2.0;
        match (is_preamble, run_start) {
            (true, None) => run_start = Some(column),
            (false, Some(start)) => {
                if column - start >= 4 {
                    preamble_runs.push((start, column));
                }
                run_start = None;
            }
            _ => {}
        }
    }
    if let Some(start) = run_start {
        preamble_runs.push((start, columns.len()));
    }
    println!("preamble runs (>=4 cols): {preamble_runs:?}");

    for (_, data_start) in preamble_runs.iter().copied() {
        print!("  after col {data_start}: nibbles ");
        let mut column = data_start;
        for _ in 0..40 {
            let Some(energies) = columns.get(column) else {
                break;
            };
            let mut peak = 0.0f32;
            let mut peak_index = 0usize;
            let mut sum = 0.0f32;
            for (index, &value) in energies[..16].iter().enumerate() {
                sum += value;
                if value > peak {
                    peak = value;
                    peak_index = index;
                }
            }
            let ratio = peak / (sum - peak).max(1e-9);
            print!("{peak_index:x}({ratio:.1}) ");
            column += COLS_PER_SYM;
        }
        println!();
    }

    let mut totals = [0.0f64; 17];
    let mut count = 0u64;
    for energies in &columns {
        if energies.iter().sum::<f32>() > 0.02 {
            for (total, &energy) in totals.iter_mut().zip(energies) {
                *total += energy as f64;
            }
            count += 1;
        }
    }
    if count > 0 {
        print!("tone profile (avg over {count} loud cols): ");
        for value in totals.iter().take(16) {
            print!("{:.3} ", value / count as f64);
        }
        println!("| marker={:.3}", totals[16] / count as f64);
    }
}
