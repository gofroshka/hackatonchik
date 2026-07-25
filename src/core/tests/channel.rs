//! Stress the codec through a simulated acoustic channel: speaker/mic
//! frequency rolloff, room echo (multipath), additive noise, DC + AGC gain,
//! soft clipping, sample-rate drift and a random leading offset.

use sonic_share_core::{
    decode_all, decode_all_ofdm, decode_all_with_profile, encode, encode_ofdm, encode_with_profile,
    AcousticProfile, OfdmProfile, ENCODE_SR,
};

struct Rng(u32);
impl Rng {
    fn next_f32(&mut self) -> f32 {
        // xorshift -> [-1, 1)
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        (self.0 as f32 / u32::MAX as f32) * 2.0 - 1.0
    }
}

/// One-pole low-pass (speaker/mic rolloff).
fn lowpass(x: &[f32], alpha: f32) -> Vec<f32> {
    let mut y = Vec::with_capacity(x.len());
    let mut prev = 0.0;
    for &s in x {
        prev += alpha * (s - prev);
        y.push(prev);
    }
    y
}

/// One-pole high-pass (removes rumble / DC coupling of the room).
fn highpass(x: &[f32], alpha: f32) -> Vec<f32> {
    let mut y = Vec::with_capacity(x.len());
    let mut prev_x = 0.0;
    let mut prev_y = 0.0;
    for &s in x {
        let out = alpha * (prev_y + s - prev_x);
        y.push(out);
        prev_x = s;
        prev_y = out;
    }
    y
}

fn resample(input: &[f32], ratio: f32) -> Vec<f32> {
    let out_len = (input.len() as f32 * ratio) as usize;
    let mut out = Vec::with_capacity(out_len);
    for i in 0..out_len {
        let src = i as f32 / ratio;
        let idx = src.floor() as usize;
        let frac = src - idx as f32;
        let a = input[idx.min(input.len() - 1)];
        let b = input[(idx + 1).min(input.len() - 1)];
        out.push(a + (b - a) * frac);
    }
    out
}

/// Apply a full nasty channel. `seed` and `strength` vary the impairments.
fn channel(
    clean: &[f32],
    seed: u32,
    rate_ratio: f32,
    gain: f32,
    noise: f32,
    echo: f32,
) -> Vec<f32> {
    let mut rng = Rng(seed);

    // Speaker/mic band shaping.
    let source: Vec<f32> = clean.iter().map(|sample| sample.clamp(-1.0, 1.0)).collect();
    let mut y = lowpass(&source, 0.6); // output clipping + speaker rolloff
    y = highpass(&y, 0.98); // roll off lows / DC

    // Room echo (a couple of delayed reflections).
    let d1 = 673usize;
    let d2 = 1511usize;
    let mut echoed = y.clone();
    for n in d1..echoed.len() {
        echoed[n] += echo * y[n - d1];
    }
    for n in d2..echoed.len() {
        echoed[n] += 0.5 * echo * y[n - d2];
    }
    y = echoed;

    // AGC-ish gain + DC offset.
    let dc = 0.01;
    for s in y.iter_mut() {
        *s = *s * gain + dc;
    }

    // Additive noise.
    for s in y.iter_mut() {
        *s += rng.next_f32() * noise;
    }

    // Soft clipping (tanh-like).
    for s in y.iter_mut() {
        *s = s.tanh();
    }

    // Random leading silence/noise offset.
    let pad = 500 + (seed as usize % 1500);
    let mut out = Vec::with_capacity(pad + y.len());
    for _ in 0..pad {
        out.push(rng.next_f32() * noise);
    }
    out.extend_from_slice(&y);

    // Sample-rate drift (speaker clock != mic clock).
    resample(&out, rate_ratio)
}

fn try_msg(msg: &[u8], seed: u32, ratio: f32, noise: f32, echo: f32) -> bool {
    let clean = encode(msg);
    let rx = channel(&clean, seed, ratio, 0.35, noise, echo);
    // The mic "rate" is ENCODE_SR * ratio.
    let mic_sr = (ENCODE_SR as f32 * ratio).round() as u32;
    decode_all(mic_sr, &rx).into_iter().any(|m| m == msg)
}

fn try_profile(
    msg: &[u8],
    profile: AcousticProfile,
    seed: u32,
    ratio: f32,
    gain: f32,
    noise: f32,
    echo: f32,
) -> bool {
    let clean = encode_with_profile(msg, profile);
    let rx = channel(&clean, seed, ratio, gain, noise, echo);
    let mic_sr = (ENCODE_SR as f32 * ratio).round() as u32;
    decode_all_with_profile(mic_sr, &rx, profile)
        .into_iter()
        .any(|decoded| decoded == msg)
}

#[test]
fn mild_channel() {
    assert!(try_msg(b"hello over sound", 1, 1.0, 0.01, 0.15));
}

#[test]
fn moderate_channel() {
    assert!(try_msg(b"hello over sound", 7, 0.999, 0.03, 0.25));
}

#[test]
fn harsh_channel() {
    assert!(try_msg(b"hello over sound", 13, 1.001, 0.05, 0.35));
}

#[test]
fn many_seeds_success_rate() {
    let msg = b"hello over sound";
    let mut ok = 0;
    let trials = 60;
    for k in 0..trials {
        let ratio = 1.0 + ((k as f32 - 30.0) / 10000.0); // +/-3000ppm drift
        let noise = 0.03 + (k % 6) as f32 * 0.015; // up to ~0.11
        let echo = 0.15 + (k % 5) as f32 * 0.09; // up to ~0.51
        if try_msg(msg, 1000 + k * 7, ratio, noise, echo) {
            ok += 1;
        }
    }
    eprintln!("channel success: {ok}/{trials}");
    assert!(
        ok >= trials * 85 / 100,
        "success rate too low: {ok}/{trials}"
    );
}

#[test]
fn robust_profile_handles_attenuated_high_noise_channel() {
    let profile = AcousticProfile::robust(0);
    assert!(try_profile(
        b"maximum distance high noise",
        profile,
        77,
        1.0015,
        0.18,
        0.045,
        0.42,
    ));
}

#[test]
fn two_pairs_exchange_simultaneously_on_separate_lanes() {
    let first_profile = AcousticProfile::fast(0);
    let second_profile = AcousticProfile::fast(1);
    let first_message = b"pair A payload";
    let second_message = b"pair B payload";
    let first = encode_with_profile(first_message, first_profile);
    let second = encode_with_profile(second_message, second_profile);
    let length = first.len().max(second.len());
    let mut mixed = vec![0.0f32; length];
    for (output, sample) in mixed.iter_mut().zip(first) {
        *output += sample * 0.5;
    }
    for (output, sample) in mixed.iter_mut().zip(second) {
        *output += sample * 0.5;
    }
    let received = channel(&mixed, 91, 1.0, 0.55, 0.012, 0.12);

    assert!(decode_all_with_profile(ENCODE_SR, &received, first_profile)
        .into_iter()
        .any(|message| message == first_message));
    assert!(
        decode_all_with_profile(ENCODE_SR, &received, second_profile)
            .into_iter()
            .any(|message| message == second_message)
    );
}

#[test]
fn qpsk_ofdm_handles_noise_rolloff_and_echo() {
    let profile = OfdmProfile::qpsk(0);
    let message = (0..211)
        .map(|value| (value * 37 + 11) as u8)
        .collect::<Vec<_>>();
    let clean = encode_ofdm(&message, profile);
    let received = channel(&clean, 301, 1.0, 0.45, 0.008, 0.08);
    assert!(decode_all_ofdm(&received, profile)
        .into_iter()
        .any(|decoded| decoded == message));
}

#[test]
fn two_ofdm_lanes_decode_from_the_same_mix() {
    let first_profile = OfdmProfile::qpsk(0);
    let second_profile = OfdmProfile::qpsk(1);
    let first_message = vec![0x35; 211];
    let second_message = vec![0xCA; 211];
    let first = encode_ofdm(&first_message, first_profile);
    let second = encode_ofdm(&second_message, second_profile);
    let mut mixed = vec![0.0f32; first.len().max(second.len())];
    for (output, sample) in mixed.iter_mut().zip(first) {
        *output += sample * 0.6;
    }
    for (output, sample) in mixed.iter_mut().zip(second) {
        *output += sample * 0.55;
    }
    // Two well-separated lanes at similar level: the point is lane isolation,
    // not surviving an extreme channel (covered by the single-lane test).
    let received = channel(&mixed, 707, 1.0, 0.9, 0.003, 0.02);

    assert!(decode_all_ofdm(&received, first_profile)
        .into_iter()
        .any(|decoded| decoded == first_message));
    assert!(decode_all_ofdm(&received, second_profile)
        .into_iter()
        .any(|decoded| decoded == second_message));
}
