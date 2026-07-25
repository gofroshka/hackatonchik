use std::io::Read;
use std::path::Path;

use sonic_share_core::transfer::{build_transfer, detect_content_type, Compression};
use sonic_share_core::{
    encode_hybrid_packet, hybrid_packet_routes, hybrid_sample_count, AcousticProfile, OfdmProfile,
    PacketPhy, ENCODE_SR, MAX_LANES,
};

struct Options {
    positional: Vec<String>,
    wav_output: Option<String>,
    content_type: Option<String>,
    name: Option<String>,
    play: bool,
    compress: bool,
    robust: bool,
    fsk: bool,
    qam16: bool,
    lane: u8,
}

pub fn run() {
    let options = parse_args();
    let (name, content_type, data) = load_input(&options);
    let plan = build_transfer(&name, &content_type, &data, options.compress)
        .unwrap_or_else(|error| fatal(&format!("cannot build transfer: {error}")));
    let fsk_profile = options.profile();
    let ofdm_profile = options.ofdm_profile();
    let routes = if options.robust || options.fsk {
        vec![PacketPhy::Fsk; plan.packets.len()]
    } else {
        hybrid_packet_routes(&plan.packets)
    };

    let estimated_samples: usize = plan
        .packets
        .iter()
        .zip(&routes)
        .map(|(packet, &route)| hybrid_sample_count(packet.len(), route, fsk_profile, ofdm_profile))
        .sum();
    let seconds = estimated_samples as f64 / ENCODE_SR as f64;
    let mode = if options.robust {
        "FSK Robust"
    } else if options.fsk {
        "FSK Fast"
    } else if options.qam16 {
        "OFDM fast"
    } else {
        "OFDM safe"
    };
    if options.qam16 {
        eprintln!(
            "warning: --qam16 (differential QPSK) has no frequency diversity and \
             is unreliable over laptop speakers; drop it to use the default OFDM \
             mode that is verified over a real acoustic path."
        );
    }
    println!(
        "transfer {:016x}: '{}' ({}), {} bytes, {} packets, {mode} lane {}, ~{seconds:.1}s",
        plan.metadata.id,
        plan.metadata.name,
        plan.metadata.content_type,
        plan.metadata.original_size,
        plan.packets.len(),
        options.lane,
    );
    if plan.metadata.compression != Compression::None {
        println!(
            "compressed: {} -> {} bytes",
            plan.metadata.original_size, plan.metadata.encoded_size
        );
    }

    if let Some(path) = &options.wav_output {
        crate::wav::write_mono(
            path,
            ENCODE_SR,
            plan.packets
                .iter()
                .zip(&routes)
                .flat_map(|(packet, &route)| {
                    encode_hybrid_packet(packet, route, fsk_profile, ofdm_profile)
                }),
        )
        .unwrap_or_else(|error| fatal(&format!("cannot write WAV: {error}")));
        println!("wrote {path}");
    }
    if options.play {
        if options.robust || options.fsk {
            crate::audio::play_packets_profile(&plan.packets, fsk_profile);
        } else {
            crate::audio::play_packets_hybrid(&plan.packets, ofdm_profile);
        }
        println!("done.");
    }
}

fn parse_args() -> Options {
    let mut args = std::env::args().skip(1);
    let mut options = Options {
        positional: Vec::new(),
        wav_output: None,
        content_type: None,
        name: None,
        play: true,
        compress: true,
        robust: false,
        fsk: false,
        qam16: false,
        lane: 0,
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--output" => options.wav_output = Some(required_value(&mut args, &arg)),
            "--content-type" => options.content_type = Some(required_value(&mut args, &arg)),
            "--name" => options.name = Some(required_value(&mut args, &arg)),
            "--no-play" => options.play = false,
            "--no-compress" => options.compress = false,
            "--robust" => options.robust = true,
            "--fsk" => options.fsk = true,
            "--qam16" => options.qam16 = true,
            "--lane" => {
                let value = required_value(&mut args, &arg);
                options.lane = parse_lane(&value);
            }
            "-h" | "--help" => {
                println!(
                    "usage:\n  send \"text\"\n  send text \"text\"\n  send file <path> [--content-type MIME]\n  send stdin --name FILE [--content-type MIME]\n\noptions:\n  -o, --output WAV   additionally write the full transfer to WAV\n  --lane 0..1        frequency lane for simultaneous pairs\n  --qam16            faster OFDM (differential QPSK) for cleaner channels\n  --fsk              legacy Fast FSK only\n  --robust           legacy Robust FSK for high noise/distance\n  --no-play          do not play through speakers\n  --no-compress      disable automatic zstd compression\n  --name NAME        transmitted file name\n  --content-type MIME"
                );
                std::process::exit(0);
            }
            _ => options.positional.push(arg),
        }
    }
    if [options.robust, options.fsk, options.qam16]
        .into_iter()
        .filter(|enabled| *enabled)
        .count()
        > 1
    {
        fatal("--robust, --fsk and --qam16 are mutually exclusive");
    }
    options
}

impl Options {
    fn profile(&self) -> AcousticProfile {
        if self.robust {
            AcousticProfile::robust(self.lane)
        } else {
            AcousticProfile::fast(self.lane)
        }
    }

    fn ofdm_profile(&self) -> OfdmProfile {
        if self.qam16 {
            OfdmProfile::qam16(self.lane)
        } else {
            OfdmProfile::qpsk(self.lane)
        }
    }
}

fn parse_lane(value: &str) -> u8 {
    value
        .parse::<u8>()
        .ok()
        .filter(|lane| *lane < MAX_LANES)
        .unwrap_or_else(|| fatal(&format!("lane must be in 0..{}", MAX_LANES - 1)))
}

fn required_value(args: &mut impl Iterator<Item = String>, flag: &str) -> String {
    args.next()
        .unwrap_or_else(|| fatal(&format!("{flag} requires a value")))
}

fn load_input(options: &Options) -> (String, String, Vec<u8>) {
    match options.positional.first().map(String::as_str) {
        Some("file") => {
            let path = options
                .positional
                .get(1)
                .unwrap_or_else(|| fatal("send file requires a path"));
            if options.positional.len() != 2 {
                fatal("send file accepts exactly one path");
            }
            let data = std::fs::read(path)
                .unwrap_or_else(|error| fatal(&format!("cannot read '{path}': {error}")));
            let name = options.name.clone().unwrap_or_else(|| {
                Path::new(path)
                    .file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("file.bin")
                    .to_owned()
            });
            let content_type = options
                .content_type
                .clone()
                .unwrap_or_else(|| detect_content_type(&name, &data));
            (name, content_type, data)
        }
        Some("stdin") => {
            let data = read_stdin_bytes();
            let name = options
                .name
                .clone()
                .unwrap_or_else(|| "stdin.bin".to_owned());
            let content_type = options
                .content_type
                .clone()
                .unwrap_or_else(|| detect_content_type(&name, &data));
            (name, content_type, data)
        }
        Some("text") => {
            let text = if options.positional.len() > 1 {
                options.positional[1..].join(" ")
            } else {
                String::from_utf8(read_stdin_bytes())
                    .unwrap_or_else(|_| fatal("stdin is not valid UTF-8 text"))
            };
            text_input(options, text)
        }
        _ => {
            let text = if options.positional.is_empty() {
                String::from_utf8(read_stdin_bytes())
                    .unwrap_or_else(|_| fatal("stdin is not valid UTF-8 text"))
            } else {
                options.positional.join(" ")
            };
            text_input(options, text)
        }
    }
}

fn text_input(options: &Options, text: String) -> (String, String, Vec<u8>) {
    (
        options
            .name
            .clone()
            .unwrap_or_else(|| "message.txt".to_owned()),
        options
            .content_type
            .clone()
            .unwrap_or_else(|| "text/plain; charset=utf-8".to_owned()),
        text.into_bytes(),
    )
}

fn read_stdin_bytes() -> Vec<u8> {
    let mut data = Vec::new();
    std::io::stdin()
        .read_to_end(&mut data)
        .unwrap_or_else(|error| fatal(&format!("cannot read stdin: {error}")));
    data
}

fn fatal(message: &str) -> ! {
    eprintln!("error: {message}");
    std::process::exit(2)
}
