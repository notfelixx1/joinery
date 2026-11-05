//! Path-safety checks shared by every tool that touches files.
//!
//! Joinery's promise: the UI never performs a dangerous operation — the
//! Rust core validates every request before touching the file system.

use std::path::{Component, Path, PathBuf};

use crate::core::renamer::RenamePlanItem;
use crate::core::renamer::RenameStatus;

/// Windows-reserved device names (case-insensitive, any extension).
const WINDOWS_RESERVED: [&str; 22] = [
    "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6", "COM7", "COM8",
    "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

/// Characters invalid in Windows file names; they are best avoided on
/// every platform so files stay portable.
const INVALID_CHARS: [char; 9] = ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

/// Make a path absolute without touching the file system.
fn lexical_absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        return normalize_lexically(path);
    }
    let base = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
    normalize_lexically(&base.join(path))
}

/// Resolve `.` and `..` components lexically.
fn normalize_lexically(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for component in path.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolve symlinks on the existing prefix of `path` and keep the rest
/// lexical. Plain `dunce::canonicalize` fails for not-yet-existing
/// targets, and the lexical fallback lives in a different name space
/// than a canonicalized `root` (macOS: `/tmp` is a symlink to
/// `/private/tmp`), which made every missing target look like it
/// escapes the folder.
fn canonicalize_best_effort(path: &Path) -> PathBuf {
    if let Ok(c) = dunce::canonicalize(path) {
        return c;
    }
    let mut missing: Vec<std::ffi::OsString> = Vec::new();
    let mut cur = path.to_path_buf();
    while let Some(parent) = cur.parent() {
        if let Some(name) = cur.file_name() {
            missing.push(name.to_os_string());
        }
        if let Ok(c) = dunce::canonicalize(parent) {
            let mut out = c;
            for part in missing.iter().rev() {
                out.push(part);
            }
            return out;
        }
        cur = parent.to_path_buf();
    }
    lexical_absolute(path)
}

/// True when `path` is (or resolves to) something inside `root`.
///
/// Both sides are resolved the same way: canonicalized as far as the
/// file system allows, lexically for the missing tail (`dunce` keeps
/// plain `C:\...` form on Windows — `fs::canonicalize` would return
/// `\\?\C:\...` and never match the lexical fallback). On Windows the
/// comparison is case-insensitive, matching the file system.
pub fn is_within(path: &Path, root: &Path) -> bool {
    let resolved = canonicalize_best_effort(path);
    let resolved_root = canonicalize_best_effort(root);
    starts_with_componentwise(&resolved, &resolved_root)
}

fn starts_with_componentwise(path: &Path, root: &Path) -> bool {
    if path.components().count() < root.components().count() {
        return false;
    }
    let mut path_components = path.components();
    for root_component in root.components() {
        let path_component = path_components.next().unwrap_or(Component::CurDir);
        let matches = if cfg!(windows) {
            path_component
                .as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case(root_component.as_os_str().to_string_lossy().as_ref())
        } else {
            path_component.as_os_str() == root_component.as_os_str()
        };
        if !matches {
            return false;
        }
    }
    true
}

/// Validate a prospective file name (name + extension, no directories).
pub fn check_new_filename(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("file name must not be empty".to_string());
    }
    if name == "." || name == ".." {
        return Err("file name must not be a relative path marker".to_string());
    }
    if name.len() > 255 {
        return Err("file name is longer than 255 bytes".to_string());
    }
    if let Some(bad) = name.chars().find(|c| INVALID_CHARS.contains(c)) {
        return Err(format!("character '{bad}' is not allowed in file names"));
    }
    if name.chars().any(|c| (c as u32) < 32) {
        return Err("control characters are not allowed in file names".to_string());
    }
    if name.ends_with(['.', ' ']) {
        return Err("file name must not end with a dot or a space".to_string());
    }
    let stem = name.split('.').next().unwrap_or_default();
    if WINDOWS_RESERVED
        .iter()
        .any(|reserved| stem.eq_ignore_ascii_case(reserved))
    {
        return Err(format!("'{stem}' is a reserved device name on Windows"));
    }
    Ok(())
}

/// Flag operations whose targets collide with each other inside the
/// same batch — the plan as a whole must stay conflict-free before the
/// "Apply" button lights up.
pub fn detect_batch_collisions(items: &mut [RenamePlanItem]) {
    use std::collections::HashSet;

    let mut seen: HashSet<String> = HashSet::new();
    for item in items.iter_mut() {
        if item.status != RenameStatus::Ready {
            continue;
        }
        let key = item.to.to_string_lossy().to_lowercase();
        if !seen.insert(key) {
            item.status = RenameStatus::BatchDuplicate;
            item.reason =
                Some("another operation in this batch produces the same name".to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filename_validation() {
        assert!(check_new_filename("photo.jpg").is_ok());
        assert!(check_new_filename("мир-и-дружба.txt").is_ok());
        assert!(check_new_filename("").is_err());
        assert!(check_new_filename(".").is_err());
        assert!(check_new_filename("bad/name.txt").is_err());
        assert!(check_new_filename("bad:name.txt").is_err());
        assert!(check_new_filename("CON").is_err());
        assert!(check_new_filename("com1.log").is_err());
        assert!(check_new_filename("trailing.").is_err());
        assert!(check_new_filename("trailing ").is_err());
    }

    #[test]
    fn is_within_rejects_escapes() {
        let root = tempfile::tempdir().expect("tempdir");
        let inside = root.path().join("a.txt");
        std::fs::File::create(&inside).expect("create");

        assert!(is_within(&inside, root.path()));

        let other = tempfile::tempdir().expect("tempdir");
        assert!(!is_within(other.path(), root.path()));

        // Non-existent target inside root is still "within" lexically.
        let ghost = root.path().join("ghost.txt");
        assert!(is_within(&ghost, root.path()));
        // Non-existent target outside root is not.
        let ghost_out = other.path().join("ghost.txt");
        assert!(!is_within(&ghost_out, root.path()));
        // Several missing levels still land in the root's name space.
        let deep_ghost = root.path().join("no").join("such").join("g.txt");
        assert!(is_within(&deep_ghost, root.path()));
    }

    #[test]
    fn lexical_normalization_handles_parent_dirs() {
        let root = tempfile::tempdir().expect("tempdir");
        let sneaky = root.path().join("sub").join("..").join("x.txt");
        let resolved = lexical_absolute(&sneaky);
        assert_eq!(resolved, lexical_absolute(&root.path().join("x.txt")));
    }
}
