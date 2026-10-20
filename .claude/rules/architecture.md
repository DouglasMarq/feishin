---
paths:
  - "rust/**"
---

## Layering

The Cargo workspace in `rust/` has one binary (`app`) and library crates around a shared domain crate (`crates/core`). Dependencies point inwards:

```
app (Slint UI + controller)  →  api-navidrome / api-subsonic / api-jellyfin  →  api (MusicServer trait)  →  core (domain)
                             →  player, remote, storage                                                  →  core
                             →  platform, lyrics, radio-browser, youtube        (no workspace dependencies)
```

- `crates/core` - domain model (`model::Song`, `Album`, `ServerConfig`, paging, queries) and the queue (`queue::Queue`, `QueueTrack`, `Uid`). Plain Rust plus `serde`; no I/O, no async, no other workspace crate.
- `crates/api` - the `MusicServer` trait, `ApiError`, `Credential`, `StreamOptions` and shared caches. Server clients (`api-navidrome`, `api-subsonic`, `api-jellyfin`) implement it and are the only crates that know a server's wire format.
- `crates/player` (libmpv), `crates/storage` (settings, credentials, queue file, cover cache, Electron import), `crates/platform` (MPRIS/D-Bus, macOS Now Playing, Discord), `crates/remote` (mDNS + TCP remote control), `crates/youtube` (YouTube Music + `yt-dlp`), `crates/lyrics`, `crates/radio-browser` - infrastructure: each wraps one external system.
- `app/` - the transport layer: `ui/` (one module per view, bound to `app/ui/*.slint`), `controller/` (the `Controller` owns the queue, playback and settings; it receives `Command`s on an mpsc channel and publishes `UiUpdate`s through `UiSink`), and the glue that wires crates together (`app.rs`, `session.rs`, `store.rs`, `list_stream/`, `discover/`, `remote/`).
- !IMPORTANT: `crates/core` NEVER depends on another workspace crate, Slint, tokio, reqwest or mpv. Infrastructure crates NEVER depend on `app`, and NEVER depend on each other unless the dependency already exists in `Cargo.toml` (today only `storage` → `api`, `youtube`).
- !IMPORTANT: Views (`app/src/ui`, `app/ui`) NEVER call a server, mpv, the disk or the network directly - they send a `Command` or go through the session/store, and render what the controller publishes.
- The app reaches servers only through `dyn MusicServer`. NEVER branch on the server kind outside the client crates; a capability a server lacks is expressed by the trait (an error or an empty result), not by the UI.
- Work that blocks or waits (HTTP, `yt-dlp`, disk writes, cover decoding) runs on the tokio runtime, NEVER on the Slint event loop. Results go back to the UI through `UiSink` / `slint::invoke_from_event_loop`.

## Organization

- One crate per external system, one module per feature or view inside a crate (`app/src/ui/<view>/`, `app/src/controller/<concern>.rs`, `crates/<crate>/src/<concern>/`).
- Wire formats (server JSON, YouTube responses, remote control messages, files on disk) are separate types from `core::model` and map to it explicitly in the crate that owns the format (`api-subsonic/src/types`, `youtube/src/music/raw`).
- Tests live next to the code (`tests.rs` or a `tests/` module) or in the crate's `tests/` folder for integration tests - see `testing.md`.
- The folder layout:

```
rust/
  Cargo.toml              workspace, shared dependencies and lints
  clippy.toml             thresholds; unwrap/expect/panic/indexing allowed in tests
  app/
    src/
      main.rs, app.rs     entry point, runtime and wiring
      cli.rs              --demo / --demo-songs
      controller/         Controller and its concerns (playback, queue_ops, list_streams, remote, youtube, ...)
      ui/<view>/          one module per view, bound to the Slint components
      list_stream/        paging of large lists into the queue
      discover/           YouTube Music views and personal mixes
      demo/               generated library for --demo
      tests/              app-level tests
    ui/*.slint            Slint components
    translations/<lang>/  gettext catalogs
    tests/                integration tests (desktop integration)
  crates/
    core/                 domain model + queue
    api/                  MusicServer trait
    api-<server>/         one client per server kind, with recorded responses in tests/
    player/ platform/ storage/ remote/ youtube/ lyrics/ radio-browser/
    live-tests/           #[ignore]d tests against real servers in containers
  scripts/                translations, live servers, RAM measurement, install
  packaging/              AppImage, Arch, macOS bundle, desktop entry
  spikes/                 wave 0 throwaway experiments - not maintained
```

## Persisted data

- !IMPORTANT: Files written by a released build must still load after an upgrade: `settings.toml` (every new field has `#[serde(default)]`), `queue.bin` (a `FileFormat` with a magic per version, `FSQ1`/`FSQ2`; a new layout gets a new magic and the old one keeps a reader, as `QUEUE_FORMAT_V1` and the legacy `queue.json` do), `remote-devices.json`, the discover mix cache. NEVER change the layout of an existing format version.
- A file that fails to load is logged and replaced by defaults - the app NEVER refuses to start because of a stored file.
- The queue is saved off the UI thread, debounced, and on SIGTERM; NEVER write it synchronously from the UI.

## Anti-patterns

- NEVER create circular dependencies between crates or modules.
- NEVER expose a server's wire types outside its client crate.
- NEVER keep an unbounded collection of songs, rows or decoded images in memory - page it or cache it with a bound.
- NEVER add abstractions, configuration or extension points that no current requirement needs.
