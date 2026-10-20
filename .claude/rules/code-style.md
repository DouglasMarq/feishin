---
paths:
  - "rust/**/*.rs"
  - "rust/**/*.slint"
  - "rust/**/Cargo.toml"
---

`rust/CLAUDE.md` is the source of truth for Rust and Slint code standards (readability, strict types, no crashing, performance, behaviour, translations). This file adds the general conventions around it and never overrides it.

## Language

- !IMPORTANT: Code, identifiers, log messages and commit messages are written in English. User-visible text is English in the source and translated through the catalogs (`rust/CLAUDE.md` §Behaviour).
- NEVER use the em dash in code, strings, docs or commit messages - use "-".

## General

- !IMPORTANT: Follow idiomatic Rust, `rustfmt` and the workspace clippy lints - run `cargo fmt --all`, never hand-format against it, and keep `cargo clippy --workspace --all-targets -- -D warnings` clean.
- Prefer small, single-purpose functions (clippy flags functions over 40 lines) and explicit types at crate boundaries.
- NEVER mix responsibilities. Each crate, module, type and function owns a single concern and must not take on work that belongs to another or depend on its internals. For example, a server request belongs in its `api-*` client, never in a view or in the controller.
- NEVER create Magic Numbers - define `const`s for values with special meaning (`const PAGE_SIZE: usize = 500;`, `Duration` constants for timeouts).
- Avoid Side Effects - `crates/core` functions do not perform I/O or touch global state. I/O (HTTP, disk, mpv, D-Bus, logging of events) belongs in the infrastructure crates and the controller.
- !IMPORTANT: Code is strictly typed (`rust/CLAUDE.md` §Types): enums and newtypes for states and ids, NEVER `serde_json::Value`, `dyn Any` or stringly-typed state outside raw response decoding.
- Avoid Regex - prefer `str` methods, `split`, `strip_prefix` and parsers.
- !IMPORTANT: Verify if a helper already exists (in the crate, `crates/core`, `crates/api`, the standard library or an existing dependency) before implementing a new one.
- NEVER implement helpers or utilities outside the crate or module that owns the concern.

## Conventions

- NEVER implement features beyond what was asked.
- NEVER create abstractions for single-use code.
- NEVER add "flexibility" or "configurability" that wasn't requested.
- NEVER add error handling for impossible scenarios.
- If you write 200 lines and it could be 50, rewrite it.
- NEVER "improve" adjacent code, comments, or formatting.
- NEVER refactor things that aren't broken.
- !IMPORTANT: Match existing style, even if you'd do it differently.
- !IMPORTANT: If you notice unrelated dead code, mention it - don't delete it.
- !IMPORTANT: Remove imports/variables/functions that YOUR changes made unused.
- NEVER remove pre-existing dead code unless asked.
- !IMPORTANT: If you notice that a well-maintained crate would simplify your code, you should suggest using it.

!IMPORTANT: Ask yourself: "Would a senior engineer say this is overcomplicated?" If yes, simplify.

## Naming

- !IMPORTANT: Names describe intent, not implementation (`play_from_list`, not `handle_click2`).
- !IMPORTANT: Booleans use an `is_`/`has_`/`can_` prefix (`is_playing`, `has_cover`).
- !IMPORTANT: Prefer `Duration` and typed values over raw numbers. When a number carries a unit, include it in the name (`position_ms`, `duration_ms`, `size_bytes`).
- NEVER use abbreviations or one-letter names, including closure arguments (`rust/CLAUDE.md` §Readability; clippy `min-ident-chars`).

| Element | Convention | Example |
| ------- | ---------- | ------- |
| Crate | `feishin-<concern>` | `feishin-api-navidrome` |
| Module / file | snake_case | `list_streams.rs` |
| Type / trait | UpperCamelCase | `MusicServer`, `QueueTrack` |
| Function / variable | snake_case | `song_list`, `row_index` |
| Constant | UPPER_SNAKE_CASE | `QUEUE_FILE` |
| Slint component | UpperCamelCase, file in snake_case | `AlbumGrid` in `album_grid.slint` |
| Error enum | `<Crate>Error`, variants by cause | `ApiError`, `StorageError::Invalid` |

## Comments

- NEVER add comments of any kind (`//`, `///`, `//!`, `/* */`) in Rust or Slint, and NEVER leave commented-out code (`rust/CLAUDE.md` §Readability).

## Error handling

- NEVER swallow errors silently. Return them with `?`, convert them to the crate's typed error, or - for ignorable failures - log them with `tracing` (`rust/CLAUDE.md` §Errors).
- !IMPORTANT: Preserve the original cause when wrapping (`#[from]` / `#[source]` in `thiserror`, `.context(..)` in the app with `anyhow`).
- Library crates use `thiserror` enums; `anyhow` only in `app`.
- Failures the user must know about are shown through `AppState.status`; NEVER only logged.
- !IMPORTANT: Log through `tracing` (`tracing::warn!("removing the old {LEGACY_QUEUE_FILE} failed: {error}")`).
- NEVER use `println!`/`eprintln!`/`dbg!` in application code. Exceptions: `main.rs` reporting a fatal startup error, and test-only output.

## Anti-patterns

- NEVER suppress compiler or clippy lints (`#[allow(..)]`, `#[expect(..)]`) to get a change through - fix the cause. If a lint is genuinely wrong for the code, ask first.
- NEVER leave debug output or `dbg!` in committed code.
