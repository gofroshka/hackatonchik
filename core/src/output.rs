//! Safe persistence of completed transfers.

use std::io;
use std::path::{Path, PathBuf};

use crate::transfer::{safe_file_name, TransferMetadata};

pub fn save_received(
    output_dir: &Path,
    metadata: &TransferMetadata,
    data: &[u8],
) -> io::Result<PathBuf> {
    std::fs::create_dir_all(output_dir)?;
    let safe_name = safe_file_name(&metadata.name);
    let destination = unique_path(output_dir, &safe_name);
    let partial_name = format!(".{safe_name}.{:016x}.partial", metadata.id);
    let partial = output_dir.join(partial_name);
    std::fs::write(&partial, data)?;
    std::fs::rename(&partial, &destination)?;
    Ok(destination)
}

fn unique_path(directory: &Path, name: &str) -> PathBuf {
    let initial = directory.join(name);
    if !initial.exists() {
        return initial;
    }
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("received");
    let extension = path.extension().and_then(|value| value.to_str());
    for index in 1u32.. {
        let candidate = match extension {
            Some(extension) => directory.join(format!("{stem}-{index}.{extension}")),
            None => directory.join(format!("{stem}-{index}")),
        };
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("u32 file-name suffixes exhausted")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transfer::Compression;

    #[test]
    fn creates_unique_safe_file_names() {
        let root =
            std::env::temp_dir().join(format!("acoustic-output-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let metadata = TransferMetadata {
            id: 1,
            name: "../data.bin".to_owned(),
            content_type: "application/octet-stream".to_owned(),
            original_size: 3,
            encoded_size: 3,
            sha256: [0; 32],
            compression: Compression::None,
            group_count: 1,
        };
        let first = save_received(&root, &metadata, b"one").expect("first save");
        let second = save_received(&root, &metadata, b"two").expect("second save");
        assert_eq!(first.file_name().unwrap(), "data.bin");
        assert_eq!(second.file_name().unwrap(), "data-1.bin");
        let _ = std::fs::remove_dir_all(root);
    }
}
