//! Ground-truth OFDM loopback BER probe.
//!
//! `ofdm-probe send [--qam16]` plays three copies of a known OFDM packet.
//! `ofdm-probe check <wav> [--qam16]` measures the true bit error rate of the
//! captured frames against the known payload.

use sonic_share_cli::audio;
use sonic_share_core::{
    decode_all_ofdm, encode_ofdm, ofdm_channel_profile, ofdm_expected_body_bits, ofdm_probe_frames,
    OfdmProfile, ENCODE_SR,
};

const PAYLOAD_LEN: usize = 200;
const PACKET_COUNT: usize = 5;

fn known_payload() -> Vec<u8> {
    (0..PAYLOAD_LEN).map(|i| (i * 37 + 11) as u8).collect()
}

fn profile_from_args(args: &[String]) -> OfdmProfile {
    if args.iter().any(|a| a == "--qam16") {
        OfdmProfile::qam16(0)
    } else {
        OfdmProfile::qpsk(0)
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let profile = profile_from_args(&args);
    match args.first().map(String::as_str) {
        Some("send") => send(profile),
        Some("check") => {
            let path = args
                .iter()
                .skip(1)
                .find(|a| !a.starts_with("--"))
                .expect("check requires a WAV path");
            check(path, profile);
        }
        Some("profile") => {
            let path = args
                .iter()
                .skip(1)
                .find(|a| !a.starts_with("--"))
                .expect("profile requires a WAV path");
            channel_profile(path, profile);
        }
        _ => {
            eprintln!("usage: ofdm-probe send|check <wav> [--qam16]");
            std::process::exit(2);
        }
    }
}

fn send(profile: OfdmProfile) {
    let payload = known_payload();
    let one = encode_ofdm(&payload, profile);
    let gap = vec![0.0f32; ENCODE_SR as usize * 2 / 5];
    let mut wave = Vec::new();
    for _ in 0..PACKET_COUNT {
        wave.extend_from_slice(&gap);
        wave.extend_from_slice(&one);
    }
    wave.extend_from_slice(&gap);
    println!(
        "playing {PACKET_COUNT}x known {PAYLOAD_LEN}-byte OFDM packets ({:?}, {:.1}s)",
        profile.modulation(),
        wave.len() as f32 / ENCODE_SR as f32
    );
    audio::play_samples(&wave).unwrap_or_else(|error| {
        eprintln!("playback failed: {error}");
        std::process::exit(1);
    });
    println!("done");
}

fn check(path: &str, profile: OfdmProfile) {
    let wav = sonic_share_cli::wav::read_mono(std::path::Path::new(path)).unwrap_or_else(|error| {
        eprintln!("cannot read '{path}': {error}");
        std::process::exit(2);
    });
    let payload = known_payload();
    let expected = ofdm_expected_body_bits(&payload, profile);
    let frames = ofdm_probe_frames(&wav.samples, profile);
    let bpc = ofdm_bits_per_carrier(profile);
    println!(
        "{}: {} Hz, {:.2}s, detected {} OFDM frame(s)",
        path,
        wav.sample_rate,
        wav.samples.len() as f32 / wav.sample_rate as f32,
        frames.len()
    );
    for (index, frame) in frames.iter().enumerate() {
        if frame.payload_len != PAYLOAD_LEN {
            println!("  frame {index}: payload_len={} (skip)", frame.payload_len);
            continue;
        }
        let compared = expected.len().min(frame.body_bits.len());
        let symbols = compared / (64 * bpc);
        let errors = (0..compared)
            .filter(|&i| expected[i] != frame.body_bits[i])
            .count();
        let ber = errors as f32 / compared as f32;
        let mut carrier_errors = [0u32; 64];
        for (i, (&e, &b)) in expected
            .iter()
            .zip(&frame.body_bits)
            .take(compared)
            .enumerate()
        {
            // Encoded-stream index -> carrier-major layout: (cell*S + s)*bpc + b.
            let cell = (i / bpc) / symbols.max(1);
            if cell < 64 && e != b {
                carrier_errors[cell] += 1;
            }
        }
        let mut ranked = (0..64).map(|c| (c, carrier_errors[c])).collect::<Vec<_>>();
        ranked.sort_by_key(|&(_, errs)| std::cmp::Reverse(errs));
        let worst = ranked.iter().take(8).copied().collect::<Vec<_>>();
        println!(
            "  frame {index}: {errors}/{compared} errors (BER {:.4}); worst(carrier,errs) {:?}",
            ber, worst
        );
    }

    // The real metric: how many of the sent packets fully recover?
    let recovered = decode_all_ofdm(&wav.samples, profile)
        .iter()
        .filter(|p| p.as_slice() == payload.as_slice())
        .count();
    println!("SWEEP recovered={recovered}/{PACKET_COUNT}");
}

fn channel_profile(path: &str, profile: OfdmProfile) {
    let wav = sonic_share_cli::wav::read_mono(std::path::Path::new(path)).unwrap_or_else(|error| {
        eprintln!("cannot read '{path}': {error}");
        std::process::exit(2);
    });
    let Some(mag) = ofdm_channel_profile(&wav.samples, profile) else {
        println!("no frame detected");
        return;
    };
    let max = mag.iter().cloned().fold(0.0f32, f32::max).max(1e-9);
    let median = {
        let mut s = mag.clone();
        s.sort_by(f32::total_cmp);
        s[s.len() / 2]
    };
    let weak = mag.iter().filter(|&&m| m < 0.4 * median).count();
    println!(
        "per-carrier channel magnitude ({} data carriers):",
        mag.len()
    );
    for (carrier, &m) in mag.iter().enumerate() {
        let bars = ((m / max) * 40.0) as usize;
        let mark = if m < 0.4 * median { " <== notch" } else { "" };
        println!("{carrier:2}: {:<40}{mark}", "#".repeat(bars));
    }
    println!(
        "median={median:.3} max={max:.3} weak(<0.4*median)={weak}/{}",
        mag.len()
    );
}

fn ofdm_bits_per_carrier(profile: OfdmProfile) -> usize {
    match profile.modulation() {
        sonic_share_core::OfdmModulation::Qpsk => 1,
        sonic_share_core::OfdmModulation::Qam16 => 2,
    }
}
