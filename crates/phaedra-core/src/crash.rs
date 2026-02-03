use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn save_crash(
    crash_dir: &Path,
    input: &[u8],
    status: &phaedra_harness::ExecutionStatus,
) -> Result<PathBuf> {
    std::fs::create_dir_all(crash_dir)?;

    let hash = crash_hash(input);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();

    let filename = format!("crash_{ts}_{hash}.bin");
    let path = crash_dir.join(&filename);

    std::fs::write(&path, input)?;

    tracing::warn!(
        "[CRASH] status={:?} size={}b saved={}",
        status,
        input.len(),
        filename
    );

    Ok(path)
}

fn crash_hash(data: &[u8]) -> String {
    let mut hash: u64 = 0xcbf29ce484222325;
    for &byte in data {
        hash ^= byte as u64;
        hash = hash.wrapping_mul(0x100000001b3);
    }
    format!("{:08x}", hash as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use phaedra_harness::ExecutionStatus;

    #[test]
    fn test_save_crash_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let input = b"crashing input";
        let path = save_crash(
            dir.path(),
            input,
            &ExecutionStatus::Crash { signal: None },
        )
        .unwrap();
        assert!(path.exists());
        assert_eq!(std::fs::read(&path).unwrap(), input);
    }

    #[test]
    fn test_crash_filename_format() {
        let dir = tempfile::tempdir().unwrap();
        let path = save_crash(
            dir.path(),
            b"test",
            &ExecutionStatus::Crash { signal: Some(11) },
        )
        .unwrap();
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("crash_"));
        assert!(name.ends_with(".bin"));
    }

    #[test]
    fn test_crash_hash_stable() {
        assert_eq!(crash_hash(b"hello"), crash_hash(b"hello"));
    }

    #[test]
    fn test_crash_hash_differs() {
        assert_ne!(crash_hash(b"hello"), crash_hash(b"world"));
    }
}
