use std::path::{Path, PathBuf};
use std::sync::mpsc::channel;

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sonic_share_core::output::save_received;
use sonic_share_core::transfer::{is_chat_content_type, TransferEvent, TransferReceiver};
use sonic_share_core::Decoder;

#[derive(Clone, Copy)]
enum OutputStyle {
    Listen,
    Decode,
}

pub fn run_listen() {
    let output_dir = listen_output_directory();
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .expect("no input device available");
    println!("input device: {}", device.name().unwrap_or_default());

    let config = device
        .default_input_config()
        .expect("no default input config");
    let sample_rate = config.sample_rate().0;
    let channels = config.channels() as usize;
    println!(
        "sample rate: {} Hz, channels: {}, format: {:?}",
        sample_rate,
        channels,
        config.sample_format()
    );
    println!(
        "listening... files are saved to '{}'; chat messages are printed here. Ctrl-C to stop.\n",
        output_dir.display()
    );

    let (tx, rx) = channel::<Vec<f32>>();
    let stream = crate::audio::build_mono_input_stream(&device, &config, move |samples| {
        let _ = tx.send(samples);
    });
    stream.play().expect("failed to start stream");

    let debug = std::env::var("DEBUG").is_ok();
    let mut decoder = Decoder::new(sample_rate);
    let mut transfers = TransferReceiver::new();
    let mut since_report = 0usize;
    let mut peak = 0.0f32;
    let mut sum_squares = 0.0f64;
    let mut sample_count = 0u64;
    for chunk in rx {
        if debug {
            for &sample in &chunk {
                peak = peak.max(sample.abs());
                sum_squares += (sample as f64) * (sample as f64);
                sample_count += 1;
            }
            since_report += chunk.len();
            if since_report >= sample_rate as usize {
                let rms = (sum_squares / sample_count.max(1) as f64).sqrt();
                eprintln!("[audio] peak={peak:.4} rms={rms:.4}");
                since_report = 0;
                peak = 0.0;
                sum_squares = 0.0;
                sample_count = 0;
            }
        }
        decoder.push(&chunk);
        while let Some(payload) = decoder.poll() {
            handle_events(transfers.ingest(&payload), &output_dir, OutputStyle::Listen);
        }
    }
}

pub fn run_decode() {
    let (path, output_dir) = decode_args();
    let wav = crate::wav::read_mono(&path).unwrap_or_else(|error| {
        eprintln!("cannot open '{}': {error}", path.display());
        std::process::exit(2);
    });
    println!(
        "{}: {} Hz, {} ch, {} samples ({:.2}s)",
        path.display(),
        wav.sample_rate,
        wav.channels,
        wav.samples.len(),
        wav.samples.len() as f32 / wav.sample_rate as f32
    );
    if std::env::var("DEBUG").is_ok() {
        sonic_share_core::diagnose(wav.sample_rate, &wav.samples);
    }

    let mut receiver = TransferReceiver::new();
    let mut completed = 0usize;
    if std::env::var("STREAM").is_ok() {
        let chunk = std::env::var("CHUNK")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(512usize);
        let mut decoder = Decoder::new(wav.sample_rate);
        for samples in wav.samples.chunks(chunk) {
            decoder.push(samples);
            while let Some(packet) = decoder.poll() {
                completed +=
                    handle_events(receiver.ingest(&packet), &output_dir, OutputStyle::Decode);
            }
        }
    } else {
        for packet in sonic_share_core::decode_all(wav.sample_rate, &wav.samples) {
            completed += handle_events(receiver.ingest(&packet), &output_dir, OutputStyle::Decode);
        }
    }
    println!("completed transfers: {completed}");
}

fn listen_output_directory() -> PathBuf {
    let mut args = std::env::args().skip(1);
    let mut output = PathBuf::from("received");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output-dir" => {
                output = PathBuf::from(args.next().unwrap_or_else(|| {
                    eprintln!("error: --output-dir requires a path");
                    std::process::exit(2);
                }));
            }
            "-h" | "--help" => {
                println!("usage: listen [--output-dir received]");
                std::process::exit(0);
            }
            _ => {
                eprintln!("error: unknown argument '{arg}'");
                std::process::exit(2);
            }
        }
    }
    output
}

fn decode_args() -> (PathBuf, PathBuf) {
    let mut args = std::env::args().skip(1);
    let mut input = None;
    let mut output = PathBuf::from("received");
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output-dir" => {
                output = PathBuf::from(args.next().unwrap_or_else(|| {
                    eprintln!("error: --output-dir requires a path");
                    std::process::exit(2);
                }));
            }
            "-h" | "--help" => {
                println!("usage: decode <recording.wav> [--output-dir received]");
                std::process::exit(0);
            }
            _ if input.is_none() => input = Some(PathBuf::from(arg)),
            _ => {
                eprintln!("error: unexpected argument '{arg}'");
                std::process::exit(2);
            }
        }
    }
    let input = input.unwrap_or_else(|| {
        eprintln!("usage: decode <recording.wav> [--output-dir received]");
        std::process::exit(2);
    });
    (input, output)
}

fn handle_events(events: Vec<TransferEvent>, output_dir: &Path, style: OutputStyle) -> usize {
    let mut completed = 0;
    for event in events {
        match event {
            TransferEvent::Started(metadata) => match style {
                OutputStyle::Listen => println!(
                    "transfer {:016x}: '{}' ({}), {} bytes, {} groups",
                    metadata.id,
                    metadata.name,
                    metadata.content_type,
                    metadata.original_size,
                    metadata.group_count
                ),
                OutputStyle::Decode => println!(
                    "transfer {:016x}: '{}' ({}), {} bytes",
                    metadata.id, metadata.name, metadata.content_type, metadata.original_size
                ),
            },
            TransferEvent::Progress {
                id,
                completed_groups,
                total_groups,
            } => match style {
                OutputStyle::Listen => println!(
                    "transfer {id:016x}: group {completed_groups}/{total_groups} reconstructed"
                ),
                OutputStyle::Decode => {
                    println!("transfer {id:016x}: {completed_groups}/{total_groups} groups")
                }
            },
            TransferEvent::Completed { metadata, data } => {
                if is_chat_content_type(&metadata.content_type) {
                    match std::str::from_utf8(&data) {
                        Ok(text) => println!("chat {:016x} >> {text}", metadata.id),
                        Err(_) => eprintln!(
                            "chat {:016x} failed: message is not valid UTF-8",
                            metadata.id
                        ),
                    }
                    completed += usize::from(matches!(style, OutputStyle::Decode));
                    continue;
                }
                match save_received(output_dir, &metadata, &data) {
                    Ok(path) => {
                        match style {
                            OutputStyle::Listen => println!(
                                "received {:016x}: '{}' verified and saved to '{}'",
                                metadata.id,
                                metadata.content_type,
                                path.display()
                            ),
                            OutputStyle::Decode => {
                                println!("saved verified file: '{}'", path.display())
                            }
                        }
                        if is_text(&metadata.content_type) {
                            match (style, std::str::from_utf8(&data)) {
                                (_, Ok(text)) => println!(">> {text}"),
                                (OutputStyle::Listen, Err(_)) => {
                                    eprintln!("warning: declared text is not valid UTF-8")
                                }
                                (OutputStyle::Decode, Err(_)) => {}
                            }
                        }
                        completed += usize::from(matches!(style, OutputStyle::Decode));
                    }
                    Err(error) => eprintln!("cannot save '{}': {error}", metadata.name),
                }
            }
            TransferEvent::Failed { id, reason } => {
                eprintln!("transfer {id:016x} failed: {reason}");
            }
            TransferEvent::HandshakeRequest { id, .. } => {
                println!("handshake request {id:016x}");
            }
            TransferEvent::HandshakeAck { id, .. } => {
                println!("handshake ack {id:016x}");
            }
            TransferEvent::EndAck { id } => {
                println!("end ack {id:016x}");
            }
        }
    }
    completed
}

fn is_text(content_type: &str) -> bool {
    content_type.starts_with("text/")
        || content_type.starts_with("application/json")
        || content_type.contains("+json")
        || content_type.starts_with("application/xml")
        || content_type.contains("+xml")
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonic_share_core::transfer::{Compression, TransferMetadata};

    #[test]
    fn chat_message_is_completed_without_creating_output_directory() {
        let output =
            std::env::temp_dir().join(format!("sonic-share-cli-chat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&output);
        let event = TransferEvent::Completed {
            metadata: TransferMetadata {
                id: 7,
                name: "chat-message.txt".to_owned(),
                content_type: "text/x-sonic-chat; charset=utf-8".to_owned(),
                original_size: 5,
                encoded_size: 5,
                sha256: [0; 32],
                compression: Compression::None,
                group_count: 1,
            },
            data: b"hello".to_vec(),
        };

        assert_eq!(handle_events(vec![event], &output, OutputStyle::Decode), 1);
        assert!(!output.exists());
    }
}
