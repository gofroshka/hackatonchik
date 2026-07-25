use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sonic_share_core::detect_tone;
use sonic_share_core::output::save_received;
use sonic_share_core::tone;
use sonic_share_core::transfer::{
    build_transfer, detect_content_type, is_chat_content_type, TransferEvent, TransferReceiver,
};
use sonic_share_core::Decoder;
use sonic_share_core::{F_HANDSHAKE_ACK, F_HANDSHAKE_REQ, F_END_ACK, HANDSHAKE_TONE_SECS};

const CHAT_CONTENT_TYPE: &str = "text/x-sonic-chat; charset=utf-8";
const PCM_QUEUE_CAPACITY: usize = 8;

#[derive(Debug)]
enum TxCommand {
    File(PathBuf),
    Chat(String),
    Shutdown,
}

#[derive(Debug)]
enum RxCommand {
    Start,
    Stop,
    Shutdown,
}

#[derive(Debug)]
pub enum RuntimeEvent {
    Status(String),
    Error(String),
    RxStarted { device: String, sample_rate: u32 },
    RxStopped,
    InputLevel(f32),
    TransferStarted { name: String, size: u64 },
    TransferProgress { completed: u32, total: u32 },
    FileReceived(PathBuf),
    ChatReceived(String),
    TxStarted,
    TxProgress { current: usize, total: usize },
    TxFinished,
}

pub struct Runtime {
    tx_commands: SyncSender<TxCommand>,
    rx_commands: SyncSender<RxCommand>,
    events: Receiver<RuntimeEvent>,
    workers: Vec<JoinHandle<()>>,
    shutdown: Arc<AtomicBool>,
}

impl Runtime {
    pub fn new(output_dir: PathBuf) -> Self {
        let (event_tx, events) = mpsc::channel();
        let (tx_commands, tx_rx) = mpsc::sync_channel(4);
        let (rx_commands, rx_rx) = mpsc::sync_channel(4);
        let shutdown = Arc::new(AtomicBool::new(false));
        let tx_events = event_tx.clone();
        let tx_shutdown = Arc::clone(&shutdown);
        let tx_worker = thread::spawn(move || tx_loop(tx_rx, tx_events, tx_shutdown));
        let rx_shutdown = Arc::clone(&shutdown);
        let rx_worker = thread::spawn(move || rx_loop(rx_rx, event_tx, output_dir, rx_shutdown));
        Self {
            tx_commands,
            rx_commands,
            events,
            workers: vec![tx_worker, rx_worker],
            shutdown,
        }
    }

    pub fn send_file(&self, path: PathBuf) -> Result<(), String> {
        self.tx_commands
            .try_send(TxCommand::File(path))
            .map_err(|error| format!("transmit queue unavailable: {error}"))
    }

    pub fn send_chat(&self, text: String) -> Result<(), String> {
        self.tx_commands
            .try_send(TxCommand::Chat(text))
            .map_err(|error| format!("transmit queue unavailable: {error}"))
    }

    pub fn start_receive(&self) -> Result<(), String> {
        self.rx_commands
            .try_send(RxCommand::Start)
            .map_err(|error| format!("receive worker unavailable: {error}"))
    }

    pub fn stop_receive(&self) -> Result<(), String> {
        self.rx_commands
            .try_send(RxCommand::Stop)
            .map_err(|error| format!("receive worker unavailable: {error}"))
    }

    pub fn try_event(&self) -> Result<RuntimeEvent, TryRecvError> {
        self.events.try_recv()
    }
}

impl Drop for Runtime {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        let _ = self.tx_commands.try_send(TxCommand::Shutdown);
        let _ = self.rx_commands.try_send(RxCommand::Shutdown);
        for worker in self.workers.drain(..) {
            let _ = worker.join();
        }
    }
}

fn tx_loop(
    commands: Receiver<TxCommand>,
    events: mpsc::Sender<RuntimeEvent>,
    shutdown: Arc<AtomicBool>,
) {
    while !shutdown.load(Ordering::Relaxed) {
        let Ok(command) = commands.recv_timeout(Duration::from_millis(20)) else {
            continue;
        };
        let result = match command {
            TxCommand::File(path) => send_file(&path, &events, &shutdown),
            TxCommand::Chat(text) => send_payload(
                "chat-message.txt",
                CHAT_CONTENT_TYPE,
                text.as_bytes(),
                &events,
                &shutdown,
            ),
            TxCommand::Shutdown => break,
        };
        if let Err(error) = result {
            let _ = events.send(RuntimeEvent::Error(error));
        }
        let _ = events.send(RuntimeEvent::TxFinished);
    }
}

fn send_file(
    path: &Path,
    events: &mpsc::Sender<RuntimeEvent>,
    shutdown: &AtomicBool,
) -> Result<(), String> {
    let data = std::fs::read(path)
        .map_err(|error| format!("cannot read '{}': {error}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file.bin");
    let content_type = detect_content_type(name, &data);
    send_payload(name, &content_type, &data, events, shutdown)
}

fn send_payload(
    name: &str,
    content_type: &str,
    data: &[u8],
    events: &mpsc::Sender<RuntimeEvent>,
    shutdown: &AtomicBool,
) -> Result<(), String> {
    let plan = build_transfer(name, content_type, data, true)
        .map_err(|error| format!("cannot build transfer: {error}"))?;
    let total = plan.packets.len();

    let _ = events.send(RuntimeEvent::Status("Handshake…".to_owned()));
    let handshake_ok = do_tone_handshake(F_HANDSHAKE_REQ, F_HANDSHAKE_ACK, 6, events, shutdown);

    if handshake_ok {
        let _ = events.send(RuntimeEvent::Status("Handshake OK!".to_owned()));
    } else {
        let _ = events.send(RuntimeEvent::Status(
            "No handshake response, sending anyway…".to_owned(),
        ));
    }

    let _ = events.send(RuntimeEvent::Status(format!(
        "Sending '{}' in {total} packet(s)",
        plan.metadata.name
    )));
    let _ = events.send(RuntimeEvent::TxStarted);
    crate::audio::play_packets_cancellable(&plan.packets, shutdown, |current, total| {
        let _ = events.send(RuntimeEvent::TxProgress { current, total });
    })?;

    let _ = events.send(RuntimeEvent::Status("End handshake…".to_owned()));
    let end_ok = do_tone_handshake_silent(F_END_ACK, 5, events, shutdown);

    let _ = events.send(RuntimeEvent::Status(if end_ok {
        "Transfer confirmed by receiver!".to_owned()
    } else {
        "Transmission complete (no confirmation)".to_owned()
    }));
    Ok(())
}

fn play_tone_pcm(frequency: f32) -> Vec<f32> {
    tone::generate(HANDSHAKE_TONE_SECS, frequency)
}

fn do_tone_handshake(
    send_freq: f32,
    listen_freq: f32,
    max_attempts: usize,
    events: &mpsc::Sender<RuntimeEvent>,
    shutdown: &AtomicBool,
) -> bool {
    for attempt in 0..max_attempts {
        if shutdown.load(Ordering::Relaxed) {
            return false;
        }
        let _ = events.send(RuntimeEvent::Status(format!(
            "Handshake attempt {}/{}",
            attempt + 1,
            max_attempts
        )));
        if play_and_listen_for_tone(send_freq, listen_freq, 1.0, shutdown) {
            return true;
        }
    }
    false
}

fn do_tone_handshake_silent(
    listen_freq: f32,
    max_attempts: usize,
    events: &mpsc::Sender<RuntimeEvent>,
    shutdown: &AtomicBool,
) -> bool {
    for attempt in 0..max_attempts {
        if shutdown.load(Ordering::Relaxed) {
            return false;
        }
        let _ = events.send(RuntimeEvent::Status(format!(
            "Waiting for confirmation {}/{}",
            attempt + 1,
            max_attempts
        )));
        if listen_for_tone(listen_freq, 0.6, shutdown) {
            return true;
        }
    }
    false
}

fn play_and_listen_for_tone(
    play_freq: f32,
    listen_freq: f32,
    duration_secs: f32,
    shutdown: &AtomicBool,
) -> bool {
    let host = match cpal::default_host().default_output_device() {
        Some(device) => device,
        None => return false,
    };
    let config = match host.default_output_config() {
        Ok(config) => config,
        Err(_) => return false,
    };
    let output_sample_rate = config.sample_rate().0;
    let tone_samples = play_tone_pcm(play_freq);
    let resampled = crate::audio::resample(&tone_samples, 48000, output_sample_rate);
    let channels = config.channels() as usize;
    let (send_tx, send_rx) = std::sync::mpsc::sync_channel::<Vec<f32>>(2);

    let stream = host
        .build_output_stream(
            &config.clone().into(),
            move |buffer: &mut [f32], _| {
                if let Ok(chunk) = send_rx.try_recv() {
                    for (frame, &value) in buffer.chunks_mut(channels).zip(chunk.iter()) {
                        for slot in frame {
                            *slot = value;
                        }
                    }
                }
            },
            |_| {},
            None,
        )
        .ok();
    let Some(stream) = stream else { return false };
    let _ = stream.play();
    let _ = send_tx.send(resampled);
    std::thread::sleep(std::time::Duration::from_secs_f32(duration_secs * 0.5));
    drop(stream);

    listen_for_tone(listen_freq, duration_secs * 0.5, shutdown)
}

fn listen_for_tone(
    frequency: f32,
    duration_secs: f32,
    shutdown: &AtomicBool,
) -> bool {
    let device = match cpal::default_host().default_input_device() {
        Some(device) => device,
        None => return false,
    };
    let config = match device.default_input_config() {
        Ok(config) => config,
        Err(_) => return false,
    };
    let sample_rate = config.sample_rate().0;
    let found = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    let found_clone = std::sync::Arc::clone(&found);

    let stream = crate::audio::build_mono_input_stream_fallible(
        &device,
        &config,
        move |samples| {
            if detect_tone(&samples, frequency, sample_rate, 0.008) {
                found_clone.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        },
        |_| {},
    );

    match stream {
        Ok(stream) => {
            let _ = stream.play();
            let start = Instant::now();
            while start.elapsed() < Duration::from_secs_f32(duration_secs) {
                if shutdown.load(Ordering::Relaxed) {
                    break;
                }
                if found.load(std::sync::atomic::Ordering::Relaxed) {
                    drop(stream);
                    return true;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            drop(stream);
            found.load(std::sync::atomic::Ordering::Relaxed)
        }
        Err(_) => false,
    }
}

fn rx_loop(
    commands: Receiver<RxCommand>,
    events: mpsc::Sender<RuntimeEvent>,
    output_dir: PathBuf,
    shutdown: Arc<AtomicBool>,
) {
    let (pcm_tx, pcm_rx) = mpsc::sync_channel::<Vec<f32>>(PCM_QUEUE_CAPACITY);
    let mut stream: Option<cpal::Stream> = None;
    let mut decoder: Option<Decoder> = None;
    let mut transfers = TransferReceiver::new();
    let mut last_level = Instant::now();
    let mut tone_cooldown = Instant::now();
    let mut input_sample_rate: u32 = 48000;
    let mut transfer_active = false;
    loop {
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        match commands.recv_timeout(Duration::from_millis(20)) {
            Ok(RxCommand::Start) if stream.is_none() => {
                match start_input(pcm_tx.clone(), events.clone()) {
                    Ok((new_stream, new_decoder, device, sample_rate)) => {
                        stream = Some(new_stream);
                        decoder = Some(new_decoder);
                        input_sample_rate = sample_rate;
                        transfers = TransferReceiver::new();
                        transfer_active = false;
                        let _ = events.send(RuntimeEvent::RxStarted {
                            device,
                            sample_rate,
                        });
                    }
                    Err(error) => {
                        let _ = events.send(RuntimeEvent::Error(error));
                    }
                }
            }
            Ok(RxCommand::Stop) => {
                stream = None;
                decoder = None;
                transfer_active = false;
                while pcm_rx.try_recv().is_ok() {}
                let _ = events.send(RuntimeEvent::RxStopped);
            }
            Ok(RxCommand::Shutdown) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(RxCommand::Start) | Err(mpsc::RecvTimeoutError::Timeout) => {}
        }

        let Some(active_decoder) = decoder.as_mut() else {
            continue;
        };
        while let Ok(chunk) = pcm_rx.try_recv() {
            if last_level.elapsed() >= Duration::from_millis(100) {
                let peak = chunk
                    .iter()
                    .fold(0.0f32, |level, sample| level.max(sample.abs()));
                let _ = events.send(RuntimeEvent::InputLevel(peak));
                last_level = Instant::now();
            }
            if !transfer_active
                && tone_cooldown.elapsed() > Duration::from_secs(1)
                && detect_tone(&chunk, F_HANDSHAKE_REQ, input_sample_rate, 0.008)
            {
                let _ = events.send(RuntimeEvent::Status(
                    "Handshake request detected — sending ACK".to_owned(),
                ));
                decoder = None;
                stream = None;
                while pcm_rx.try_recv().is_ok() {}
                let ack_samples = play_tone_pcm(F_HANDSHAKE_ACK);
                if let Some(output_device) = cpal::default_host().default_output_device() {
                    if let Ok(config) = output_device.default_output_config() {
                        let output_sr = config.sample_rate().0;
                        let resampled = crate::audio::resample(&ack_samples, 48000, output_sr);
                        let channels = config.channels() as usize;
                        let (ack_tx, ack_rx) =
                            std::sync::mpsc::sync_channel::<Vec<f32>>(2);
                        if let Ok(ack_stream) = output_device.build_output_stream(
                            &config.clone().into(),
                            move |buffer: &mut [f32], _| {
                                if let Ok(chunk) = ack_rx.try_recv() {
                                    for (frame, &value) in
                                        buffer.chunks_mut(channels).zip(chunk.iter())
                                    {
                                        for slot in frame {
                                            *slot = value;
                                        }
                                    }
                                }
                            },
                            |_| {},
                            None,
                        ) {
                            let _ = ack_stream.play();
                            let _ = ack_tx.send(resampled);
                            std::thread::sleep(Duration::from_millis(600));
                            drop(ack_stream);
                        }
                    }
                }
                match start_input(pcm_tx.clone(), events.clone()) {
                    Ok((new_stream, new_decoder, device, new_sr)) => {
                        stream = Some(new_stream);
                        decoder = Some(new_decoder);
                        input_sample_rate = new_sr;
                        transfer_active = false;
                        let _ = events.send(RuntimeEvent::RxStarted {
                            device,
                            sample_rate: new_sr,
                        });
                    }
                    Err(error) => {
                        let _ = events.send(RuntimeEvent::Error(error));
                    }
                }
                tone_cooldown = Instant::now();
                break;
            }
            active_decoder.push(&chunk);
            while let Some(packet) = active_decoder.poll() {
                for event in transfers.ingest(&packet) {
                    let is_start = matches!(event, TransferEvent::Started { .. });
                    let is_end = matches!(
                        event,
                        TransferEvent::Completed { .. } | TransferEvent::Failed { .. }
                    );
                    if is_start {
                        transfer_active = true;
                    } else if is_end {
                        transfer_active = false;
                    }
                    for event in classify_transfer_event(event, &output_dir) {
                        let _ = events.send(event);
                    }
                }
            }
        }
    }
}

fn start_input(
    pcm_tx: SyncSender<Vec<f32>>,
    events: mpsc::Sender<RuntimeEvent>,
) -> Result<(cpal::Stream, Decoder, String, u32), String> {
    let device = cpal::default_host()
        .default_input_device()
        .ok_or_else(|| "no input device available".to_owned())?;
    let name = device.name().unwrap_or_else(|_| "Default input".to_owned());
    let config = device
        .default_input_config()
        .map_err(|error| format!("no default input config: {error}"))?;
    let sample_rate = config.sample_rate().0;
    let stream = crate::audio::build_mono_input_stream_fallible(
        &device,
        &config,
        move |samples| {
            let _ = pcm_tx.try_send(samples);
        },
        move |error| {
            let _ = events.send(RuntimeEvent::Error(format!("input stream error: {error}")));
        },
    )
    .map_err(|error| format!("cannot open input stream: {error}"))?;
    stream
        .play()
        .map_err(|error| format!("cannot start input stream: {error}"))?;
    Ok((stream, Decoder::new(sample_rate), name, sample_rate))
}

fn classify_transfer_event(event: TransferEvent, output_dir: &Path) -> Vec<RuntimeEvent> {
    match event {
        TransferEvent::Started(metadata) => vec![RuntimeEvent::TransferStarted {
            name: metadata.name,
            size: metadata.original_size,
        }],
        TransferEvent::Progress {
            completed_groups,
            total_groups,
            ..
        } => vec![RuntimeEvent::TransferProgress {
            completed: completed_groups,
            total: total_groups,
        }],
        TransferEvent::Completed { metadata, data } => {
            if is_chat_content_type(&metadata.content_type) {
                return vec![match String::from_utf8(data) {
                    Ok(text) => RuntimeEvent::ChatReceived(text),
                    Err(_) => RuntimeEvent::Error("received chat is not valid UTF-8".to_owned()),
                }];
            }
            vec![match save_received(output_dir, &metadata, &data) {
                Ok(path) => RuntimeEvent::FileReceived(path),
                Err(error) => {
                    RuntimeEvent::Error(format!("cannot save '{}': {error}", metadata.name))
                }
            }]
        }
        TransferEvent::Failed { id, reason } => vec![RuntimeEvent::Error(format!(
            "transfer {id:016x} failed: {reason}"
        ))],
        TransferEvent::HandshakeRequest { id } => {
            vec![RuntimeEvent::Status(format!("handshake request {id:016x}"))]
        }
        TransferEvent::HandshakeAck { id } => {
            vec![RuntimeEvent::Status(format!("handshake ack {id:016x}"))]
        }
        TransferEvent::EndAck { id } => {
            vec![RuntimeEvent::Status(format!("end ack {id:016x}"))]
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sonic_share_core::transfer::{Compression, TransferMetadata};

    #[test]
    fn received_chat_is_classified_without_persistence() {
        let output =
            std::env::temp_dir().join(format!("sonic-share-tui-chat-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&output);
        let metadata = TransferMetadata {
            id: 9,
            name: "chat-message.txt".to_owned(),
            content_type: CHAT_CONTENT_TYPE.to_owned(),
            original_size: 2,
            encoded_size: 2,
            sha256: [0; 32],
            compression: Compression::None,
            group_count: 0,
        };

        let events = classify_transfer_event(
            TransferEvent::Completed {
                metadata,
                data: b"hi".to_vec(),
            },
            &output,
        );

        assert!(matches!(events.as_slice(), [RuntimeEvent::ChatReceived(text)] if text == "hi"));
        assert!(!output.exists());
    }
}
