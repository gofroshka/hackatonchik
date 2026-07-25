use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

pub fn detect_content_type(name: &str, data: &[u8]) -> String {
    infer::get(data)
        .map(|kind| kind.mime_type().to_owned())
        .or_else(|| mime_guess::from_path(name).first_raw().map(str::to_owned))
        .unwrap_or_else(|| "application/octet-stream".to_owned())
}

pub fn is_chat_content_type(content_type: &str) -> bool {
    content_type
        .split(';')
        .next()
        .is_some_and(|value| value.trim().eq_ignore_ascii_case("text/x-sonic-chat"))
}

pub fn safe_file_name(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("received.bin");
    let cleaned: String = base
        .chars()
        .map(|character| {
            if character.is_control() || matches!(character, '/' | '\\' | ':') {
                '_'
            } else {
                character
            }
        })
        .collect();
    if cleaned.trim_matches(['.', ' ']).is_empty() {
        "received.bin".to_owned()
    } else {
        truncate_utf8(&cleaned, 96)
    }
}

pub(super) fn transfer_id(hash: &[u8; 32]) -> u64 {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut hasher = Sha256::new();
    hasher.update(hash);
    hasher.update(nanos.to_le_bytes());
    let digest = hasher.finalize();
    u64::from_le_bytes(digest[..8].try_into().expect("SHA-256 has eight bytes"))
}

pub(super) fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let mut end = max_bytes;
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    value[..end].to_owned()
}
