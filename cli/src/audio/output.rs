use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, sync_channel, TryRecvError, TrySendError};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use sonic_share_core::{encode, ENCODE_SR};

pub fn play_packets(packets: &[Vec<u8>]) {
    play_packets_fallible(packets, |index, total| println!("packet {index}/{total}"))
        .unwrap_or_else(|error| fatal(&error));
}

pub fn play_packets_fallible(
    packets: &[Vec<u8>],
    on_progress: impl FnMut(usize, usize),
) -> Result<(), String> {
    play_packets_cancellable(packets, &AtomicBool::new(false), on_progress)
}

pub fn play_packets_cancellable(
    packets: &[Vec<u8>],
    cancelled: &AtomicBool,
    mut on_progress: impl FnMut(usize, usize),
) -> Result<(), String> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or_else(|| "no output device available".to_owned())?;
    let config = device
        .default_output_config()
        .map_err(|error| format!("no default output config: {error}"))?;
    let output_sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    let (audio_tx, audio_rx) = sync_channel::<Vec<f32>>(2);
    let (done_tx, done_rx) = channel::<()>();
    let (error_tx, error_rx) = channel::<String>();

    macro_rules! build {
        ($sample:ty, $convert:expr) => {{
            let mut current = Vec::<f32>::new();
            let mut position = 0usize;
            let mut finished = false;
            let done_tx = done_tx.clone();
            let error_tx = error_tx.clone();
            device.build_output_stream(
                &config.clone().into(),
                move |buffer: &mut [$sample], _| {
                    for frame in buffer.chunks_mut(channels) {
                        if position >= current.len() && !finished {
                            match audio_rx.try_recv() {
                                Ok(next) if next.is_empty() => {
                                    finished = true;
                                    let _ = done_tx.send(());
                                }
                                Ok(next) => {
                                    current = next;
                                    position = 0;
                                }
                                Err(TryRecvError::Disconnected) => finished = true,
                                Err(TryRecvError::Empty) => {}
                            }
                        }
                        let value = if position < current.len() {
                            let value = current[position];
                            position += 1;
                            value
                        } else {
                            0.0
                        };
                        for slot in frame {
                            *slot = $convert(value);
                        }
                    }
                },
                move |error| {
                    let _ = error_tx.send(error.to_string());
                },
                None,
            )
        }};
    }

    let stream = match config.sample_format() {
        SampleFormat::F32 => build!(f32, |value: f32| value),
        SampleFormat::I16 => build!(i16, |value: f32| {
            (value.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
        }),
        SampleFormat::U16 => build!(u16, |value: f32| {
            ((value.clamp(-1.0, 1.0) * 0.5 + 0.5) * u16::MAX as f32) as u16
        }),
        other => return Err(format!("unsupported output format: {other:?}")),
    }
    .map_err(|error| format!("cannot open output stream: {error}"))?;
    stream
        .play()
        .map_err(|error| format!("cannot start output: {error}"))?;

    for (index, packet) in packets.iter().enumerate() {
        if cancelled.load(Ordering::Relaxed) {
            return Err("transmission cancelled".to_owned());
        }
        let mut wave = resample(&encode(packet), ENCODE_SR, output_sample_rate);
        loop {
            match audio_tx.try_send(wave) {
                Ok(()) => break,
                Err(TrySendError::Full(returned)) => {
                    if cancelled.load(Ordering::Relaxed) {
                        return Err("transmission cancelled".to_owned());
                    }
                    wave = returned;
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(TrySendError::Disconnected(_)) => {
                    return Err("audio stream stopped".to_owned());
                }
            }
        }
        if let Ok(error) = error_rx.try_recv() {
            return Err(format!("output stream error: {error}"));
        }
        on_progress(index + 1, packets.len());
    }
    let mut sentinel = Vec::new();
    loop {
        match audio_tx.try_send(sentinel) {
            Ok(()) => break,
            Err(TrySendError::Full(returned)) => {
                if cancelled.load(Ordering::Relaxed) {
                    return Err("transmission cancelled".to_owned());
                }
                sentinel = returned;
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            Err(TrySendError::Disconnected(_)) => {
                return Err("audio stream stopped".to_owned());
            }
        }
    }
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err("transmission cancelled".to_owned());
        }
        match done_rx.recv_timeout(std::time::Duration::from_millis(20)) {
            Ok(()) => break,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                return Err("audio stream stopped before completion".to_owned());
            }
        }
    }
    std::thread::sleep(std::time::Duration::from_millis(300));
    if let Ok(error) = error_rx.try_recv() {
        return Err(format!("output stream error: {error}"));
    }
    Ok(())
}

pub fn resample(input: &[f32], from: u32, to: u32) -> Vec<f32> {
    if from == to || input.is_empty() {
        return input.to_vec();
    }
    let ratio = to as f64 / from as f64;
    let output_len = (input.len() as f64 * ratio).round() as usize;
    let mut output = Vec::with_capacity(output_len);
    for index in 0..output_len {
        let source = index as f64 / ratio;
        let source_index = source.floor() as usize;
        let fraction = (source - source_index as f64) as f32;
        let first = input[source_index.min(input.len() - 1)];
        let second = input[(source_index + 1).min(input.len() - 1)];
        output.push(first + (second - first) * fraction);
    }
    output
}

fn fatal(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(2)
}
