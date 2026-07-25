//! Ground-truth OFDM loopback BER probe.
//!
//! `ofdm-probe send [--qam16]` plays three copies of a known OFDM packet.
//! `ofdm-probe check <wav> [--qam16]` measures the true bit error rate of the
//! captured frames against the known payload.

use sonic_share_cli::audio;
use sonic_share_core::{
    decode_all_ofdm, encode_ofdm, ofdm_expected_body_bits, ofdm_probe_frames, OfdmProfile,
    ENCODE_SR,
};

const PAYLOAD_LEN: usize = 200;

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
        _ => {
            eprintln!("usage: ofdm-probe send|check <wav> [--qam16]");
            std::process::exit(2);
        }
    }
}

fn send(profile: OfdmProfile) {
    let payload = known_payload();
    let one = encode_ofdm(&payload, profile);
    let gap = vec![0.0f32; ENCODE_SR as usize / 2];
    let mut wave = Vec::new();
    for _ in 0..3 {
        wave.extend_from_slice(&gap);
        wave.extend_from_slice(&one);
    }
    wave.extend_from_slice(&gap);
    println!(
        "playing 3x known {PAYLOAD_LEN}-byte OFDM packets ({:?})",
        profile.modulation()
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

    // The real metric: does RS + erasure fully recover the payload?
    let recovered = decode_all_ofdm(&wav.samples, profile)
        .iter()
        .filter(|p| p.as_slice() == payload.as_slice())
        .count();
    println!("full decode: {recovered} frame(s) recovered the exact payload");
}

fn ofdm_bits_per_carrier(profile: OfdmProfile) -> usize {
    match profile.modulation() {
        sonic_share_core::OfdmModulation::Qpsk => 1,
        sonic_share_core::OfdmModulation::Qam16 => 2,
    }
}
