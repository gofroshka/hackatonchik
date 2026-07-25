use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use sonic_share_core::output::save_received;
use sonic_share_core::transfer::{
    build_transfer, detect_content_type, is_chat_content_type, TransferEvent, TransferReceiver,
};
use sonic_share_core::{HybridDecoder, OfdmProfile};

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
    let _ = events.send(RuntimeEvent::Status(format!(
        "Sending '{}' in {total} packet(s)",
        plan.metadata.name
    )));
    let _ = events.send(RuntimeEvent::TxStarted);
    crate::audio::play_packets_cancellable_hybrid(
        &plan.packets,
        shutdown,
        OfdmProfile::qpsk(0),
        |current, total| {
            let _ = events.send(RuntimeEvent::TxProgress { current, total });
        },
    )?;
    let _ = events.send(RuntimeEvent::Status("Transmission complete".to_owned()));
    Ok(())
}

fn rx_loop(
    commands: Receiver<RxCommand>,
    events: mpsc::Sender<RuntimeEvent>,
    output_dir: PathBuf,
    shutdown: Arc<AtomicBool>,
) {
    let (pcm_tx, pcm_rx) = mpsc::sync_channel::<Vec<f32>>(PCM_QUEUE_CAPACITY);
    let mut stream = None;
    let mut decoder = None;
    let mut transfers = TransferReceiver::new();
    let mut last_level = Instant::now();

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
                        transfers = TransferReceiver::new();
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
            active_decoder.push(&chunk);
            while let Some(packet) = active_decoder.poll() {
                for event in transfers.ingest(&packet) {
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
) -> Result<(cpal::Stream, HybridDecoder, String, u32), String> {
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
    Ok((
        stream,
        HybridDecoder::with_sample_rate(sample_rate, OfdmProfile::qpsk(0)),
        name,
        sample_rate,
    ))
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
