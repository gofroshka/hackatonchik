use std::path::{Path, PathBuf};
use std::time::Instant;

use sonic_share_core::transfer::{
    build_transfer, detect_content_type, TransferEvent, TransferReceiver,
};
use sonic_share_core::{
    decode_all_with_profile, encode_hybrid_packet, encode_with_profile,
    encoded_sample_count_with_profile, hybrid_packet_routes, hybrid_sample_count, AcousticProfile,
    HybridDecoder, OfdmProfile, ENCODE_SR,
};

fn main() {
    let assets = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("assets"));
    if let Err(error) = run(&assets) {
        eprintln!("asset benchmark failed: {error}");
        std::process::exit(1);
    }
}

fn run(assets: &Path) -> Result<(), String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(assets)
        .map_err(|error| format!("cannot read '{}': {error}", assets.display()))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_file())
        .collect();
    paths.sort();
    if paths.is_empty() {
        return Err(format!("'{}' contains no files", assets.display()));
    }

    println!(
        "{:<20} {:>8} {:>8} {:>10} {:>10} {:>10} {:>10}",
        "asset", "bytes", "packets", "FSK fast", "FSK robust", "OFDM safe", "OFDM fast"
    );
    let started = Instant::now();
    for path in paths {
        benchmark_asset(&path)?;
    }
    println!("verified in {:.2?}", started.elapsed());
    Ok(())
}

fn benchmark_asset(path: &Path) -> Result<(), String> {
    let data = std::fs::read(path)
        .map_err(|error| format!("cannot read '{}': {error}", path.display()))?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("asset.bin");
    let content_type = detect_content_type(name, &data);
    let plan = build_transfer(name, &content_type, &data, true)
        .map_err(|error| format!("cannot packetize '{name}': {error}"))?;

    let fast = AcousticProfile::fast(0);
    let robust = AcousticProfile::robust(0);
    let fast_seconds = airtime(&plan.packets, fast);
    let robust_seconds = airtime(&plan.packets, robust);
    let qpsk = OfdmProfile::qpsk(0);
    let qam16 = OfdmProfile::qam16(0);
    let routes = hybrid_packet_routes(&plan.packets);
    let qpsk_seconds = hybrid_airtime(&plan.packets, &routes, fast, qpsk);
    let qam16_seconds = hybrid_airtime(&plan.packets, &routes, fast, qam16);

    verify_transfer(&plan.packets, &data, name)?;
    verify_representative_phy(&plan.packets, fast, name)?;
    verify_hybrid_phy(&plan.packets, &routes, &data, fast, qpsk, name)?;
    verify_hybrid_phy(&plan.packets, &routes, &data, fast, qam16, name)?;

    // Regression guards, not the marketing target. The reliable default
    // (rep-2 DBPSK) trades rate for surviving a real acoustic path.
    if data.len() >= 20_000 && qpsk_seconds > 400.0 {
        return Err(format!(
            "reliable OFDM airtime regressed for '{name}': {qpsk_seconds:.1}s"
        ));
    }
    if data.len() >= 20_000 && qam16_seconds > 200.0 {
        return Err(format!(
            "fast OFDM airtime regressed for '{name}': {qam16_seconds:.1}s"
        ));
    }

    println!(
        "{name:<20} {:>8} {:>8} {:>9.1}s {:>9.1}s {:>9.1}s {:>9.1}s",
        data.len(),
        plan.packets.len(),
        fast_seconds,
        robust_seconds,
        qpsk_seconds,
        qam16_seconds,
    );
    Ok(())
}

fn hybrid_airtime(
    packets: &[Vec<u8>],
    routes: &[sonic_share_core::PacketPhy],
    fsk: AcousticProfile,
    ofdm: OfdmProfile,
) -> f64 {
    packets
        .iter()
        .zip(routes)
        .map(|(packet, &route)| hybrid_sample_count(packet.len(), route, fsk, ofdm) as f64)
        .sum::<f64>()
        / ENCODE_SR as f64
}

fn airtime(packets: &[Vec<u8>], profile: AcousticProfile) -> f64 {
    packets
        .iter()
        .map(|packet| encoded_sample_count_with_profile(packet.len(), profile) as f64)
        .sum::<f64>()
        / ENCODE_SR as f64
}

fn verify_transfer(packets: &[Vec<u8>], expected: &[u8], name: &str) -> Result<(), String> {
    let mut receiver = TransferReceiver::new();
    let mut completed = None;
    for packet in packets {
        for event in receiver.ingest(packet) {
            if let TransferEvent::Completed { data, .. } = event {
                completed = Some(data);
            }
        }
    }
    if completed.as_deref() != Some(expected) {
        return Err(format!("transfer verification failed for '{name}'"));
    }
    Ok(())
}

fn verify_representative_phy(
    packets: &[Vec<u8>],
    profile: AcousticProfile,
    name: &str,
) -> Result<(), String> {
    let mut representatives = Vec::new();
    if let Some(first) = packets.first() {
        representatives.push(first);
    }
    if let Some(largest) = packets.iter().max_by_key(|packet| packet.len()) {
        if !representatives.contains(&largest) {
            representatives.push(largest);
        }
    }
    if let Some(last) = packets.last() {
        if !representatives.contains(&last) {
            representatives.push(last);
        }
    }

    for packet in representatives {
        let wave = encode_with_profile(packet, profile);
        if !decode_all_with_profile(ENCODE_SR, &wave, profile)
            .iter()
            .any(|decoded| decoded == packet)
        {
            return Err(format!("fast PHY verification failed for '{name}'"));
        }
    }
    Ok(())
}

fn verify_hybrid_phy(
    packets: &[Vec<u8>],
    routes: &[sonic_share_core::PacketPhy],
    expected: &[u8],
    fsk: AcousticProfile,
    ofdm: OfdmProfile,
    name: &str,
) -> Result<(), String> {
    let mut decoder = HybridDecoder::new(ofdm);
    let mut receiver = TransferReceiver::new();
    let mut completed = None;
    for (packet, &route) in packets.iter().zip(routes) {
        let waveform = encode_hybrid_packet(packet, route, fsk, ofdm);
        decoder.push(&waveform);
        while let Some(decoded) = decoder.poll() {
            for event in receiver.ingest(&decoded) {
                if let TransferEvent::Completed { data, .. } = event {
                    completed = Some(data);
                }
            }
        }
    }
    if completed.as_deref() != Some(expected) {
        return Err(format!("hybrid PHY verification failed for '{name}'"));
    }
    Ok(())
}
