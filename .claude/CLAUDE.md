# Feishin (native)

Feishin is a desktop music player for self-hosted servers (Navidrome, Subsonic/OpenSubsonic, Jellyfin). This branch (`feat/rust-native`) replaces the Electron app with a native Rust + Slint app in `rust/` that uses a fraction of the RAM. It is a single desktop binary (`rust/app`) for Linux and macOS, with no HTTP API; it also announces itself on the local network for player-to-player remote control. History, scope and status of the rewrite: `rust/MIGRATION.md`.

The root `CLAUDE.md` (mirrored in `AGENTS.md`) holds the agent workflow: em dash ban, commit requests, issue tracker, triage labels, domain docs, translations, commit scopes (`docs/agents/commits.md`) and the `ponytail` working mode. This file does not repeat them. Rust code standards live in `rust/CLAUDE.md`.

## 1. Think Before Coding

**Don't assume. Don't hide confusion. Surface tradeoffs.**

Before implementing:

- State your assumptions explicitly. If uncertain, ask.
- If multiple interpretations exist, present them - don't pick silently.
- If a simpler approach exists, say so. Push back when warranted.
- If something is unclear, stop. Name what's confusing. Ask.
- If a server behaves differently from what a backend assumes, check it against a real server (`scripts/live-servers.sh`) before changing the client - never guess a server response shape.

## 2. Goal-Driven Execution

**Define success criteria. Loop until verified.**

Transform tasks into verifiable goals:

- "Add validation" → "Write tests for invalid inputs, then make them pass"
- "Fix the bug" → "Write a test that reproduces it, then make it pass"
- "Refactor X" → "Ensure tests pass before and after"
- "Support X on a server" → "A `wiremock` test in the client's `tests/`, then a live check in `crates/live-tests`"

Specs and issues live in `.scratch/<feature>/` (see `docs/agents/issue-tracker.md`). For multi-step implementation tasks, write the plan in docs/plans/TASK<UnixTimestamp>-<branch-slug>.md from docs/plans/\_template.md before coding, link the `.scratch` spec from it, and wait for approval. Keep its checkboxes and status current.

```
1. [Step] → verify: [check]
2. [Step] → verify: [check]
3. [Step] → verify: [check]
```

Strong success criteria let you loop independently. Weak criteria ("make it work") require constant clarification.

## 3. Rules Layout

- `rust/CLAUDE.md` - Rust and Slint code standards (readability, types, errors, performance, behaviour, translations). Source of truth for code style.
- `.claude/rules/*.md` - core rules. Rules with `paths:` frontmatter apply only to matching files.
  - `architecture.md` - workspace crates, dependency direction, threads, persisted formats.
  - `code-style.md` - general conventions, naming, errors and logging on top of `rust/CLAUDE.md`.
  - `security.md` - credentials, user data, remote control, YouTube cookies, input from servers.
  - `testing.md` - unit, headless Slint, recorded-server and live tests.

## 4. Domain

- **Servers**: every backend implements the `MusicServer` trait (`crates/api`): Navidrome (`api-navidrome`, native API plus Subsonic), Subsonic/OpenSubsonic (`api-subsonic`) and Jellyfin (`api-jellyfin`). Views and the controller talk to the trait only; a feature a server lacks is reported by the client, never special-cased in the UI. Several servers can be configured; one is connected at a time. Servers can be imported from the Electron app (`storage::feishin_import`).
- **Queue** (`crates/core::queue`): Spotify-like. Playing from a list queues the whole list from the clicked song; shuffle is visible and reshuffled on every click; shuffle off restores the original order; re-clicking the current song restarts it. Queue items are compact and shared (`Arc<QueueTrack>`, interned strings) and identified by `Uid`. The queue is synced to the server play queue, keyed on the queue revision.
- **Playback** (`crates/player`): libmpv. Gapless, crossfade 0-12 s with two alternating mpv players, ReplayGain, output device, bitrate/transcoding. Internet radio plays server stations and the radio-browser.info directory with live ICY titles.
- **Large libraries**: lists, albums, search and playlists page from the server while the user scrolls and keep only the visible rows plus a bounded page cache. !IMPORTANT: NEVER load a whole library or list into memory or into a Slint model. Play from a list starts after the first page and streams the rest in. Selections are row ranges; bulk actions stream in 500-song chunks.
- **RAM targets** (desired, not blocking): ≤ 80 MB idle, ≤ 150 MB with a 20k-track list open, measured with `scripts/measure-ram.sh`. Changes to lists, queue or covers are measured with `--demo --demo-songs 100000`.
- **YouTube Music** (`crates/youtube`, Discover view): songs found in the server library play from the server; others play from YouTube through the external `yt-dlp` binary, resolved off the UI thread. YouTube tracks are never scrobbled or synced to the server. Daily Mixes and Discover Weekly are built from the server's most played and favorite songs and cached once a day (UTC) per server.
- **Remote control** (`crates/remote`, off by default): players are announced with mDNS (`_feishin-remote._tcp`) and accept newline-delimited JSON over TCP from the local network only, after the user approves a device on the controlled player. Server songs move between players only when both use the same server (URL and user).
- **Platform** (`crates/platform`): freedesktop (MPRIS, D-Bus sleep inhibitors, portal color scheme) on Linux, macOS (Now Playing and media keys through `souvlaki`, IOKit sleep assertion, `AppleInterfaceStyle`). Discord Rich Presence on both. Windows is out of scope for now.
- **Scrobbling**: server songs are scrobbled to the server; radio and YouTube tracks never are.
- The Electron/TypeScript app was removed in wave 7; it is in git history only. Butterchurn visualizer, waveform, mobile layout and the web remote are cut on purpose.

## 5. Environment

- Rust, edition 2024, Cargo workspace in `rust/` (resolver 3), stable toolchain
- Slint 1.18 for the UI (`rust/app/ui/*.slint`), `i-slint-backend-testing` for headless UI tests
- libmpv through `libmpv2` (system libmpv; Homebrew's on macOS)
- `tokio` + `reqwest` (rustls) for async and HTTP, `serde`/`serde_json`, `thiserror` in crates and `anyhow` in the app
- `tracing` + `tracing-subscriber` (`RUST_LOG`) for logs
- Storage: TOML settings, `keyring-core` (Secret Service / login keychain) for passwords, `bincode` for `queue.bin`, `rusty-leveldb` to import the Electron app's data, an LRU + disk cache for covers. Files under `~/.config|.cache|.local/share/feishin-rs` on Linux and `~/Library/...` on macOS (`storage::paths`)
- Workspace lints deny `unwrap_used`, `expect_used`, `panic`, `unreachable`, `todo`, `unimplemented`, `indexing_slicing` (allowed in tests, `rust/clippy.toml`); `too-many-lines-threshold = 40`
- gettext catalogs for 39 languages in `rust/app/translations`, managed by `rust/scripts/translations.py`
- External tools at runtime: `yt-dlp` (optional, YouTube)
- Live tests: podman, ffmpeg, curl, jq (`scripts/live-servers.sh`)
- CI: GitHub Actions `rust.yml` (fmt, clippy, test on Ubuntu), `rust-appimage.yml` (manual AppImage), `commitlint.yml`
- Distributed as AppImage, Arch `feishin-native-git` PKGBUILD and an ad hoc signed macOS `Feishin.app` (`rust/packaging/`); app id `org.jeffvli.feishin.native`

## 6. Commands

Run from `rust/`.

| Command | Description |
|---------|-------------|
| `cargo run -p feishin` | Run the app against your servers |
| `cargo run -p feishin -- --demo` | Run with a generated library and silent audio, no server needed |
| `cargo run --release -p feishin -- --demo-songs 100000` | Demo with a huge library (RAM and paging checks) |
| `cargo build --workspace` | Build every crate |
| `cargo fmt --all` / `cargo fmt --all --check` | Format / check formatting |
| `cargo clippy --workspace --all-targets -- -D warnings` | Lint (CI fails on any warning) |
| `cargo test --workspace` | Run all tests (live tests are `#[ignore]`d) |
| `cargo test -p feishin-core queue` | Run the tests of one crate matching a filter |
| `cargo test -p feishin-core add_last_without_shuffle_appends` | Run a single test |
| `python3 scripts/translations.py` / `python3 scripts/translations.py check` | Extract new strings into every catalog / check every language is complete |
| `scripts/live-servers.sh up` / `test` / `down` | Start Navidrome, gonic and Jellyfin containers, run the live tests against them, remove them |
| `scripts/measure-ram.sh` | Measure RSS/PSS of a running app |
| `scripts/install.sh` | Build and install the app locally |
| `packaging/appimage/build-appimage.sh` | Build the AppImage |
| `packaging/macos/build-app.sh` | Build `Feishin.app` with libmpv bundled |

## 7. Before Finishing

- Ensure `cargo fmt --all --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass.
- Ensure `cargo build --workspace` succeeds.
- Ensure `cargo test --workspace` passes.
- Every new or changed branch of logic has a test.
- User-visible text changed: `python3 scripts/translations.py check` reports every language complete.
- A change to a server client: the live tests touching it pass (`scripts/live-servers.sh up && scripts/live-servers.sh test`), or say they were not run.
- A change to lists, queue, covers or caches: say whether RAM was measured (`--demo-songs 100000`).

## 8. Conventions

- Commits follow Conventional Commits with the scopes in `docs/agents/commits.md`, written in English.
- Never put credentials, tokens or real user data (server URLs, usernames, library contents) in code, tests or examples - use fictitious values.
- Never use the em dash; see the root `CLAUDE.md`.
