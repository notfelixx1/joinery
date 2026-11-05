# Contributing to Joinery Tools

Thanks for helping! The project keeps a deliberately small scope: quality
over feature count.

## Ground rules

1. **Local-first, forever.** No network calls, no telemetry, no accounts.
   A PR that phones home will not be merged.
2. **Preview before destruction.** Any tool that modifies files must show
   what it is about to do and support undo (rename) or go to the trash
   (delete).
3. **Each tool is a module.** New tools live in `core/src/core/<tool>.rs`
   and get a view in `native-ui/src/views/`. Nothing tool-specific belongs
   in the core `lib.rs` or in the app update loop.
4. **Tests for the core.** New logic in `core/` or `safety/` needs unit
   tests (`cargo test`). UI changes should be verifiable manually.

## Development

```bash
cargo build -p joinery          # debug build
cargo test                      # run core tests
cargo run -p joinery            # run the app
```

## Code style

- Rust: `cargo fmt`, zero clippy warnings on touched code.
- iced UI: keep views focused; strings come from `native-ui/src/i18n.rs`
  (English **and** Russian for every user-visible string).
