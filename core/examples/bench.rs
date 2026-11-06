//! Release-mode benchmarks for every core module.
//!
//! Run: cargo run --release -p joinery-tools --example bench
//!
//! Fixtures are generated in a temp directory and cleaned up on exit.
//! Timings are wall-clock on the running machine — background load
//! makes absolute numbers noisy, relative comparisons stay valid.

use std::fs;
use std::path::Path;
use std::time::Instant;

use joinery_tools::core::analyzer::analyze_folder;
use joinery_tools::core::checksum::{hash_file, Algorithm};
use joinery_tools::core::duplicates::find_duplicates;
use joinery_tools::core::renamer::{apply_plan, plan_rename, RenameOptions, RenameRecord};
use joinery_tools::core::text_diff::diff_texts;
use joinery_tools::core::validators::{validate_json, validate_yaml};
use joinery_tools::safety::undo::undo_records;

const BIG_FILE_BYTES: u64 = 96 * 1024 * 1024;
const SMALL_FILES: usize = 400;
const DUP_GROUPS: usize = 8;
const DUP_COPIES: usize = 3;
const DUP_FILE_BYTES: usize = 2 * 1024 * 1024;
const UNIQUE_FILES: usize = 12;

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
    println!("  {name:<46} {best:>8.3} s (best of {runs})");
    (last.expect("at least one run"), best)
}

fn mb_s(bytes: u64, seconds: f64) -> f64 {
    if seconds <= 0.0 {
        0.0
    } else {
        bytes as f64 / 1024.0 / 1024.0 / seconds
    }
}

/// Deterministic pseudo-random bytes (fast, no dependency).
fn pseudo_bytes(len: usize, seed: u32) -> Vec<u8> {
    let mut out = Vec::with_capacity(len);
    let mut state = seed.wrapping_mul(0x9E37_79B1) | 1;
    for _ in 0..len {
        state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        out.push((state >> 24) as u8);
    }
    out
}

fn write_file(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).expect("write fixture");
}

fn main() {
    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1);
    println!("Joinery core benchmarks — release profile, {threads} threads");
    println!("SHA-NI hardware: {}", is_x86_feature_detected!("sha"));
    println!("(wall time on this machine; background load adds noise)");
    println!();

    let root = tempfile::tempdir().expect("tempdir");

    println!("Generating fixtures (~160 MB)…");
    let big = root.path().join("big.bin");
    write_file(&big, &pseudo_bytes(BIG_FILE_BYTES as usize, 7));

    // Small tree: 400 files × ~4 KiB across nested folders.
    let small = root.path().join("small");
    for i in 0..SMALL_FILES {
        let sub = small
            .join(format!("dir{}", i % 4))
            .join(format!("lvl{}", i % 3));
        fs::create_dir_all(&sub).expect("mkdir");
        write_file(
            &sub.join(format!("file_{:04}.dat", i)),
            &pseudo_bytes(4096, i as u32),
        );
    }

    // Duplicate tree: real groups (identical copies) + unique files.
    let dups = root.path().join("dups");
    fs::create_dir_all(&dups).expect("mkdir");
    for g in 0..DUP_GROUPS {
        let body = pseudo_bytes(DUP_FILE_BYTES, (g + 1) as u32);
        for c in 0..DUP_COPIES {
            write_file(&dups.join(format!("group{g}_copy{c}.bin")), &body);
        }
    }
    for i in 0..UNIQUE_FILES {
        write_file(
            &dups.join(format!("unique_{i:02}.bin")),
            &pseudo_bytes(1024 * 1024, (i as u32) + 500),
        );
    }
    let expected_wasted = (DUP_GROUPS * DUP_FILE_BYTES * (DUP_COPIES - 1)) as u64;

    // Large JSON document for the validators.
    let mut json_doc = String::from("[\n");
    for i in 0..12_000 {
        json_doc.push_str(&format!(
            "  {{\"id\": {i}, \"name\": \"record-{i}\", \"tags\": [\"a{i}\", \"b\"], \"score\": {}}},\n",
            i % 100
        ));
    }
    json_doc.push_str("]\n");
    let yaml_doc = "name: joinery\nitems:\n  - one\n  - two\n".to_string();

    println!();
    println!("--- checksum (single 96 MB file, warm cache) ---");
    for alg in [Algorithm::Sha256, Algorithm::Sha1, Algorithm::Md5] {
        let name = format!("checksum::{alg:?} (96 MB)");
        let (_, seconds) = bench(&name, 3, || {
            hash_file(&big, alg).expect("hash");
        });
        println!("      → {:.0} MB/s", mb_s(BIG_FILE_BYTES, seconds));
    }

    println!();
    println!("--- batch renamer ({SMALL_FILES} files, pattern rename) ---");
    let options = RenameOptions {
        pattern: "renamed_{n:4}".into(),
        include_subfolders: true,
        ..RenameOptions::default()
    };
    bench("renamer::plan (400 files)", 3, || {
        plan_rename(&small, &options).expect("plan").len()
    });
    let plan = plan_rename(&small, &options).expect("plan");
    bench("renamer::apply (400 renames)", 3, || {
        apply_plan(&small, &plan, false).expect("apply")
    });
    let records: Vec<RenameRecord> = plan
        .iter()
        .map(|i| RenameRecord {
            from: i.to.clone(),
            to: i.from.clone(),
        })
        .collect();
    bench("renamer::undo (400 renames)", 3, || {
        undo_records(&records).expect("undo")
    });

    println!();
    println!(
        "--- duplicate finder ({DUP_GROUPS} groups × {DUP_COPIES} × {} MB + {UNIQUE_FILES} unique) ---",
        DUP_FILE_BYTES / 1024 / 1024
    );
    let (_, seconds) = bench("duplicates::scan (48 MB tree)", 2, || {
        find_duplicates(&dups, 1, &mut |_| {}).expect("scan")
    });
    let scan = find_duplicates(&dups, 1, &mut |_| {}).expect("scan");
    println!(
        "      → {} groups, {} wasted bytes (expected {expected_wasted}), {} files skipped",
        scan.stats.duplicate_groups, scan.stats.wasted_bytes, scan.stats.skipped_files
    );
    println!(
        "      → {:.0} MB/s hashed",
        mb_s((DUP_GROUPS * DUP_FILE_BYTES * DUP_COPIES) as u64, seconds)
    );

    println!();
    println!("--- folder analyzer ({SMALL_FILES}-file tree / 48 MB dup tree) ---");
    bench("analyzer::small tree (400 files)", 3, || {
        analyze_folder(&small).expect("analyze")
    });
    bench("analyzer::dup tree (60 MB)", 3, || {
        analyze_folder(&dups).expect("analyze")
    });

    println!();
    println!("--- text diff (20 000 lines, 5% changed) ---");
    let old_text = pseudo_lines(20_000, 0);
    let new_text = pseudo_lines(20_000, 1);
    let (_, seconds) = bench("text_diff::diff_texts", 3, || {
        diff_texts(&old_text, &new_text)
    });
    println!("      → {:.0} k lines/s", 20_000.0 / 1000.0 / seconds);

    println!();
    println!(
        "--- validators ({} KB JSON, canonical YAML) ---",
        json_doc.len() / 1024
    );
    bench("validators::validate_json (60 KB doc)", 3, || {
        validate_json(&json_doc)
    });
    bench("validators::validate_yaml", 3, || validate_yaml(&yaml_doc));

    println!();
    println!("Done. Fixtures live in a temp dir and are removed on exit.");
}

fn pseudo_lines(count: usize, salt: u8) -> String {
    let mut out = String::with_capacity(count * 40);
    for i in 0..count {
        // 5% of the lines change between the two versions.
        if i % 20 == salt as usize {
            out.push_str(&format!("line {i} changed-{salt} value {}\n", i * 7));
        } else {
            out.push_str(&format!("line {i} value {}\n", i * 7));
        }
    }
    out
}
