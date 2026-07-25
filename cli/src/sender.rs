use std::io::Read;
use std::path::Path;

use sonic_share_core::transfer::{build_transfer, detect_content_type, Compression};
use sonic_share_core::{encode, ENCODE_SR};

struct Options {
    positional: Vec<String>,
    wav_output: Option<String>,
    content_type: Option<String>,
    name: Option<String>,
    play: bool,
    compress: bool,
}

pub fn run() {
    let options = parse_args();
    let (name, content_type, data) = load_input(&options);
    let plan = build_transfer(&name, &content_type, &data, options.compress)
        .unwrap_or_else(|error| fatal(&format!("cannot build transfer: {error}")));

    let estimated_samples: usize = plan.packets.iter().map(|packet| encode(packet).len()).sum();
    let seconds = estimated_samples as f64 / ENCODE_SR as f64;
    println!(
        "transfer {:016x}: '{}' ({}), {} bytes, {} packets, ~{seconds:.1}s",
        plan.metadata.id,
        plan.metadata.name,
        plan.metadata.content_type,
        plan.metadata.original_size,
        plan.packets.len(),
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
            plan.packets.iter().flat_map(|packet| encode(packet)),
        )
        .unwrap_or_else(|error| fatal(&format!("cannot write WAV: {error}")));
        println!("wrote {path}");
    }
    if options.play {
        crate::audio::play_packets(&plan.packets);
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
    };
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-o" | "--output" => options.wav_output = Some(required_value(&mut args, &arg)),
            "--content-type" => options.content_type = Some(required_value(&mut args, &arg)),
            "--name" => options.name = Some(required_value(&mut args, &arg)),
            "--no-play" => options.play = false,
            "--no-compress" => options.compress = false,
            "-h" | "--help" => {
                println!(
                    "usage:\n  send \"text\"\n  send text \"text\"\n  send file <path> [--content-type MIME]\n  send stdin --name FILE [--content-type MIME]\n\noptions:\n  -o, --output WAV   additionally write the full transfer to WAV\n  --no-play          do not play through speakers\n  --no-compress      disable automatic zstd compression\n  --name NAME        transmitted file name\n  --content-type MIME"
                );
                std::process::exit(0);
            }
            _ => options.positional.push(arg),
        }
    }
    options
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
