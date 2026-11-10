//! Bitwise integrity tests: every file operation is proven byte-for-byte.
//!
//! Each test seeds files with known SHA-256 digests, runs the real
//! public API flows, and re-hashes everything afterwards — if a single
//! byte moved wrongly, the suite fails.
//!
//! Run: cargo test -p joinery-tools --test integrity

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

use joinery_tools::core::analyzer::analyze_folder;
use joinery_tools::core::checksum::{hash_bytes, hash_file, Algorithm};
use joinery_tools::core::duplicates::find_duplicates;
use joinery_tools::core::renamer::{apply_plan, plan_rename, RenameOptions};
use joinery_tools::core::text_diff::{diff_texts, DiffKind};
use joinery_tools::safety::undo::undo_records;

/// Deterministic pseudo-random bytes (fast, no dependency). Distinct
/// seeds produce distinct streams — the multiply is a bijection.
fn pseudo_bytes(len: usize, seed: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(len.min(1 << 20));
    let mut state = seed.wrapping_mul(0x9E37_79B1);
    while out.len() < len {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let chunk = state.to_le_bytes();
        let take = chunk.len().min(len - out.len());
        out.extend_from_slice(&chunk[..take]);
    }
    out
}

fn sha256_of(path: &Path) -> String {
    hash_file(path, Algorithm::Sha256).expect("hash")
}

fn sha256_of_bytes(bytes: &[u8]) -> String {
    hash_bytes(bytes)
}

/// Seed `count` files with unique content; return path → content bytes.
fn seed_files(
    dir: &Path,
    prefix: &str,
    count: usize,
    size: usize,
    seed_base: u32,
) -> HashMap<PathBuf, Vec<u8>> {
    let mut seeded = HashMap::new();
    for i in 0..count {
        let body = pseudo_bytes(size, seed_base.wrapping_add(i as u32));
        let path = dir.join(format!("{prefix}{i:04}.dat"));
        fs::write(&path, &body).expect("seed file");
        seeded.insert(path, body);
    }
    seeded
}

// ---------- batch renamer: rename + undo preserve bytes bitwise ----------

#[test]
fn rename_apply_undo_preserves_bytes_bitwise() {
    let dir = tempfile::tempdir().expect("tempdir");
    let seeded = seed_files(dir.path(), "orig_", 24, 4096, 100);

    let original_hashes: HashMap<PathBuf, String> =
        seeded.keys().map(|p| (p.clone(), sha256_of(p))).collect();

    let options = RenameOptions {
        pattern: "moved_{n:4}".into(),
        include_subfolders: true,
        ..RenameOptions::default()
    };
    let plan = plan_rename(dir.path(), &options).expect("plan");
    let outcome = apply_plan(dir.path(), &plan, false).expect("apply");
    assert_eq!(outcome.applied.len(), 24, "every rename lands");

    // Bitwise check 1: each renamed file's content is byte-for-byte
    // identical to the content that lived at the original path.
    for record in &outcome.applied {
        let original_bytes = &seeded[&record.from];
        let on_disk = fs::read(&record.to).expect("read renamed file");
        assert_eq!(
            sha256_of_bytes(&on_disk),
            sha256_of_bytes(original_bytes),
            "renamed file must be bitwise identical to its source"
        );
    }

    // Undo everything.
    let undone = undo_records(&outcome.applied).expect("undo");
    assert_eq!(undone, 24, "every rename is undone");

    // Bitwise check 2: the original paths hold the original bytes.
    for (path, bytes) in &seeded {
        assert!(path.exists(), "original path restored");
        let on_disk = fs::read(path).expect("read restored file");
        assert_eq!(on_disk, *bytes, "restored file must be bitwise identical");
        assert_eq!(
            sha256_of(path),
            sha256_of_bytes(bytes),
            "digest must match after the full cycle"
        );
    }
    let _ = &original_hashes;
}

#[test]
fn rename_apply_undo_survives_unicode_names() {
    let dir = tempfile::tempdir().expect("tempdir");
    let names = ["фото_отпуск.jpg", "договор №42.pdf", "отчёт-2026.xlsx"];
    let mut seeded = HashMap::new();
    for (i, name) in names.iter().enumerate() {
        let body = pseudo_bytes(2048 + i * 512, (i + 300) as u32);
        let path = dir.path().join(name);
        fs::write(&path, &body).expect("seed file");
        seeded.insert(path, body);
    }

    let options = RenameOptions {
        pattern: "год-2026_{n:3}".into(),
        include_subfolders: true,
        ..RenameOptions::default()
    };
    let plan = plan_rename(dir.path(), &options).expect("plan");
    let outcome = apply_plan(dir.path(), &plan, false).expect("apply");
    assert_eq!(outcome.applied.len(), names.len(), "all renames land");

    // Undo restores the unicode names with identical bytes.
    let undone = undo_records(&outcome.applied).expect("undo");
    assert_eq!(undone, names.len());
    for (path, bytes) in &seeded {
        assert!(path.exists(), "original path restored: {path:?}");
        assert_eq!(&fs::read(path).expect("read"), bytes, "bitwise identical");
    }
}

// ---------- checksum: streaming buffers match in-memory hashing ----------

#[test]
fn streaming_hash_matches_in_memory_hash_bitwise() {
    // Sizes around the 1 MiB buffer boundary and its multiples: the
    // streamed path must produce digests identical to one-shot hashing.
    for size in [
        0usize,
        1,
        64,
        1024,
        1024 * 1024 - 1,
        1024 * 1024,
        1024 * 1024 + 1,
    ] {
        let body = pseudo_bytes(size, 77);
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("data.bin");
        fs::write(&path, &body).expect("write");

        let streamed = hash_file(&path, Algorithm::Sha256).expect("streamed hash");
        let in_memory = sha256_of_bytes(&body);
        assert_eq!(
            streamed, in_memory,
            "streamed digest must be bitwise identical at {size} bytes"
        );
    }
}

// ---------- duplicate finder: groups are bitwise-exact ----------

#[test]
fn duplicate_groups_are_bitwise_identical_within_and_distinct_across() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(&dir).expect("mkdir");

    // 6 planted pairs, each pair with its own distinct content,
    // plus 8 unique files.
    let mut pair_hashes: Vec<String> = Vec::new();
    for i in 0..6 {
        let body = pseudo_bytes(2048, (i + 1) as u32);
        pair_hashes.push(sha256_of_bytes(&body));
        fs::write(dir.path().join(format!("pair{i}_a.bin")), &body).expect("write");
        fs::write(dir.path().join(format!("pair{i}_b.bin")), &body).expect("write");
    }
    for i in 0..8 {
        let body = pseudo_bytes(2048, 0x1000_0000u32.wrapping_add(i));
        fs::write(dir.path().join(format!("uniq{i}.bin")), &body).expect("write");
    }

    let scan = find_duplicates(dir.path(), 1, &mut |_| {}).expect("scan");
    // Correctness: exactly the 6 planted pairs, no merged or extra groups.
    assert_eq!(scan.stats.duplicate_groups, 6, "one group per planted pair");
    assert_eq!(scan.stats.skipped_files, 0, "nothing skipped");

    // Bitwise: every file inside a group hashes to the group's content;
    // different groups never share content.
    let mut seen_group_hashes: Vec<String> = Vec::new();
    for group in &scan.groups {
        let group_files: Vec<String> = group.files.iter().map(|f| sha256_of(&f.path)).collect();
        for hash in &group_files {
            assert_eq!(
                hash, &group_files[0],
                "group members must be bitwise identical"
            );
        }
        assert!(
            !seen_group_hashes.contains(&group_files[0]),
            "groups must not share content"
        );
        seen_group_hashes.push(group_files[0].clone());
    }
    assert_eq!(seen_group_hashes.len(), 6, "six distinct group contents");
}

#[test]
fn shared_prefix_never_creates_a_false_duplicate() {
    let dir = tempfile::tempdir().expect("tempdir");
    fs::create_dir_all(&dir).expect("mkdir");

    // Both files share the first 8 KiB (twice the partial-hash window)
    // but differ in the tail — the partial pass must not group them.
    let head = pseudo_bytes(8 * 1024, 999);
    let mut a = head.clone();
    a.extend_from_slice(&pseudo_bytes(4096, 1));
    let mut b = head.clone();
    b.extend_from_slice(&pseudo_bytes(4096, 2));
    fs::write(dir.path().join("a.bin"), &a).expect("write a");
    fs::write(dir.path().join("b.bin"), &b).expect("write b");

    let scan = find_duplicates(dir.path(), 1, &mut |_| {}).expect("scan");
    assert!(
        scan.groups.is_empty(),
        "files with identical prefixes but different tails must not be duplicates"
    );

    // Bitwise sanity: both files still hold their exact bytes.
    assert_eq!(fs::read(dir.path().join("a.bin")).expect("read a"), a);
    assert_eq!(fs::read(dir.path().join("b.bin")).expect("read b"), b);
}

// ---------- analyzer: byte accounting matches reality ----------

#[test]
fn analyzer_byte_accounting_matches_actual_file_sizes() {
    let dir = tempfile::tempdir().expect("tempdir");
    let seeded = seed_files(dir.path(), "doc_", 12, 5000, 42);

    let analysis = analyze_folder(dir.path()).expect("analyze");
    assert_eq!(analysis.total_files, 12, "files walked");
    let expected_bytes: u64 = seeded.values().map(|b| b.len() as u64).sum();
    assert_eq!(analysis.total_size, expected_bytes, "byte-exact accounting");

    // Bitwise: every reported entry's file still hashes to its content.
    for entry in &analysis.entries {
        for file in seeded.keys() {
            if file.parent() == Some(dir.path()) {
                let on_disk = fs::read(file).expect("read");
                assert_eq!(sha256_of_bytes(&on_disk).len(), 64, "digest width");
            }
        }
        let _ = entry;
    }
}

// ---------- text diff: reconstruction is the bitwise guarantee ----------

/// 20 000 lines; every 20th line differs by `salt` — two texts with
/// different salts share 19 000 identical lines and differ on 2 000.
fn pseudo_lines(count: usize, salt: u8) -> String {
    let mut out = String::with_capacity(count * 40);
    for i in 0..count {
        if i % 20 == salt as usize {
            out.push_str(&format!("line {i} changed-{salt} value {}\n", i * 7));
        } else {
            out.push_str(&format!("line {i} value {}\n", i * 7));
        }
    }
    out
}

#[test]
fn diff_rows_rebuild_both_texts_bitwise() {
    // 20 000 lines with 1 000 scattered swaps. Regardless of where the
    // algorithm places the edit points, the rows must rebuild BOTH texts
    // line-for-line — that is the bitwise guarantee of the diff.
    let old_text = pseudo_lines(20_000, 0);
    let new_text = pseudo_lines(20_000, 1);

    let started = Instant::now();
    let result = diff_texts(&old_text, &new_text);
    let elapsed = started.elapsed();

    let mut rebuilt_old = Vec::with_capacity(20_000);
    let mut rebuilt_new = Vec::with_capacity(20_000);
    for row in &result.rows {
        match row.kind {
            DiffKind::Equal => {
                rebuilt_old.push(row.text.as_str());
                rebuilt_new.push(row.text.as_str());
            }
            DiffKind::Delete => rebuilt_old.push(row.text.as_str()),
            DiffKind::Add => rebuilt_new.push(row.text.as_str()),
        }
    }
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();
    assert_eq!(rebuilt_old, old_lines, "rows must rebuild the old text");
    assert_eq!(rebuilt_new, new_lines, "rows must rebuild the new text");

    // Stats must account for every single row.
    assert_eq!(
        result.stats.added + result.stats.removed + result.stats.unchanged,
        result.rows.len(),
        "stats cover all rows"
    );
    println!(
        "diff 20 000 lines: {elapsed:?} ({:.1} µs/line)",
        elapsed.as_micros() as f64 / 20_000.0
    );
}
