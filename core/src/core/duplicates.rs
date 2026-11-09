//! Duplicate Finder core.
//!
//! Three-pass strategy tuned for slow disks and big folders:
//! 1. group files by size (cheap, metadata only);
//! 2. hash the first few KiB of same-size candidates;
//! 3. hash the full content of the survivors.
//!
//! Results are grouped and sorted by wasted space. Nothing is deleted
//! here — the UI shows the groups and deletion happens only after an
//! explicit confirmation, into the system trash.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

use crate::core::checksum::{
    hash_bytes, hash_file, hash_prefix, Algorithm, INLINE_HASH_LIMIT, PARTIAL_HASH_BYTES,
};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateFile {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateGroup {
    pub hash: String,
    pub size: u64,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStats {
    pub scanned_files: u64,
    pub skipped_files: u64,
    pub duplicate_groups: u64,
    pub wasted_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub groups: Vec<DuplicateGroup>,
    pub stats: ScanStats,
}

fn group_by<F>(paths: &[PathBuf], hash: F, skipped: &mut u64) -> HashMap<String, Vec<PathBuf>>
where
    F: Fn(&Path) -> io::Result<String>,
{
    let mut groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
    for path in paths {
        match hash(path) {
            Ok(digest) => groups.entry(digest).or_default().push(path.clone()),
            // Unreadable files are skipped, never fatal — and counted,
            // so the statistics stay honest.
            Err(_) => *skipped += 1,
        }
    }
    groups
}

/// Find duplicate files under `root`.
///
/// `min_size` filters out tiny files (default 1 byte — zero-byte files
/// are all "identical" but rarely interesting). `progress` is called
/// with the number of files discovered so far.
pub fn find_duplicates(
    root: &Path,
    min_size: u64,
    progress: &mut dyn FnMut(u64),
) -> io::Result<ScanResult> {
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut scanned: u64 = 0;
    let mut skipped: u64 = 0;

    for entry in WalkDir::new(root).follow_links(false) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        let size = match entry.metadata() {
            Ok(meta) => meta.len(),
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if size < min_size {
            continue;
        }
        by_size.entry(size).or_default().push(entry.into_path());
        scanned += 1;
        if scanned % 500 == 0 {
            progress(scanned);
        }
    }
    progress(scanned);

    // Sort candidate groups by reclaimable space (largest first).
    let mut candidates: Vec<(u64, Vec<PathBuf>)> = by_size
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .collect();
    candidates.sort_by(|a, b| {
        let wasted_a = a.0.saturating_mul(a.1.len() as u64 - 1);
        let wasted_b = b.0.saturating_mul(b.1.len() as u64 - 1);
        wasted_b.cmp(&wasted_a).then_with(|| b.0.cmp(&a.0))
    });

    let mut groups: Vec<DuplicateGroup> = Vec::new();
    for (size, paths) in candidates {
        // Files up to the inline limit are read ONCE and fully hashed
        // from memory — the partial pass is subsumed, halving the file
        // opens for the common small-file case. Larger files keep the
        // two-pass flow (partial 4 KiB pre-filter, then full hash).
        let inline_limit = INLINE_HASH_LIMIT.min(size);
        let (inline, streamed): (Vec<PathBuf>, Vec<PathBuf>) =
            paths.into_iter().partition(|_| size <= inline_limit);

        let mut inline_groups: HashMap<String, Vec<PathBuf>> = HashMap::new();
        for path in &inline {
            match fs::read(path) {
                Ok(data) => {
                    inline_groups
                        .entry(hash_bytes(&data))
                        .or_default()
                        .push(path.clone());
                }
                Err(_) => skipped += 1,
            }
        }
        for (hash, duplicates) in inline_groups {
            if duplicates.len() < 2 {
                continue;
            }
            let files = duplicates
                .into_iter()
                .map(|path| DuplicateFile { path, size })
                .collect();
            groups.push(DuplicateGroup { hash, size, files });
        }

        if streamed.is_empty() {
            continue;
        }
        // Pass 2: cheap partial hash.
        let partial = group_by(
            &streamed,
            |p| hash_prefix(p, Algorithm::Sha256, PARTIAL_HASH_BYTES.min(size)),
            &mut skipped,
        );
        for (_, similar_start) in partial {
            if similar_start.len() < 2 {
                continue;
            }
            // Pass 3: full hash.
            let full = group_by(
                &similar_start,
                |p| hash_file(p, Algorithm::Sha256),
                &mut skipped,
            );
            for (hash, duplicates) in full {
                if duplicates.len() < 2 {
                    continue;
                }
                let files = duplicates
                    .into_iter()
                    .map(|path| DuplicateFile { path, size })
                    .collect();
                groups.push(DuplicateGroup { hash, size, files });
            }
        }
    }

    groups.sort_by(|a, b| {
        let wasted_a = a.size.saturating_mul(a.files.len() as u64 - 1);
        let wasted_b = b.size.saturating_mul(b.files.len() as u64 - 1);
        wasted_b.cmp(&wasted_a)
    });

    let wasted_bytes = groups
        .iter()
        .map(|g| g.size.saturating_mul(g.files.len() as u64 - 1))
        .fold(0u64, u64::saturating_add);

    Ok(ScanResult {
        stats: ScanStats {
            scanned_files: scanned,
            skipped_files: skipped,
            duplicate_groups: groups.len() as u64,
            wasted_bytes,
        },
        groups,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn write(dir: &Path, name: &str, content: &[u8]) -> PathBuf {
        let path = dir.join(name);
        let mut file = File::create(&path).expect("create");
        file.write_all(content).expect("write");
        path
    }

    #[test]
    fn finds_duplicates_grouped_by_content() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "a.bin", b"same content here");
        write(dir.path(), "b.bin", b"same content here");
        write(dir.path(), "c.bin", b"totally different");

        let result = find_duplicates(dir.path(), 1, &mut |_| {}).expect("scan");
        assert_eq!(result.stats.scanned_files, 3);
        assert_eq!(result.groups.len(), 1);
        let group = &result.groups[0];
        assert_eq!(group.size, 17);
        assert_eq!(group.files.len(), 2);
        assert_eq!(result.stats.wasted_bytes, 17);
    }

    #[test]
    fn min_size_filters_small_files() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "small.bin", b"same");
        write(dir.path(), "small2.bin", b"same");

        let result = find_duplicates(dir.path(), 10, &mut |_| {}).expect("scan");
        assert!(result.groups.is_empty());
        assert_eq!(result.stats.scanned_files, 0);
    }

    #[test]
    fn nested_directories_are_scanned() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sub = dir.path().join("nested");
        std::fs::create_dir(&sub).expect("mkdir");
        write(dir.path(), "one.txt", b"duplicate body");
        write(&sub, "two.txt", b"duplicate body");

        let result = find_duplicates(dir.path(), 1, &mut |_| {}).expect("scan");
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].files.len(), 2);
    }

    #[test]
    fn distinct_content_same_size_not_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "x.bin", b"aaaaaaa");
        write(dir.path(), "y.bin", b"bbbbbbb");

        let result = find_duplicates(dir.path(), 1, &mut |_| {}).expect("scan");
        assert!(result.groups.is_empty());
    }

    #[test]
    fn groups_sorted_by_wasted_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        // Small duplicates: 3 × 10 bytes → 20 wasted.
        for name in ["s1", "s2", "s3"] {
            write(dir.path(), &format!("{name}.bin"), b"0123456789");
        }
        // Large duplicates: 2 × 100 bytes → 100 wasted.
        let big = vec![b'x'; 100];
        write(dir.path(), "b1.bin", &big);
        write(dir.path(), "b2.bin", &big);

        let result = find_duplicates(dir.path(), 1, &mut |_| {}).expect("scan");
        assert_eq!(result.groups.len(), 2);
        assert_eq!(result.groups[0].size, 100, "largest waste first");
        assert_eq!(result.groups[1].size, 10);
    }

    #[test]
    fn hash_failures_are_counted_as_skipped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let a = write(dir.path(), "a.bin", b"one");
        let b = write(dir.path(), "b.bin", b"two");

        let mut skipped = 0u64;
        let groups = group_by(
            &[a, b],
            |_| Err(io::Error::other("unreadable")),
            &mut skipped,
        );
        assert!(groups.is_empty());
        assert_eq!(skipped, 2);
    }
}
