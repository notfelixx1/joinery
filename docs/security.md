# Security & Safety Model

Safety is a product feature of Joinery, not an afterthought. This document
lists the guarantees the 0.1 core makes and how they are enforced.

## Privacy guarantees

- **No network.** The app performs no HTTP requests; the Tauri config
  ships a restrictive CSP (`default-src 'self'`). There are no update
  pings, no crash reporting, no analytics — nothing.
- **No telemetry.** Nothing is collected, nothing is sent.
- **Local storage only.** SQLite database (settings + undo journal) in the
  OS app-data directory. Uninstalling removes the app; the database stays
  with the user.

## File-operation guarantees

### Batch Renamer

1. **Preview-first.** Renames never happen without a rendered preview.
2. **Status gating.** Only items marked `Ready` execute. Items marked
   `TargetExists`, `BatchDuplicate` or `InvalidName` are never executed
   by the plan builder; `BatchDuplicate` additionally blocks the Apply
   button until resolved.
3. **Escape protection.** Every source *and* target path is verified to
   resolve inside the selected folder (`safety::preview::is_within`,
   canonical on existing paths, lexical otherwise, case-insensitive on
   Windows). Forged command payloads cannot rename files outside the
   folder.
4. **No silent overwrites.** Overwriting an existing target requires an
   explicit UI checkbox (`allow_overwrite`) and is visible in the preview.
5. **Undo journal.** Each applied batch is persisted before the success
   message; Undo replays records in reverse and skips entries that would
   clobber files created after the rename.
6. **Name validation.** Target names are checked for platform-invalid
   characters, Windows reserved device names (`CON`, `COM1`, …), control
   characters and trailing dots/spaces.

### Duplicate Finder

- Deletion is **only** to the system trash, **only** after an explicit
  confirmation dialog, **only** for the exact files the user ticked.
- The scan itself is read-only.

### Checksum / Analyzer / Validators

- Read-only operations.

## Threat model notes

- The Rust command layer treats the frontend as untrusted input: paths are
  re-validated server-side (in the core), and options are typed serde
  structs — unknown fields are rejected rather than ignored.
- Long operations stream progress and can be observed; the UI stays
  responsive (`spawn_blocking`).
- Text-file loading for the Developer tools is capped at 5 MiB to keep
  memory predictable on weak machines.

## Known limitations (0.1)

- Trash restore is out of scope: files moved to the trash are recovered
  via the OS, not via Joinery.
- The undo journal covers renames only; duplicate deletion is
  trash-based, which is the recovery path.
