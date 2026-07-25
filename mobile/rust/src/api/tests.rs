use super::rx::RxSession;
use super::tx::TxSession;

#[test]
fn mobile_sessions_roundtrip_pcm16() {
    let mut tx = TxSession::from_data(
        "message.txt".to_owned(),
        Some("text/plain; charset=utf-8".to_owned()),
        b"hello from mobile sessions".to_vec(),
    )
    .expect("sender");
    let output = std::env::temp_dir().join(format!("sonic-share-ffi-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&output);
    let mut rx = RxSession::new(
        sonic_share_core::ENCODE_SR,
        output.to_string_lossy().into_owned(),
    );
    let mut completed = false;
    loop {
        let chunk = tx.next_pcm_chunk(4096);
        for event in rx.push_pcm16(chunk.pcm16_le) {
            if event.kind == "completed" {
                completed = true;
            }
        }
        if chunk.done {
            break;
        }
    }
    assert!(completed);
    let _ = std::fs::remove_dir_all(output);
}

#[test]
fn chat_message_is_returned_without_creating_a_file() {
    let mut tx = TxSession::from_data(
        "chat-message.txt".to_owned(),
        Some("text/x-sonic-chat; charset=utf-8".to_owned()),
        b"hello over sound".to_vec(),
    )
    .expect("sender");
    let output = std::env::temp_dir().join(format!("sonic-share-chat-ffi-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&output);
    let mut rx = RxSession::new(
        sonic_share_core::ENCODE_SR,
        output.to_string_lossy().into_owned(),
    );
    let completed = loop {
        let chunk = tx.next_pcm_chunk(4096);
        if let Some(event) = rx
            .push_pcm16(chunk.pcm16_le)
            .into_iter()
            .find(|event| event.kind == "completed")
        {
            break event;
        }
        assert!(!chunk.done, "chat transfer did not complete");
    };

    assert_eq!(completed.text.as_deref(), Some("hello over sound"));
    assert!(completed.path.is_none());
    assert!(!output.exists());
}
