# Changelog

All notable changes to Joinery Tools are documented here.
Format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- **History page** — every applied batch rename is listed with its date,
  tool and file count; undo works right from the list, undone entries
  stay as quiet history.
- **Empty states** for the file tools: an icon, one sentence and the
  next action instead of silent empty forms.
- **Recent folders** — the last five folders per tool are remembered in
  SQLite and offered as one click in the empty states.

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
