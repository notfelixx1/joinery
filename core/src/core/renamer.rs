//! Batch Renamer core.
//!
//! The renamer is *preview-first*: [`plan_rename`] only computes what
//! would change, [`apply_plan`] performs the file system operations and
//! returns an undo journal. Applying never silently overwrites existing
//! files.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use walkdir::WalkDir;

use crate::safety::preview::{check_new_filename, detect_batch_collisions, is_within};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RenameStatus {
    /// Safe to rename.
    Ready,
    /// New name equals the current one — nothing to do.
    NoChange,
    /// A file with the target name already exists on disk.
    TargetExists,
    /// Another operation in the same batch produces this target name.
    BatchDuplicate,
    /// Target name is invalid on at least one supported platform.
    InvalidName,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenamePlanItem {
    pub from: PathBuf,
    pub to: PathBuf,
    pub from_name: String,
    pub to_name: String,
    pub status: RenameStatus,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct RenameOptions {
    /// Plain-text substring replaced by `replace` in the stem.
    pub find: String,
    pub replace: String,
    /// Inserted before the stem.
    pub prefix: String,
    /// Inserted after the stem (before the extension).
    pub suffix: String,
    /// When non-empty, replaces the extension (without the leading dot).
    pub new_extension: String,
    /// Optional template. Tokens: `{name}`, `{ext}`, `{n}`, `{n:3}`.
    /// When non-empty it replaces the whole stem; the extension is kept
    /// (or replaced by `new_extension`).
    pub pattern: String,
    pub include_subfolders: bool,
}

/// One performed rename, recorded for undo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameRecord {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SkippedRename {
    pub from: PathBuf,
    pub to: PathBuf,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyOutcome {
    pub applied: Vec<RenameRecord>,
    pub skipped: Vec<SkippedRename>,
}

/// Collect the files a rename plan will cover, deterministically ordered.
fn collect_files(root: &Path, include_subfolders: bool) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    if include_subfolders {
        for entry in WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_type().is_file() {
                files.push(entry.into_path());
            }
        }
    } else {
        for entry in fs::read_dir(root)? {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => continue,
            };
            let path = entry.path();
            if path.is_file() {
                files.push(path);
            }
        }
    }
    files.sort();
    Ok(files)
}

/// Render the optional pattern template.
fn render_pattern(pattern: &str, stem: &str, ext: &str, index: usize) -> String {
    let mut out = String::with_capacity(pattern.len() + 8);
    let bytes = pattern.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'{' {
            if let Some(close) = pattern[i + 1..].find('}') {
                let token = &pattern[i + 1..i + 1 + close];
                match token {
                    "name" => out.push_str(stem),
                    "ext" => out.push_str(ext),
                    "n" => out.push_str(&(index + 1).to_string()),
                    t if t.starts_with("n:") => {
                        if let Ok(width) = t[2..].parse::<usize>() {
                            out.push_str(&format!("{:0width$}", index + 1, width = width));
                        } else {
                            out.push_str(&(index + 1).to_string());
                        }
                    }
                    literal => {
                        out.push('{');
                        out.push_str(literal);
                        out.push('}');
                    }
                }
                i += close + 2;
                continue;
            }
        }
        // Advance one UTF-8 character.
        let ch_len = utf8_char_len(bytes[i]);
        out.push_str(&pattern[i..i + ch_len]);
        i += ch_len;
    }
    out
}

fn utf8_char_len(first_byte: u8) -> usize {
    match first_byte {
        b if b < 0x80 => 1,
        b if b >> 5 == 0b110 => 2,
        b if b >> 4 == 0b1110 => 3,
        _ => 4,
    }
}

/// Compute the new file name (with extension) for one file.
fn build_new_name(stem: &str, ext: &str, index: usize, options: &RenameOptions) -> String {
    let new_stem = if !options.pattern.is_empty() {
        render_pattern(&options.pattern, stem, ext, index)
    } else {
        let mut s = stem.to_string();
        if !options.find.is_empty() {
            s = s.replace(&options.find, &options.replace);
        }
        format!("{}{}{}", options.prefix, s, options.suffix)
    };

    let new_ext = if options.new_extension.is_empty() {
        ext
    } else {
        options.new_extension.as_str()
    };

    if new_ext.is_empty() {
        new_stem
    } else {
        format!("{new_stem}.{new_ext}")
    }
}

/// Build the preview plan for a folder. Performs no file system changes.
pub fn plan_rename(root: &Path, options: &RenameOptions) -> io::Result<Vec<RenamePlanItem>> {
    let files = collect_files(root, options.include_subfolders)?;
    let mut items = Vec::with_capacity(files.len());

    for (index, path) in files.iter().enumerate() {
        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let stem = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_string())
            .unwrap_or_default();

        let new_name = build_new_name(&stem, &ext, index, options);
        let (to, status, reason) = if new_name == file_name {
            (path.clone(), RenameStatus::NoChange, None)
        } else if let Err(problem) = check_new_filename(&new_name) {
            (path.with_file_name(&new_name), RenameStatus::InvalidName, Some(problem))
        } else {
            let target = path.with_file_name(&new_name);
            if target.exists() {
                (
                    target,
                    RenameStatus::TargetExists,
                    Some("a file with this name already exists".to_string()),
                )
            } else {
                (target, RenameStatus::Ready, None)
            }
        };

        items.push(RenamePlanItem {
            from: path.clone(),
            to,
            from_name: file_name,
            to_name: new_name,
            status,
            reason,
        });
    }

    detect_batch_collisions(&mut items);
    Ok(items)
}

/// Execute a previewed plan.
///
/// Safety rules:
/// - only items with status [`RenameStatus::Ready`] are executed;
/// - every path must resolve inside `root` (canonical check);
/// - existing targets are never overwritten unless `allow_overwrite`.
pub fn apply_plan(
    root: &Path,
    items: &[RenamePlanItem],
    allow_overwrite: bool,
) -> io::Result<ApplyOutcome> {
    // Fails when the root vanished or is unreadable — a hard stop.
    fs::canonicalize(root)?;
    let mut outcome = ApplyOutcome {
        applied: Vec::new(),
        skipped: Vec::new(),
    };

    for item in items.iter().filter(|i| i.status == RenameStatus::Ready) {
        let skip = |reason: &str| SkippedRename {
            from: item.from.clone(),
            to: item.to.clone(),
            reason: reason.to_string(),
        };

        if !is_within(&item.from, root) || !is_within(&item.to, root) {
            outcome.skipped.push(skip("path escapes the selected folder"));
            continue;
        }
        if !item.from.exists() {
            outcome.skipped.push(skip("source file no longer exists"));
            continue;
        }
        if item.to.exists() {
            if allow_overwrite {
                // Explicit, user-visible decision — still refuse to
                // overwrite with an identical path.
                if item.from == item.to {
                    outcome.skipped.push(skip("source and target are the same file"));
                    continue;
                }
            } else {
                outcome.skipped.push(skip("target already exists"));
                continue;
            }
        }

        match fs::rename(&item.from, &item.to) {
            Ok(()) => outcome.applied.push(RenameRecord {
                from: item.from.clone(),
                to: item.to.clone(),
            }),
            Err(err) => outcome.skipped.push(skip(&format!("rename failed: {err}"))),
        }
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;

    fn touch(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        File::create(&path).expect("create file");
        path
    }

    fn opts() -> RenameOptions {
        serde_json::from_str("{}").expect("default options")
    }

    #[test]
    fn find_replace_and_prefix_suffix() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "photo_001.jpg");
        touch(dir.path(), "photo_002.jpg");

        let options = RenameOptions {
            find: "photo".into(),
            replace: "vacation".into(),
            prefix: "2024_".into(),
            suffix: "_final".into(),
            ..opts()
        };
        let plan = plan_rename(dir.path(), &options).expect("plan");
        assert_eq!(plan.len(), 2);
        assert!(plan
            .iter()
            .all(|item| item.status == RenameStatus::Ready
                && item.to_name.starts_with("2024_vacation")
                && item.to_name.ends_with("_final.jpg")));
    }

    #[test]
    fn pattern_with_numbering() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "a.txt");
        touch(dir.path(), "b.txt");

        let options = RenameOptions {
            pattern: "file_{n:3}".into(),
            ..opts()
        };
        let plan = plan_rename(dir.path(), &options).expect("plan");
        let names: Vec<&str> = plan.iter().map(|i| i.to_name.as_str()).collect();
        assert_eq!(names, vec!["file_001.txt", "file_002.txt"]);
    }

    #[test]
    fn extension_change() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "report.docx");
        let options = RenameOptions {
            new_extension: "pdf".into(),
            ..opts()
        };
        let plan = plan_rename(dir.path(), &options).expect("plan");
        assert_eq!(plan[0].to_name, "report.pdf");
    }

    #[test]
    fn no_change_and_target_exists_statuses() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "same.txt");
        touch(dir.path(), "other.txt");

        // Renaming other.txt onto same.txt must be flagged, not applied.
        let options = RenameOptions {
            find: "other".into(),
            replace: "same".into(),
            ..opts()
        };
        let plan = plan_rename(dir.path(), &options).expect("plan");
        let statuses: Vec<RenameStatus> = plan.iter().map(|i| i.status).collect();
        assert!(statuses.contains(&RenameStatus::NoChange));
        assert!(statuses.contains(&RenameStatus::TargetExists));
    }

    #[test]
    fn batch_duplicates_are_detected() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "a1.txt");
        touch(dir.path(), "a2.txt");

        // Both files map to "b.txt" (which does not exist on disk):
        // the first stays Ready, the second must be flagged.
        let options = RenameOptions {
            pattern: "b".into(),
            ..opts()
        };
        let plan = plan_rename(dir.path(), &options).expect("plan");
        let statuses: Vec<RenameStatus> = plan.iter().map(|i| i.status).collect();
        assert_eq!(
            statuses.iter().filter(|s| **s == RenameStatus::Ready).count(),
            1
        );
        assert_eq!(
            statuses
                .iter()
                .filter(|s| **s == RenameStatus::BatchDuplicate)
                .count(),
            1
        );
    }

    #[test]
    fn apply_and_undo_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        touch(dir.path(), "photo_001.jpg");
        touch(dir.path(), "photo_002.jpg");

        let options = RenameOptions {
            find: "photo".into(),
            replace: "vacation".into(),
            ..opts()
        };
        let plan = plan_rename(dir.path(), &options).expect("plan");
        let outcome = apply_plan(dir.path(), &plan, false).expect("apply");
        assert_eq!(outcome.applied.len(), 2);
        assert!(outcome.skipped.is_empty());
        assert!(dir.path().join("vacation_001.jpg").exists());
        assert!(!dir.path().join("photo_001.jpg").exists());

        // Undo in reverse order.
        let undone = crate::safety::undo::undo_records(&outcome.applied).expect("undo");
        assert_eq!(undone, 2);
        assert!(dir.path().join("photo_001.jpg").exists());
        assert!(!dir.path().join("vacation_001.jpg").exists());
    }

    #[test]
    fn apply_never_escapes_root() {
        let dir = tempfile::tempdir().expect("tempdir");
        let outside = tempfile::tempdir().expect("tempdir");
        let victim = touch(outside.path(), "victim.txt");

        // Forge a plan item pointing outside the root.
        let forged = RenamePlanItem {
            from: victim.clone(),
            to: dir.path().join("moved.txt"),
            from_name: "victim.txt".into(),
            to_name: "moved.txt".into(),
            status: RenameStatus::Ready,
            reason: None,
        };
        let outcome = apply_plan(dir.path(), &[forged], false).expect("apply");
        assert_eq!(outcome.applied.len(), 0);
        assert_eq!(outcome.skipped.len(), 1);
        assert!(victim.exists(), "outside file must stay untouched");
    }

    #[test]
    fn subfolder_toggle() {
        let dir = tempfile::tempdir().expect("tempdir");
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).expect("mkdir");
        touch(&sub, "nested.txt");

        let flat = plan_rename(dir.path(), &opts()).expect("plan");
        assert_eq!(flat.len(), 0);

        let recursive = plan_rename(dir.path(), &RenameOptions {
            include_subfolders: true,
            ..opts()
        })
        .expect("plan");
        assert_eq!(recursive.len(), 1);
    }
}
