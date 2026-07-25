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
fn sanitizes_received_file_names() {
    assert_eq!(safe_file_name("../../secret.txt"), "secret.txt");
    assert_eq!(safe_file_name(".."), "received.bin");
}

#[test]
fn encrypted_transfer_full_roundtrip() {
    use crate::crypto;
    let session_key = crypto::generate_key();
    let data: Vec<u8> = (0..5000u32).map(|v| (v * 31) as u8).collect();
    let plan =
        build_transfer_encrypted("secret.bin", "application/octet-stream", &data, false, &session_key)
            .expect("build encrypted transfer");
    let mut receiver = TransferReceiver::new();
    receiver.set_session_key(session_key);
    let mut completed = None;
    for packet in plan.packets {
        for event in receiver.ingest(&packet) {
            if let TransferEvent::Completed { data, .. } = event {
                completed = Some(data);
            }
        }
    }
    assert_eq!(completed, Some(data));
}

#[test]
fn encrypted_transfer_with_compression() {
    use crate::crypto;
    let session_key = crypto::generate_key();
    let text = "Sonic Share encrypted! ".repeat(300).into_bytes();
    let plan =
        build_transfer_encrypted("secret.txt", "text/plain", &text, true, &session_key)
            .expect("build encrypted transfer with compression");
    assert_eq!(plan.metadata.compression, Compression::Zstd);
    let mut receiver = TransferReceiver::new();
    receiver.set_session_key(session_key);
    let mut completed = None;
    for packet in plan.packets {
        for event in receiver.ingest(&packet) {
            if let TransferEvent::Completed { data, .. } = event {
                completed = Some(data);
            }
        }
    }
    assert_eq!(completed, Some(text));
}

#[test]
fn encrypted_transfer_fails_with_wrong_key() {
    use crate::crypto;
    let enc_key = crypto::generate_key();
    let dec_key = crypto::generate_key();
    assert_ne!(enc_key, dec_key);
    let data = b"hello encrypted world".to_vec();
    let plan =
        build_transfer_encrypted("test.bin", "application/octet-stream", &data, false, &enc_key)
            .expect("build encrypted transfer");
    let mut receiver = TransferReceiver::new();
    receiver.set_session_key(dec_key);
    let mut failed = false;
    for packet in plan.packets {
        for event in receiver.ingest(&packet) {
            if let TransferEvent::Failed { .. } = event {
                failed = true;
            }
        }
    }
    assert!(failed, "expected decryption to fail with wrong key");
}

#[test]
fn encrypted_transfer_fails_with_no_key() {
    use crate::crypto;
    let session_key = crypto::generate_key();
    let data = b"secret data".to_vec();
    let plan =
        build_transfer_encrypted("test.bin", "application/octet-stream", &data, false, &session_key)
            .expect("build encrypted transfer");
    let mut receiver = TransferReceiver::new();
    let mut failed = false;
    for packet in plan.packets {
        for event in receiver.ingest(&packet) {
            if let TransferEvent::Failed { .. } = event {
                failed = true;
            }
        }
    }
    assert!(failed, "expected decryption to fail with no key set");
}

#[test]
fn encrypted_inline_transfer() {
    use crate::crypto;
    let session_key = crypto::generate_key();
    let data = b"tiny".to_vec();
    let plan =
        build_transfer_encrypted("tiny.txt", "text/plain", &data, false, &session_key)
            .expect("build encrypted inline transfer");
    assert_eq!(plan.packets.len(), 1, "inline transfer should be a single packet");
    let mut receiver = TransferReceiver::new();
    receiver.set_session_key(session_key);
    let mut completed = None;
    for event in receiver.ingest(&plan.packets[0]) {
        if let TransferEvent::Completed { data, .. } = event {
            completed = Some(data);
        }
    }
    assert_eq!(completed, Some(data));
}

#[test]
fn encrypted_transfer_with_manual_key_exchange() {
    use crate::crypto;
    let alice = crypto::generate_keypair();
    let bob = crypto::generate_keypair();
    let alice_pk = alice.public;
    let bob_pk = bob.public;
    let alice_key = crypto::derive_session_key(alice, &bob_pk);
    let bob_key = crypto::derive_session_key(bob, &alice_pk);
    assert_eq!(alice_key, bob_key);
    let data: Vec<u8> = (0..1000u32).map(|v| v as u8).collect();
    let plan = build_transfer_encrypted(
        "dh-test.bin", "application/octet-stream", &data, false, &alice_key,
    ).expect("build encrypted transfer");
    let mut receiver = TransferReceiver::new();
    receiver.set_session_key(bob_key);
    let mut completed = None;
    for packet in plan.packets {
        for event in receiver.ingest(&packet) {
            if let TransferEvent::Completed { data, .. } = event {
                completed = Some(data);
            }
        }
    }
    assert_eq!(completed, Some(data));
}
