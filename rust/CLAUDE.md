# Rust rewrite: code standards

These apply to everything under `rust/`. Clippy enforces the mechanical ones (`cargo clippy --workspace --all-targets -- -D warnings`).

## Readability

- Code documents itself. No comments of any kind (`//`, `///`, `//!`, `/* */`), in Rust or Slint.
- Descriptive names for functions, variables, parameters and closure arguments. No one-letter or cryptic names (`i`, `k`, `e`, `s`, `tx`, `res`, `tmp`): use `index`, `key`, `error`, `song`, `sender`, `response`.
- Small functions and small files. A function does one thing; split long ones into well-named helpers. Split files that mix concerns into modules.
- Prefer `match`, early returns, `let ... else` and `Option`/`Result` combinators over `if`/`else` chains.

## Types

- Strictly typed: no `serde_json::Value`, `dyn Any` or stringly-typed state outside the raw response decoding of an API client. Model states with enums and newtypes.

## Errors

- Never crash in the user's face: no `unwrap`, `expect`, `panic!`, `unreachable!`, `todo!` or indexing that can go out of bounds in non-test code. Use `?`, `.get()`, `.ok_or(..)`, `.unwrap_or(..)`, `.unwrap_or_default()`.
- Ignorable failures (a missing cover, a failed scrobble, an MPRIS update) are logged with `tracing` and otherwise silent.
- Failures the user must know about (login, playback, saving a playlist) are shown in the UI through `AppState.status`.

## Performance

- Large collections are streamed: views page data from the server while the user scrolls and only keep what is needed for the visible rows plus a bounded cache. Never load a whole library into memory or into a Slint model.
- Decoded images live in bounded caches.

## Behaviour

- Refactors keep behaviour identical; the tests are the contract. Add tests when changing logic.
- Every clickable or draggable control has a tooltip; stateful controls show their state.
