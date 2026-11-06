//! Folder Analyzer core — what is eating the disk space.
//!
//! One walk over the tree; every file's size is attributed to its
//! top-level entry so the UI can render a "biggest children first" list
//! without re-walking per entry.
//!
//! The walk never follows links. A top-level symlink (or a Windows
//! junction) is instead flagged as a link, so the report never presents
//! its uncounted target content as a silent zero.

use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use walkdir::WalkDir;

/// How many largest files to report.
pub const TOP_LARGEST: usize = 20;

/// A top-level entry the walk will not descend into: a symlink, or on
/// Windows also a junction (a mount-point reparse). std's `is_symlink`
/// recognizes only real symlinks, junctions are caught by their reparse
/// attribute.
fn is_link_entry(path: &Path) -> bool {
    let md = match fs::symlink_metadata(path) {
        Ok(md) => md,
        Err(_) => return false,
    };
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        if md.file_attributes()
            & windows_sys::Win32::Storage::FileSystem::FILE_ATTRIBUTE_REPARSE_POINT
            != 0
        {
            return true;
        }
    }
    md.file_type().is_symlink()
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryStat {
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    /// The entry (or its target) is a directory, but the walk never
    /// descends into links — a symlink (all platforms) or a junction
    /// (Windows). Such an entry's `size` covers only the link itself;
    /// the target's content stays uncounted, and the UI labels the row
    /// so a bare "0 bytes" folder is not misread.
    pub is_link: bool,
    /// Total size of the entry (for dirs — the whole subtree).
    pub size: u64,
    /// Number of files inside (0 for plain files).
    pub files: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileStat {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FolderAnalysis {
    pub root: PathBuf,
    pub total_files: u64,
    pub total_dirs: u64,
    pub total_size: u64,
    pub skipped: u64,
    pub entries: Vec<EntryStat>,
    pub largest: Vec<FileStat>,
}

/// Analyze `root`: per-entry sizes and the largest files in the tree.
pub fn analyze_folder(root: &Path) -> io::Result<FolderAnalysis> {
    // Top-level entries first — they define the report rows. The name
    // stays an OsString so lossy conversions cannot merge two distinct
    // entries into one report row.
    let mut top_level: Vec<(std::ffi::OsString, PathBuf, bool)> = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        let is_dir = fs::metadata(&path).map(|m| m.is_dir()).unwrap_or(false);
        top_level.push((entry.file_name().to_os_string(), path, is_dir));
    }

    // Single walk over the whole tree.
    let mut sizes: HashMap<std::ffi::OsString, (u64, u64)> = HashMap::new(); // name -> (size, files)
    let mut total_files: u64 = 0;
    let mut total_dirs: u64 = 0;
    let mut total_size: u64 = 0;
    let mut skipped: u64 = 0;
    let mut largest: Vec<(u64, PathBuf)> = Vec::new();

    for entry in WalkDir::new(root).follow_links(false) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };
        if entry.depth() == 0 {
            continue; // the root itself
        }
        let file_type = entry.file_type();
        if file_type.is_dir() {
            total_dirs += 1;
            continue;
        }
        if !file_type.is_file() {
            continue; // symlinks etc.
        }
        let size = match entry.metadata() {
            Ok(meta) => meta.len(),
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        total_files += 1;
        total_size = total_size.saturating_add(size);

        // Attribute to the top-level ancestor.
        let relative = entry.path().strip_prefix(root).unwrap_or(entry.path());
        let owner = relative
            .components()
            .next()
            .map(|c| c.as_os_str().to_os_string())
            .unwrap_or_default();
        let bucket = sizes.entry(owner).or_insert((0, 0));
        bucket.0 = bucket.0.saturating_add(size);
        bucket.1 += 1;

        // Keep the top-N largest files (min-heap pattern).
        largest.push((size, entry.into_path()));
        if largest.len() > TOP_LARGEST * 4 {
            largest.sort_by_key(|b| std::cmp::Reverse(b.0));
            largest.truncate(TOP_LARGEST);
        }
    }
    largest.sort_by_key(|b| std::cmp::Reverse(b.0));
    largest.truncate(TOP_LARGEST);

    let mut entries: Vec<EntryStat> = top_level
        .into_iter()
        .map(|(name, path, is_dir)| {
            let (size, files) = sizes.get(&name).copied().unwrap_or((0, 0));
            let is_link = is_link_entry(&path);
            EntryStat {
                name: name.to_string_lossy().to_string(),
                path,
                is_dir,
                is_link,
                size,
                files,
            }
        })
        .collect();
    entries.sort_by(|a, b| b.size.cmp(&a.size).then_with(|| a.name.cmp(&b.name)));

    Ok(FolderAnalysis {
        root: root.to_path_buf(),
        total_files,
        total_dirs,
        total_size,
        skipped,
        entries,
        largest: largest
            .into_iter()
            .map(|(size, path)| FileStat { path, size })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;

    fn write(dir: &Path, name: &str, size: usize) -> PathBuf {
        let path = dir.join(name);
        let mut file = File::create(&path).expect("create");
        file.write_all(&vec![b'a'; size]).expect("write");
        path
    }

    #[test]
    fn aggregates_top_level_sizes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sub = dir.path().join("media");
        std::fs::create_dir(&sub).expect("mkdir");
        write(dir.path(), "readme.txt", 10);
        write(&sub, "big.bin", 1000);
        write(&sub, "small.bin", 100);

        let analysis = analyze_folder(dir.path()).expect("analyze");
        assert_eq!(analysis.total_files, 3);
        assert_eq!(analysis.total_dirs, 1);
        assert_eq!(analysis.total_size, 1110);

        // "media" (1100) first, then "readme.txt" (10).
        assert_eq!(analysis.entries[0].name, "media");
        assert_eq!(analysis.entries[0].size, 1100);
        assert_eq!(analysis.entries[0].files, 2);
        assert!(analysis.entries[0].is_dir);
        assert_eq!(analysis.entries[1].name, "readme.txt");
        assert_eq!(analysis.entries[1].size, 10);
    }

    #[test]
    fn largest_files_list() {
        let dir = tempfile::tempdir().expect("tempdir");
        write(dir.path(), "a.bin", 5);
        write(dir.path(), "b.bin", 500);
        write(dir.path(), "c.bin", 50);

        let analysis = analyze_folder(dir.path()).expect("analyze");
        assert_eq!(analysis.largest.len(), 3);
        assert_eq!(analysis.largest[0].size, 500);
        assert_eq!(analysis.largest[1].size, 50);
        assert_eq!(analysis.largest[2].size, 5);
    }

    #[test]
    fn empty_folder_is_clean() {
        let dir = tempfile::tempdir().expect("tempdir");
        let analysis = analyze_folder(dir.path()).expect("analyze");
        assert_eq!(analysis.total_files, 0);
        assert_eq!(analysis.total_size, 0);
        assert!(analysis.entries.is_empty());
        assert!(analysis.largest.is_empty());
    }

    // Symlink to a folder: reported as a dir AND as a link, and the
    // walk must not follow it — the target's content stays attributed
    // to the real folder, not double-counted under the link's name.
    #[cfg(unix)]
    #[test]
    fn symlink_to_a_folder_is_labelled_and_not_followed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("real");
        std::fs::create_dir(&target).expect("mkdir");
        write(&target, "inside.bin", 100);
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        let analysis = analyze_folder(dir.path()).expect("analyze");

        let link_entry = analysis
            .entries
            .iter()
            .find(|e| e.name == "link")
            .expect("link entry");
        assert!(link_entry.is_link);
        assert!(link_entry.is_dir, "the target is a folder");
        assert_eq!(link_entry.size, 0);
        assert_eq!(link_entry.files, 0);

        let real = analysis
            .entries
            .iter()
            .find(|e| e.name == "real")
            .expect("real entry");
        assert!(!real.is_link);
        assert_eq!(real.size, 100);
    }

    // Windows junctions (mount points) look like plain directories to
    // std — but the walk never descends into them either, so they must
    // be labelled instead of showing a silent zero too.
    #[cfg(windows)]
    #[test]
    fn junction_dir_is_labelled_and_not_followed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("real");
        std::fs::create_dir(&target).expect("mkdir");
        write(&target, "inside.bin", 100);

        let junction = dir.path().join("junction");
        let out = std::process::Command::new("cmd")
            .args(["/C", "mklink", "/J"])
            .arg(&junction)
            .arg(&target)
            .output()
            .expect("run mklink");
        assert!(
            out.status.success(),
            "mklink /J failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );

        let analysis = analyze_folder(dir.path()).expect("analyze");

        let j = analysis
            .entries
            .iter()
            .find(|e| e.name == "junction")
            .expect("junction entry");
        assert!(j.is_link, "a junction must carry the link label");
        assert!(j.is_dir);
        assert_eq!(j.size, 0);

        let real = analysis
            .entries
            .iter()
            .find(|e| e.name == "real")
            .expect("real entry");
        assert!(!real.is_link);
        assert_eq!(real.size, 100);
    }

    // Only Linux: APFS (macOS) rejects non-UTF-8 names outright
    // (EILSEQ), so the lossy-merge bug cannot exist there; ext4 is
    // byte-transparent and can.
    #[cfg(target_os = "linux")]
    #[test]
    fn non_utf8_names_are_not_merged() {
        use std::ffi::OsStr;
        use std::os::unix::ffi::OsStrExt;

        let dir = tempfile::tempdir().expect("tempdir");
        // Two distinct non-UTF-8 names whose lossy forms are identical:
        // they must stay two report rows, not merge into one bucket.
        std::fs::write(dir.path().join(OsStr::from_bytes(b"\xff\xfe-one")), b"xx").expect("write");
        std::fs::write(dir.path().join(OsStr::from_bytes(b"\xff\xfe-two")), b"yy").expect("write");

        let analysis = analyze_folder(dir.path()).expect("analyze");
        assert_eq!(analysis.entries.len(), 2, "lossy names must not merge");
        assert_eq!(analysis.total_files, 2);
        assert_eq!(analysis.total_size, 4);
    }
}
