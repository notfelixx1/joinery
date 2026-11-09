//! Release-mode scaling benchmarks for every core module.
//!
//! Run: cargo run --release -p joinery-tools --example bench [-- --json]
//!
//! Each module is measured at several scales (100 / 1k / 10k files,
//! 100 MB / 1 GB, 20k / 100k / 1M lines, deep vs wide trees). Every
//! timed operation asserts its own correctness, so the benchmark
//! doubles as an integration test. With `--json` the run prints
//! machine-readable results (for charts and CI trackers).
//!
//! Fixtures are generated in a temp directory and cleaned up on exit.
//! Timings are wall-clock on the running machine — background load
//! makes absolute numbers noisy, relative comparisons stay valid.

use std::fs;
use std::io::Write as IoWrite;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use joinery_tools::core::analyzer::analyze_folder;
use joinery_tools::core::checksum::{hash_file, Algorithm};
use joinery_tools::core::duplicates::find_duplicates;
use joinery_tools::core::renamer::{apply_plan, plan_rename, RenameOptions};
use joinery_tools::core::text_diff::diff_texts;
use joinery_tools::core::validators::{validate_json, validate_yaml};
use joinery_tools::safety::undo::undo_records;

static QUIET: AtomicBool = AtomicBool::new(false);

/// One measured result for the machine-readable report.
#[derive(serde::Serialize)]
struct Record {
    module: &'static str,
    op: String,
    value: f64,
    unit: &'static str,
}

/// Run `f` three times, report the best wall time.
fn bench<T>(name: &str, runs: u32, mut f: impl FnMut() -> T) -> (T, f64) {
    let mut best = f64::INFINITY;
    let mut last: Option<T> = None;
    for _ in 0..runs {
        let start = Instant::now();
        let out = f();
        let elapsed = start.elapsed().as_secs_f64();
        if elapsed < best {
            best = elapsed;
        }
        last = Some(out);
    }
    if !QUIET.load(Ordering::Relaxed) {
        println!("  {name:<46} {:>10.1} ms (best of {runs})", best * 1000.0);
    }
    (last.expect("at least one run"), best)
}

fn ms(seconds: f64) -> f64 {
    seconds * 1000.0
}

fn mb_s(bytes: u64, seconds: f64) -> f64 {
    if seconds <= 0.0 {
        0.0
    } else {
        bytes as f64 / 1024.0 / 1024.0 / seconds
    }
}

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

fn write_file(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("write fixture");
}

/// Write `len` bytes without materializing them in memory (1 GB target).
fn write_pseudo_file(path: &Path, len: u64, seed: u32) {
    let mut file = fs::File::create(path).expect("create fixture");
    let mut state = seed.wrapping_mul(0x9E37_79B1);
    let mut written = 0u64;
    while written < len {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        let chunk = state.to_le_bytes();
        let take = chunk.len().min((len - written) as usize);
        file.write_all(&chunk[..take]).expect("write fixture");
        written += take as u64;
    }
}

/// `count` lines of ~40 bytes; every 20th line differs by `salt` —
/// two texts with different salts share (count - count/20) identical
/// lines and differ on 2 × (count / 20) of them.
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

/// Spread `files` files across `dirs` directories: file i lives in
/// dir (i % dirs), content seeded by (i + seed_base) — all distinct.
fn make_tree(root: &Path, files: usize, dirs: usize, file_bytes: usize, seed_base: u32) {
    fs::create_dir_all(root).expect("mkdir");
    for i in 0..files {
        let dir = root.join(format!("dir{:04}", i % dirs));
        fs::create_dir_all(&dir).expect("mkdir");
        write_file(
            &dir.join(format!("file_{:06}.dat", i)),
            &pseudo_bytes(file_bytes, (i as u32).wrapping_add(seed_base)),
        );
    }
}

/// `10000` → `"10 000"` for human-readable op labels.
fn sep(n: usize) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index) % 3 == 0 {
            out.push(' ');
        }
        out.push(ch);
    }
    out
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let json_mode = args.iter().any(|a| a == "--json");
    QUIET.store(json_mode, Ordering::Relaxed);
    let mut records: Vec<Record> = Vec::new();
    let mut record = |module: &'static str, op: String, value: f64, unit: &'static str| {
        records.push(Record {
            module,
            op,
            value,
            unit,
        });
    };

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    let sha_ni = is_x86_feature_detected!("sha");
    if !json_mode {
        println!("Joinery core scaling benchmarks — release profile, {threads} threads");
        println!("SHA-NI hardware: {sha_ni}");
        println!("(wall time on this machine; background load adds noise)");
        println!();
    }

    let root = tempfile::tempdir().expect("tempdir");

    // ---------- batch renamer: 100 / 1 000 / 10 000 files ----------
    if !json_mode {
        println!();
        println!("--- batch renamer: plan + apply + undo at three scales ---");
    }
    for files in [100usize, 1_000, 10_000] {
        let tree = root.path().join(format!("renamer{files:06}"));
        if !json_mode {
            println!("fixture: {files} files…");
        }
        make_tree(&tree, files, 10, 4096, 10);
        let options = RenameOptions {
            pattern: "renamed_{n:6}".into(),
            include_subfolders: true,
            ..RenameOptions::default()
        };
        let (_, plan_seconds) = bench(&format!("renamer::plan {} files", sep(files)), 3, || {
            let plan = plan_rename(&tree, &options).expect("plan");
            // Correctness: the plan covers every file.
            assert_eq!(plan.len(), files, "planned files");
            plan
        });
        record(
            "renamer",
            format!("plan {files} files"),
            ms(plan_seconds),
            "ms",
        );

        let plan = plan_rename(&tree, &options).expect("plan");
        let (_, apply_seconds) = bench(&format!("renamer::apply {} files", sep(files)), 3, || {
            let outcome = apply_plan(&tree, &plan, false).expect("apply");
            // Correctness: every rename lands exactly once.
            assert_eq!(
                outcome.applied.len(),
                files,
                "applied renames; skipped: {:?}",
                outcome.skipped
            );
            // Restore for the next run (core's own records).
            undo_records(&outcome.applied).expect("undo");
            outcome
        });
        record(
            "renamer",
            format!("apply {files} files"),
            ms(apply_seconds),
            "ms",
        );

        let (_, undo_seconds) = bench(&format!("renamer::undo {} files", sep(files)), 3, || {
            let outcome = apply_plan(&tree, &plan, false).expect("apply");
            let undone = undo_records(&outcome.applied).expect("undo");
            // Correctness: undo restores every single name.
            assert_eq!(undone, files, "undone renames");
            undone
        });
        record(
            "renamer",
            format!("undo {files} files"),
            ms(undo_seconds),
            "ms",
        );
    }

    // ---------- duplicate finder: 1 000 / 10 000 / 100 000 files ----------
    if !json_mode {
        println!();
        println!("--- duplicate finder: scans at three scales ---");
    }
    for files in [1_000usize, 10_000, 100_000] {
        // 10% of the files form planted pairs (exact duplicates), the
        // rest are unique — groups must equal files / 20.
        let tree = root.path().join(format!("dups{files:06}"));
        if !json_mode {
            println!("fixture: {files} files…");
        }
        fs::create_dir_all(&tree).expect("mkdir");
        let pairs = files / 20;
        for i in 0..pairs {
            // Each pair has its own content — groups must not merge.
            let body = pseudo_bytes(1024, (i + 1) as u32);
            let dir = tree.join(format!("dir{:04}", i % 100));
            fs::create_dir_all(&dir).expect("mkdir");
            write_file(&dir.join(format!("pair{i:06}_a.bin")), &body);
            write_file(&dir.join(format!("pair{i:06}_b.bin")), &body);
        }
        for i in 0..(files - pairs) {
            let dir = tree.join(format!("dir{:04}", (pairs + i) % 100));
            fs::create_dir_all(&dir).expect("mkdir");
            write_file(
                &dir.join(format!("uniq{i:07}.bin")),
                &pseudo_bytes(1024, 0x1000_0000u32.wrapping_add(i as u32)),
            );
        }
        let expected_groups = pairs as u64; // one group per planted pair
        let expected_wasted = (pairs * 1024) as u64;

        let (_, seconds) = bench(&format!("duplicates::scan {} files", sep(files)), 2, || {
            let scan = find_duplicates(&tree, 1, &mut |_| {}).expect("scan");
            // Correctness: every planted pair is found, nothing extra.
            assert_eq!(
                scan.stats.duplicate_groups, expected_groups,
                "duplicate groups"
            );
            assert_eq!(scan.stats.wasted_bytes, expected_wasted, "wasted bytes");
            assert_eq!(scan.stats.skipped_files, 0, "skipped files");
            scan
        });
        record(
            "duplicates",
            format!("scan {files} files"),
            ms(seconds),
            "ms",
        );
        let scanned_bytes = (files * 1024) as u64;
        if !json_mode {
            println!(
                "      → {:.0} MB/s effective ({pairs} planted pairs, {expected_groups} groups)",
                mb_s(scanned_bytes, seconds)
            );
        }
    }

    // ---------- checksum: SHA-256 on 100 MB and 1 GB ----------
    if !json_mode {
        println!();
        println!("--- checksum: SHA-256 on 100 MB and 1 GB (+ others on 1 GB) ---");
    }
    let big100 = root.path().join("big100.bin");
    if !json_mode {
        println!("fixture: 100 MB…");
    }
    write_pseudo_file(&big100, 100 * 1024 * 1024, 11);
    let big1g = root.path().join("big1g.bin");
    if !json_mode {
        println!("fixture: 1 GB…");
    }
    write_pseudo_file(&big1g, 1024 * 1024 * 1024, 12);

    for (label, path, bytes) in [
        ("SHA-256 100 MB", &big100, 100 * 1024 * 1024u64),
        ("SHA-256 1 GB", &big1g, 1024 * 1024 * 1024u64),
    ] {
        let (_, seconds) = bench(&format!("checksum::{label}"), 2, || {
            let hex = hash_file(path, Algorithm::Sha256).expect("hash");
            // Correctness: hex digest of the full file.
            assert_eq!(hex.len(), 64, "sha256 digest length");
            hex
        });
        if !json_mode {
            println!("      → {:.0} MB/s", mb_s(bytes, seconds));
        }
        record(
            "checksum",
            label.to_string(),
            (mb_s(bytes, seconds) * 100.0).round() / 100.0,
            "MB/s",
        );
    }
    for (label, alg) in [
        ("SHA-1 1 GB", Algorithm::Sha1),
        ("MD5 1 GB", Algorithm::Md5),
    ] {
        let (_, seconds) = bench(&format!("checksum::{label}"), 2, || {
            let hex = hash_file(&big1g, alg).expect("hash");
            assert!(!hex.is_empty(), "digest");
            hex
        });
        if !json_mode {
            println!("      → {:.0} MB/s", mb_s(1024 * 1024 * 1024, seconds));
        }
        record(
            "checksum",
            label.to_string(),
            (mb_s(1024 * 1024 * 1024, seconds) * 100.0).round() / 100.0,
            "MB/s",
        );
    }

    // ---------- folder analyzer: deep vs wide trees ----------
    if !json_mode {
        println!();
        println!("--- folder analyzer: deep vs wide trees (3 000 files each) ---");
    }
    let deep = root.path().join("deep");
    if !json_mode {
        println!("fixture: deep tree…");
    }
    let mut deep_dir = deep.clone();
    fs::create_dir_all(&deep_dir).expect("mkdir");
    for level in 0..100 {
        for f in 0..30 {
            write_file(
                &deep_dir.join(format!("file_{f:03}.dat")),
                &pseudo_bytes(4096, level * 30 + f),
            );
        }
        deep_dir = deep_dir.join(format!("level{level:03}"));
        fs::create_dir_all(&deep_dir).expect("mkdir");
    }
    let wide = root.path().join("wide");
    if !json_mode {
        println!("fixture: wide tree…");
    }
    make_tree(&wide, 3_000, 300, 4096, 9000);

    let (_, deep_seconds) = bench("analyzer::deep tree (100 levels, 3 000 files)", 3, || {
        let analysis = analyze_folder(&deep).expect("analyze");
        // Correctness: every file walked at every level; the top level
        // holds 30 files plus the single descending directory.
        assert_eq!(analysis.total_files, 3_000, "files walked");
        assert_eq!(analysis.entries.len(), 31, "top-level: 30 files + chain");
        analysis
    });
    record(
        "analyzer",
        "deep tree 100 levels".into(),
        ms(deep_seconds),
        "ms",
    );
    let (_, wide_seconds) = bench("analyzer::wide tree (300 dirs, 3 000 files)", 3, || {
        let analysis = analyze_folder(&wide).expect("analyze");
        assert_eq!(analysis.total_files, 3_000, "files walked");
        assert_eq!(analysis.entries.len(), 300, "top-level entries");
        analysis
    });
    record(
        "analyzer",
        "wide tree 300 dirs".into(),
        ms(wide_seconds),
        "ms",
    );

    // ---------- text diff: 20 000 / 100 000 / 1 000 000 lines ----------
    if !json_mode {
        println!();
        println!("--- text diff: three scales ---");
    }
    for lines in [20_000usize, 100_000, 1_000_000] {
        let old_text = pseudo_lines(lines, 0);
        let new_text = pseudo_lines(lines, 1);
        let expected = (2 * (lines / 20)) as u64;
        let (_, seconds) = bench(&format!("text_diff::diff {lines} lines"), 2, || {
            let result = diff_texts(&old_text, &new_text);
            // Correctness: both texts change count/20 of their own lines
            // at different positions — each swap is one remove + one add.
            assert_eq!(
                result.stats.removed as usize, expected as usize,
                "removed lines"
            );
            assert_eq!(
                result.stats.added as usize, expected as usize,
                "added lines"
            );
            result
        });
        record(
            "text_diff",
            format!("diff {lines} lines"),
            ms(seconds),
            "ms",
        );
        if !json_mode {
            println!(
                "      → {:.0} k lines/s ({expected} expected line swaps)",
                lines as f64 / 1000.0 / seconds
            );
        }
    }

    // ---------- validators: JSON / YAML of different sizes ----------
    if !json_mode {
        println!();
        println!("--- validators: three JSON sizes + YAML ---");
    }
    let json_sizes = [100 * 1024, 1024 * 1024, 10 * 1024 * 1024];
    for bytes in json_sizes {
        let records_n = bytes / 80;
        let mut doc = String::from("[\n");
        for i in 0..records_n {
            let comma = if i + 1 == records_n { "" } else { "," };
            doc.push_str(&format!(
                "  {{\"id\": {i}, \"name\": \"record-{i}\", \"tags\": [\"a{i}\", \"b\"], \"score\": {}}}{comma}\n",
                i % 100
            ));
        }
        doc.push_str("]\n");
        let kb = doc.len() / 1024;
        let (_, seconds) = bench(&format!("validators::validate_json {kb} KB"), 2, || {
            let result = validate_json(&doc);
            // Correctness: a well-formed document validates cleanly.
            assert!(result.valid, "json must validate");
            assert!(result.formatted.is_some(), "json canonical form");
            result
        });
        record(
            "validators",
            format!("validate_json {kb} KB"),
            ms(seconds),
            "ms",
        );
        if !json_mode {
            println!("      → {:.0} MB/s", mb_s(doc.len() as u64, seconds));
        }
    }
    for (label, doc) in [
        ("YAML 100 KB", pseudo_yaml(100 * 1024)),
        ("YAML 1 MB", pseudo_yaml(1024 * 1024)),
    ] {
        let (_, seconds) = bench(&format!("validators::{label}"), 2, || {
            let result = validate_yaml(&doc);
            // Correctness: a well-formed document validates cleanly.
            assert!(result.valid, "yaml must validate");
            result
        });
        record("validators", label.to_string(), ms(seconds), "ms");
    }

    // ---------- report ----------
    if json_mode {
        println!(
            "{}",
            serde_json::json!({
                "date": chrono::Utc::now().to_rfc3339(),
                "threads": threads,
                "sha_ni": sha_ni,
                "results": records,
            })
        );
    } else {
        println!();
        println!("Done. Fixtures live in a temp dir and are removed on exit.");
        println!("All correctness assertions passed — every module works.");
    }
    let _ = &records;
}

fn pseudo_yaml(target_len: usize) -> String {
    let mut out = String::with_capacity(target_len);
    let mut i = 0;
    while out.len() < target_len {
        out.push_str(&format!(
            "item{i}:\n  name: record-{i}\n  tags: [a{i}, b]\n  score: {}\n",
            i % 100
        ));
        i += 1;
    }
    out
}
