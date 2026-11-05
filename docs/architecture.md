# Architecture

Joinery Tools is a Rust core with a native [iced](https://iced.rs/)
frontend. The frontend calls the core in-process — there is no IPC, no
bundled browser and no network access.

```
┌─────────────────────────────────────────────┐
│  iced UI (native-ui/)                       │
│  pages: Home · Files · Developer · System   │
│  · Support, custom palette, i18n (en/ru)    │
└────────────────┬────────────────────────────┘
                 │ direct function calls
┌────────────────▼────────────────────────────┐
│  joinery-tools core (core/)                 │
├─────────────────────────────────────────────┤
│  core/                                        │
│  renamer · duplicates · analyzer · checksum │
│  text_diff · validators                     │
├─────────────────────────────────────────────┤
│  safety/                                      │
│  preview (path checks, collision detection) │
│  undo (journal, trash delete)               │
├─────────────────────────────────────────────┤
│  storage/ (SQLite)                          │
│  settings · undo_journal                    │
└─────────────────────────────────────────────┘
```

## Principles

1. **The UI never touches files.** Every operation goes through the Rust
   core, which validates paths, checks collisions and refuses to act
   outside the selected folder.
2. **Each tool is a separate module** in `core/`. Adding a tool must not
   require touching the others.
3. **Heavy work is off the UI thread.** Long scans and hashing run in
   `tokio::task::spawn_blocking`; the iced frontend uses subscriptions for
   progress updates.
4. **Deterministic core.** `core/` and `safety/` are plain Rust without
   GUI types — fully covered by unit tests and reusable by any frontend.

## Data flow example: Batch Renamer

1. UI collects options → `core::renamer::plan_rename(root, options)`.
2. `plan_rename` builds a plan: every file gets a target name and a status
   (`Ready`, `NoChange`, `TargetExists`, `BatchDuplicate`, `InvalidName`).
   Nothing is written to disk.
3. User presses **Apply** → `core::renamer::apply_plan(root, items, allow_overwrite)`.
4. `apply_plan` re-verifies that each path is inside the root
   (`safety::preview::is_within`), never overwrites unless explicitly
   allowed, and returns the applied records.
5. The UI stores the records in the SQLite `undo_journal`
   (`safety::undo::save_journal`).
6. **Undo** → `safety::undo::undo_records(records)` replays the records
   backwards; conflicting entries are skipped, never overwritten.

## Storage

One SQLite database in the OS app-data directory:

- `settings` — theme, language (key/value).
- `undo_journal` — applied rename batches (JSON payload + `undone` flag).

Nothing else is persisted; nothing leaves the machine.
