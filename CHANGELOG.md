# Changelog

All notable changes to Joinery Tools are documented here.
Format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **Settings page** — a gear button at the bottom of the sidebar opens
  grouped settings. Appearance: two palette presets (dark and light)
  plus five individually editable colors — buttons & accents, app
  background, sidebar, cards & surfaces, text — each as a live
  `#RRGGBB` hex input (typed like `#11AA11`) with a preview swatch.
  Hover/active surfaces, borders, dividers and muted text are derived
  automatically, so any recolor stays coherent; text-on-accent
  contrast is computed from luminance. Everything persists across
  restarts. Functionality: the checksum auto-copy toggle moved here
  from the tool page.

- **Window memory** — the size and position come back after a
  restart; a maximized session keeps its saved normal-size geometry.
- **Checksum auto-copy** — on by default: a single-file hash lands in
  the clipboard the moment it is computed, reusing the transient
  "Copied" label; the toggle sits next to the algorithm.
- **Open where you left off** — the last page, tool and sidebar
  section come back on startup; command-line flags still override.
- **Progress in the top bar** — a thin strip under the header driven
  by long operations: per-file checksum hashing and duplicate
  scanning.

### Changed

- **Themes removed** — the dark/light toggle (and the `--dark` /
  `--light` flags) are replaced by the user palette: the background,
  sidebar, cards and text are colored individually, so a theme is just
  a palette preset now. The language picker stays in the top bar.
- **Diff engine** — the external `imara-diff` (Histogram) is replaced
  by an in-house contiguity-first engine: anchors are maximal common
  runs discovered through line-trigram seeds, selected by a
  maximum-weight monotone chain and refined per gap. On the validation
  protocol — real file pairs from the project history, five synthetic
  classes up to 1M lines and a 600-case mutation fuzz — it matches the
  reference quality everywhere, peaks at 201 MB vs 229 MB on a
  1M-line diff, and runs the scattered-changes class 28–206× faster.
  The external dependency is dropped; the property and fuzz tests now
  live in the core test suite.

## [0.1.2] — 2026-11-17

### Added

- **Breadcrumbs in the top bar** — tool pages read as
  "Files → Checksums": the section stays muted, the active tool is in
  the normal text color.
- **Collapsible sidebar sections** — "Files" and "Developer" are groups
  now: the header slides out the tool sub-items (chevron + a short
  reveal animation), and a sub-item jumps straight to that tool from
  any page. Launching on a tool via `--renamer` & co. opens its group.
- **Eight interface languages** — English, Русский, Українська,
  Español, Deutsch, Français, Português, Italiano. Translations live
  in editable JSON dictionaries (`native-ui/i18n/<code>.json`),
  picked in the top bar or via `--lang`; tests enforce that every
  language covers all keys and keeps the `{var}` placeholders.

### Fixed

- The footer version comes from the crate version instead of a
  hardcoded string that stayed at 0.1.0 after the release bump.

## [0.1.1] — 2026-11-10

### Added

- **History page** — every applied batch rename is listed with its date,
  tool and file count; undo works right from the list, undone entries
  stay as quiet history.
- **Empty states** for the file tools: an icon, one sentence and the
  next action instead of silent empty forms.
- **Recent folders** — the last five folders per tool are remembered in
  SQLite and offered as one click in the empty states.
- **Ko-fi** — the Support page opens the donation page in the browser,
  and the README links it directly.
- **Command-line flags** — `joinery --renamer --folder "D:\photos"
  --dark` opens straight on a tool with the theme, language and folder
  preset; `--help` lists them all.
- **Scaling benchmarks** — `core/examples/bench.rs` measures every core
  module at three scales (100/1k/10k files, 100 MB/1 GB hashing,
  20k/100k/1M diff lines, deep vs wide trees) with correctness asserts
  inside each timed operation and machine-readable `--json` output;
  `scripts/bench_chart.py` renders the results chart.

### Changed

- **Diff engine** — `similar` (Myers) is replaced by `imara-diff`
  (Histogram, the engine used by the Helix editor): a 1M-line diff
  drops from ~5 minutes to ~2 minutes, and 100k-line inputs from
  3.3 s to 1.2 s on the reference machine.
- **Duplicate scanning** — files up to 256 KB are read once and fully
  hashed from memory instead of being opened twice (partial + full
  passes); hashing uses 1 MiB read buffers.
- Button feedback everywhere: a pressed state (the button sinks one
  surface step), hover fills on ghost and danger buttons, tooltips on
  the icon-only buttons and the overwrite checkbox, a transient
  "Copied" label on checksum results, and an undo confirmation dialog
  on the History page.

### Fixed

- **Security** — the bundled SQLite moved to 3.54.1 (`rusqlite` 0.41 →
  `libsqlite3-sys` 0.39), closing CVE-2026-119375: a crafted `-wal`
  file next to a database could make SQLite return uninitialized
  page-cache memory in ordinary queries. The minimum build toolchain
  is now Rust 1.89. The benchmark example also compiles on ARM64
  again (the SHA-NI probe is gated by target architecture), which
  unblocks the macOS CI job.

## [0.1.0] — 2026-11-05

First public milestone: six quality tools instead of twenty half-made ones.

### Added

- **Native frontend** (`joinery`, iced/tiny-skia): all six tools, dark/light
  themes, EN/RU, small memory footprint and a single process.
- **Batch Renamer** — find/replace, prefix/suffix, extension change,
  numbering patterns (`{name}`, `{ext}`, `{n}`, `{n:3}`), recursive mode.
  Preview-first workflow with collision detection, optional overwrite
  protection and one-click undo via a local SQLite journal.
- **Duplicate Finder** — three-pass scan (size → 4 KiB partial hash → full
  SHA-256), results sorted by wasted space, per-file selection, deletion
  only to the system trash after confirmation.
- **Folder Analyzer** — top-level size breakdown, largest-files list,
  one-pass tree walk.
- **Checksum** — SHA-256 / SHA-1 / MD5, streamed in 64 KiB chunks, verify
  against an expected hash.
- **JSON / YAML Validator** — validation with exact line/column of errors,
  pretty-print and minify for JSON, canonical output for YAML.
- **Text Diff** — line-based diff with add/remove/unchanged statistics and
  file loading.
- Dark & light themes, English & Russian interface (persisted in SQLite).
- Windows / Linux / macOS build pipeline (GitHub Actions).

### Fixed

- The Windows release build no longer opens a console window next to
  the app; debug builds keep it for the logs.
- Dark theme survives a restart — the saved theme setting is honored on
  startup.
- The folder-safety check resolves symlinked roots correctly (macOS
  `/tmp`), instead of flagging every rename as escaping the folder.
- Batch renamer and undo move files without ever replacing an existing
  destination, closing the check-then-rename race; undo continues past
  a failed record instead of stopping mid-way.
- Duplicate Finder counts unreadable files in the scan statistics.
