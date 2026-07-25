use super::geometry::shard_geometry;
use super::packet::{decode_packet, Packet};
use super::*;

#[test]
fn reconstructs_binary_file_with_four_missing_shards_per_group() {
    let data: Vec<u8> = (0..5000u32).map(|value| (value * 31) as u8).collect();
    let plan = build_transfer("photo.bin", "application/octet-stream", &data, false)
        .expect("build transfer");
    let mut receiver = TransferReceiver::new();
    let mut completed = None;

    for packet in plan.packets {
        if let Ok(Packet::Shard { group, index, .. }) = decode_packet(&packet) {
            if index < 4 && group % 2 == 0 {
                continue;
            }
        }
        for event in receiver.ingest(&packet) {
            if let TransferEvent::Completed { data, .. } = event {
                completed = Some(data);
            }
        }
    }
    assert_eq!(completed, Some(data));
}

#[test]
fn ignores_duplicates_and_restores_compressed_unicode_text() {
    let text = "Привет через акустический канал. ".repeat(200).into_bytes();
    let plan = build_transfer("message.txt", "text/plain; charset=utf-8", &text, true)
        .expect("build transfer");
    assert_eq!(plan.metadata.compression, Compression::Zstd);
    let mut receiver = TransferReceiver::new();
    let mut completed = None;
    for packet in plan.packets {
        let duplicate = packet.clone();
        for event in receiver
            .ingest(&packet)
            .into_iter()
            .chain(receiver.ingest(&duplicate))
        {
            if let TransferEvent::Completed { data, .. } = event {
                completed = Some(data);
            }
        }
    }
    assert_eq!(completed, Some(text));
}

#[test]
fn medium_transfer_uses_adaptive_geometry_and_survives_two_losses() {
    let data: Vec<u8> = (0..300u16).map(|value| (value * 73 + 19) as u8).collect();
    let plan = build_transfer("medium.bin", "application/octet-stream", &data, false)
        .expect("build transfer");
    assert_eq!(shard_geometry(plan.metadata.encoded_size), (2, 2));
    let mut receiver = TransferReceiver::new();
    let mut completed = None;
    for packet in plan.packets {
        if let Ok(Packet::Shard { index, .. }) = decode_packet(&packet) {
            if index < 2 {
                continue;
            }
        }
        for event in receiver.ingest(&packet) {
            if let TransferEvent::Completed { data, .. } = event {
                completed = Some(data);
            }
        }
    }
    assert_eq!(completed, Some(data));
}

#[test]
fn replays_shards_that_arrive_before_manifest() {
    let data = (0..4096)
        .map(|value| (value * 29) as u8)
        .collect::<Vec<_>>();
    let plan = build_transfer("reordered.bin", "application/octet-stream", &data, false).unwrap();
    let manifest = plan
        .packets
        .iter()
        .find(|packet| matches!(decode_packet(packet), Ok(Packet::Manifest(_))))
        .unwrap();
    let mut reordered = plan
        .packets
        .iter()
        .filter(|packet| matches!(decode_packet(packet), Ok(Packet::Shard { .. })))
        .cloned()
        .collect::<Vec<_>>();
    reordered.push(manifest.clone());

    let mut receiver = TransferReceiver::new();
    let completed = reordered
        .iter()
        .flat_map(|packet| receiver.ingest(packet))
        .find_map(|event| match event {
            TransferEvent::Completed { data, .. } => Some(data),
            _ => None,
        });
    assert_eq!(completed.as_deref(), Some(data.as_slice()));
}

#[test]
fn sanitizes_received_file_names() {
    assert_eq!(safe_file_name("../../secret.txt"), "secret.txt");
    assert_eq!(safe_file_name(".."), "received.bin");
}
