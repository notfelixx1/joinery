//! Streaming file checksums (SHA-256, SHA-1, MD5).
//!
//! Files are read in fixed-size chunks so even large files never load
//! fully into memory — important for low-end machines.

use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use md5::Md5;
use serde::{Deserialize, Serialize};
use sha1::Sha1;
use sha2::{Digest, Sha256};

/// Size of one read chunk (64 KiB).
pub const BUFFER_SIZE: usize = 64 * 1024;

/// Number of bytes hashed for the cheap "partial" pass used by the
/// duplicate finder before committing to a full read.
pub const PARTIAL_HASH_BYTES: u64 = 4 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Algorithm {
    Sha256,
    Sha1,
    Md5,
}

impl Algorithm {
    /// Parse a user-supplied algorithm name (`sha256`, `SHA-1`, `md5`, ...).
    pub fn parse(value: &str) -> Option<Algorithm> {
        match value.to_ascii_lowercase().replace('-', "").as_str() {
            "sha256" | "sha2" => Some(Algorithm::Sha256),
            "sha1" => Some(Algorithm::Sha1),
            "md5" => Some(Algorithm::Md5),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Algorithm::Sha256 => "sha256",
            Algorithm::Sha1 => "sha1",
            Algorithm::Md5 => "md5",
        }
    }
}

fn hash_with<D: Digest>(path: &Path, mut hasher: D, limit: Option<u64>) -> std::io::Result<String> {
    let file = File::open(path)?;
    let file_len = file.metadata().map(|m| m.len()).unwrap_or(u64::MAX);
    let remaining_limit = limit.map(|l| l.min(file_len));
    let mut reader = BufReader::with_capacity(BUFFER_SIZE, file);
    let mut buffer = [0u8; BUFFER_SIZE];
    let mut taken: u64 = 0;
    loop {
        // Never read past the optional byte limit.
        let max_read = match remaining_limit {
            Some(limit) => {
                let left = limit.saturating_sub(taken);
                if left == 0 {
                    break;
                }
                left.min(buffer.len() as u64) as usize
            }
            None => buffer.len(),
        };
        let chunk = &mut buffer[..max_read];
        let read = reader.read(chunk)?;
        if read == 0 {
            break;
        }
        hasher.update(&chunk[..read]);
        taken += read as u64;
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Full hash of a file.
pub fn hash_file(path: &Path, algorithm: Algorithm) -> std::io::Result<String> {
    match algorithm {
        Algorithm::Sha256 => hash_with(path, Sha256::new(), None),
        Algorithm::Sha1 => hash_with(path, Sha1::new(), None),
        Algorithm::Md5 => hash_with(path, Md5::new(), None),
    }
}

/// Hash of at most `limit` leading bytes — a cheap pre-filter for the
/// duplicate finder.
pub fn hash_prefix(path: &Path, algorithm: Algorithm, limit: u64) -> std::io::Result<String> {
    match algorithm {
        Algorithm::Sha256 => hash_with(path, Sha256::new(), Some(limit)),
        Algorithm::Sha1 => hash_with(path, Sha1::new(), Some(limit)),
        Algorithm::Md5 => hash_with(path, Md5::new(), Some(limit)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_temp(content: &[u8]) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().expect("temp file");
        file.write_all(content).expect("write");
        file
    }

    #[test]
    fn sha256_known_vector() {
        let file = write_temp(b"abc");
        let hash = hash_file(file.path(), Algorithm::Sha256).expect("hash");
        assert_eq!(
            hash,
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[test]
    fn sha1_known_vector() {
        let file = write_temp(b"abc");
        let hash = hash_file(file.path(), Algorithm::Sha1).expect("hash");
        assert_eq!(hash, "a9993e364706816aba3e25717850c26c9cd0d89d");
    }

    #[test]
    fn md5_known_vector() {
        let file = write_temp(b"abc");
        let hash = hash_file(file.path(), Algorithm::Md5).expect("hash");
        assert_eq!(hash, "900150983cd24fb0d6963f7d28e17f72");
    }

    #[test]
    fn empty_file_sha256() {
        let file = write_temp(b"");
        let hash = hash_file(file.path(), Algorithm::Sha256).expect("hash");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn large_file_streams_correctly() {
        // 3 chunks + a remainder, larger than one 64 KiB buffer.
        let mut content = Vec::with_capacity(BUFFER_SIZE * 3 + 17);
        for i in 0..BUFFER_SIZE * 3 + 17 {
            content.push((i % 251) as u8);
        }
        let file = write_temp(&content);
        let hash = hash_file(file.path(), Algorithm::Sha256).expect("hash");
        // Same as hashing the whole content at once.
        let mut expected = Sha256::new();
        expected.update(&content);
        assert_eq!(hash, hex::encode(expected.finalize()));
    }

    #[test]
    fn prefix_is_stable_prefix_of_full_hash() {
        let mut content = vec![7u8; PARTIAL_HASH_BYTES as usize + 512];
        content[0] = 1;
        let file = write_temp(&content);
        let prefix = hash_prefix(file.path(), Algorithm::Sha256, PARTIAL_HASH_BYTES).expect("hash");
        // A file sharing the first bytes must share the prefix hash.
        let mut same_start = content.clone();
        same_start[PARTIAL_HASH_BYTES as usize + 10] ^= 0xFF;
        let file2 = write_temp(&same_start);
        let prefix2 =
            hash_prefix(file2.path(), Algorithm::Sha256, PARTIAL_HASH_BYTES).expect("hash");
        assert_eq!(prefix, prefix2);
        // ...but the full hashes must differ.
        let full1 = hash_file(file.path(), Algorithm::Sha256).expect("hash");
        let full2 = hash_file(file2.path(), Algorithm::Sha256).expect("hash");
        assert_ne!(full1, full2);
    }

    #[test]
    fn parse_algorithm_names() {
        assert_eq!(Algorithm::parse("SHA-256"), Some(Algorithm::Sha256));
        assert_eq!(Algorithm::parse("sha1"), Some(Algorithm::Sha1));
        assert_eq!(Algorithm::parse("MD5"), Some(Algorithm::Md5));
        assert_eq!(Algorithm::parse("crc32"), None);
    }
}
