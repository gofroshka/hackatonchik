use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, SampleFormat, Stream, SupportedStreamConfig};

pub fn run_record() {
    let mut args = std::env::args().skip(1);
    let out = args.next().unwrap_or_else(|| {
        eprintln!("usage: record <out.wav> [seconds]");
        std::process::exit(2);
    });
    let secs: f32 = args
        .next()
        .and_then(|value| value.parse().ok())
        .unwrap_or(10.0);

    let host = cpal::default_host();
    let device = host.default_input_device().expect("no input device");
    let config = device
        .default_input_config()
        .expect("no default input config");
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    println!(
        "recording {secs}s from {} @ {sample_rate} Hz, {channels} ch -> {out}",
        device.name().unwrap_or_default()
    );

    let samples = Arc::new(Mutex::new(Vec::<f32>::new()));
    let output = Arc::clone(&samples);
    let stream =
        build_mono_input_stream_with_u16_midpoint(&device, &config, 32768.0, move |data| {
            output.lock().unwrap().extend(data);
        });
    stream.play().expect("failed to start input stream");
    std::thread::sleep(std::time::Duration::from_secs_f32(secs));
    drop(stream);

    let data = samples.lock().unwrap();
    let peak = data
        .iter()
        .fold(0.0f32, |peak, sample| peak.max(sample.abs()));
    crate::wav::write_mono(&out, sample_rate, data.iter().copied()).expect("write wav");
    println!("wrote {} samples, peak={peak:.4}", data.len());
}

pub fn build_mono_input_stream(
    device: &Device,
    config: &SupportedStreamConfig,
    on_samples: impl FnMut(Vec<f32>) + Send + 'static,
) -> Stream {
    build_mono_input_stream_fallible(device, config, on_samples, |error| {
        eprintln!("stream error: {error}")
    })
    .unwrap_or_else(|error| panic!("cannot open input stream: {error}"))
}

pub fn build_mono_input_stream_fallible(
    device: &Device,
    config: &SupportedStreamConfig,
    on_samples: impl FnMut(Vec<f32>) + Send + 'static,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, cpal::BuildStreamError> {
    build_mono_input_stream_with_callbacks(
        device,
        config,
        u16::MAX as f32 / 2.0,
        on_samples,
        on_error,
    )
}

fn build_mono_input_stream_with_u16_midpoint(
    device: &Device,
    config: &SupportedStreamConfig,
    u16_midpoint: f32,
    on_samples: impl FnMut(Vec<f32>) + Send + 'static,
) -> Stream {
    build_mono_input_stream_with_callbacks(device, config, u16_midpoint, on_samples, |error| {
        eprintln!("stream error: {error}")
    })
    .unwrap_or_else(|error| panic!("cannot open input stream: {error}"))
}

fn build_mono_input_stream_with_callbacks(
    device: &Device,
    config: &SupportedStreamConfig,
    u16_midpoint: f32,
    on_samples: impl FnMut(Vec<f32>) + Send + 'static,
    on_error: impl FnMut(cpal::StreamError) + Send + 'static,
) -> Result<Stream, cpal::BuildStreamError> {
    let channels = config.channels() as usize;
    let on_samples = Arc::new(Mutex::new(on_samples));
    let on_error = Arc::new(Mutex::new(on_error));

    macro_rules! build {
        ($sample:ty, $convert:expr) => {{
            let on_samples = Arc::clone(&on_samples);
            let on_error = Arc::clone(&on_error);
            device.build_input_stream(
                &config.clone().into(),
                move |data: &[$sample], _| {
                    let mono = downmix(data, channels, $convert);
                    on_samples.lock().unwrap()(mono);
                },
                move |error| {
                    if let Ok(mut callback) = on_error.lock() {
                        callback(error);
                    }
                },
                None,
            )
        }};
    }

    match config.sample_format() {
        SampleFormat::F32 => build!(f32, |sample: f32| sample),
        SampleFormat::I16 => build!(i16, |sample: i16| sample as f32 / i16::MAX as f32),
        SampleFormat::U16 => build!(u16, |sample: u16| {
            (sample as f32 - u16_midpoint) / u16_midpoint
        }),
        other => panic!("unsupported sample format: {other:?}"),
    }
}

pub fn downmix<T: Copy>(data: &[T], channels: usize, convert: impl Fn(T) -> f32) -> Vec<f32> {
    let channels = channels.max(1);
    data.chunks(channels)
        .map(|frame| frame.iter().map(|&sample| convert(sample)).sum::<f32>() / channels as f32)
        .collect()
}
