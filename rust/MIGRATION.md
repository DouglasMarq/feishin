# Feishin native Rust migration

Goal: replace the Electron app with a native Rust app that uses a fraction of the RAM.

- Today: ~530 MB PSS / ~977 MB RSS (installed Feishin, Electron 43, measured 2026-10-13 with `scripts/measure-ram.sh`).
- Desired (not mandatory): <= 80 MB idle, <= 150 MB with a 20k-track list open. Measured at the end of every wave to catch regressions, but missing it does not block a wave.

## Status (2026-10-13)

- Wave 0: done (Slint chosen; spike numbers below).
- Wave 1: done and in daily testing against Navidrome (login/import, Tracks, Spotify-like queue, player bar, queue panel, MPRIS, scrobble, persistence).
- Wave 2: views done (home, albums, artists, genres, favorites, playlists + picker, search, tracks context menu/filters); needs real-library testing.
- Wave 3: done (gapless, crossfade 0-12 s with two alternating mpv players, ReplayGain, output device, bitrate, hotkeys, sleep timer, full-screen now playing, server queue sync).
- Wave 4: Subsonic and Jellyfin backends implemented and tested against real servers in containers (`scripts/live-servers.sh up|test|down`: Navidrome latest/0.54/0.56, gonic, Jellyfin 12.2/10.10; 328 live tests, 11 backend bugs fixed).
- Wave 5: lyrics, Discord, internet radio (server stations plus the radio-browser.info directory, live ICY titles), 32 themes from the Electron app with follow-system light/dark and accent color, folder browsing, share links. i18n not started.
- Discover (beyond the Electron app): a view fed by YouTube Music (For you, new releases, charts per country, moods). Songs found in the server library play from the server; others play from YouTube through the external `yt-dlp` binary, resolved to a direct URL off the UI thread. Signing in uses the cookies of a browser chosen in Settings, exported by yt-dlp into `$XDG_RUNTIME_DIR/feishin-rs` and deleted after reading. YouTube tracks are never scrobbled or synced to the server.
- Power management: screen locking and system sleep are blocked while playing, through the `org.freedesktop.ScreenSaver` and `org.freedesktop.PowerManagement` D-Bus inhibitors (the Electron app had this behind two off-by-default settings; the native app always does it).
- Wave 7: packaging done (`packaging/`: desktop entry and metainfo for `org.jeffvli.feishin.native`, Arch `feishin-native-git` PKGBUILD, AppImage script, manual GitHub workflow). Removing the Electron app is still pending, blocked on i18n (which reuses `src/i18n/locales`) and real-library testing of the wave 2 views.
- Demo mode RSS/PSS with all views: 135 / 91 MB.
- Huge libraries (measured with `--demo --demo-songs 100000`, release build):
  - Idle RssAnon about 46 MB at both 20k and 100k songs; total RSS about 141 MB.
  - Lists, albums, search and playlists page from the server with a bounded page cache (1.6 MB retained after scrolling 100k rows).
  - Queue items are compact and shared (`Arc<QueueTrack>` with interned strings): 23.5 MiB per 100k songs. The queue panel and Up Next build rows on demand from a shared `QueueView`.
  - Play from a list starts after the first page (about 1-5 ms from click to playback, previously 100-425 ms at row 50000) and streams the rest in; peak RssAnon about 95-105 MB, previously 160 MB.
  - Track selection is stored as row ranges; bulk actions and add-to-playlist stream in 500-song chunks.
  - The queue is saved off the UI thread in a compact binary file (`queue.bin`, 5.6 MB per 100k songs, previously 24 MB of JSON), debounced, and also on SIGTERM. It loads after the window opens. Server queue sync is keyed on the queue revision.
- Known limitations:
  - A server-side change that keeps a list's size (one add and one delete) is not detected while scrolling.
  - Quitting while a play-from-list stream takes longer than 3 s to load saves only the part loaded so far.
  - The signed-in YouTube Music home has never been tested against a real account; the YouTube Music web client version is hard-coded.
  - Radio mode is not restored after a restart and dropped streams do not reconnect automatically.

## Scope decisions

- Linux only at first. Windows/macOS later, after cutover.
- Navidrome first. Subsonic/OpenSubsonic and Jellyfin in wave 4.
- Cut: Butterchurn visualizer, waveform, mobile layout, remote control.
- The TypeScript app stays in the repo as reference until wave 7.

## Stack

| Layer | Choice | Notes |
|---|---|---|
| UI | Slint (fallback: iced) | Final choice made by the wave 0 spike |
| Audio | libmpv (`libmpv2`) | Same engine as Feishin's mpv player |
| HTTP / async | `reqwest` + `tokio` | |
| MPRIS / media keys | `souvlaki` | |
| Credentials | `keyring` | |
| Cover art | `image` + disk cache + bounded LRU | Main RAM risk |
| Config | `serde` + TOML in `~/.config/feishin-rs` | |

## Workspace layout

```
rust/
  crates/core      domain types + queue (port of player.store.ts), unit tested
  crates/api       MusicServer trait (mirrors controller.ts) + per-server clients
  crates/player    libmpv wrapper
  crates/platform  MPRIS, media keys, Discord
  app/             UI binary
  spikes/          wave 0 throwaway experiments
```

## Waves

### Wave 0 - Foundation (~1 week)
- Cargo workspace, CI (`cargo clippy`, `cargo test`), RAM measurement script.
- UI spike: 20k-row virtualized table with resizable columns + cover grid; measure RAM and smoothness.
- Playback proof: stream from Navidrome through libmpv.
- Exit: UI toolkit chosen (RAM measured against the desired target); a track plays.

Wave 0 results (2026-10-13, `spikes/ui-slint`, release build, Wayland):

| Scenario | PSS | RSS |
|---|---|---|
| 20k tracks loaded, idle | 52 MB | 99 MB |
| Autoscrolling the 20k-track list | 49 MB | 95 MB |
| Autoscrolling a 3,000-album cover grid (LRU of 120 covers) | 72 MB | 118 MB |

Decision: Slint. iced spike not needed.

### Wave 1 - Daily-driver MVP (~3-5 weeks)
- Navidrome login; import servers from Feishin (`~/.config/feishin`).
- Tracks: virtualized list, infinite paging, sort, basic filters, search.
- Spotify-like queue: whole list from the clicked song; visible shuffle reshuffled on every click; shuffle off restores original order; re-clicking restarts the song.
- Player bar: play/pause, next/prev, seek, volume, repeat, shuffle.
- Queue panel: play, remove, reorder.
- MPRIS + media keys, scrobble, persisted settings, queue restore on start.
- Exit: replaces Feishin day to day.

### Wave 2 - Library navigation (~3-4 weeks)
- Albums and artists (grid/list + detail pages), genres, folders, favorites, ratings.
- Playlists: view, play, create, add, reorder.
- Home (recent, random, most played), global search, context menus.

### Wave 3 - Advanced playback (~2-3 weeks)
- Gapless, ReplayGain, crossfade, output device, bitrate/transcoding.
- Hotkeys, sleep timer, full-screen player, server play queue sync.

### Wave 4 - Other servers (~2-3 weeks)
- Subsonic/OpenSubsonic and Jellyfin behind the same `MusicServer` trait; multiple servers.

### Wave 5 - Extras (~3-4 weeks)
- Lyrics (lrclib, NetEase, Genius; synced), Discord Rich Presence, radio, similar songs, sharing.
- i18n reusing `src/i18n/locales`, themes.

### Wave 6 - Optional heavy features (decide one by one)
- Tag editor (`lofty`), DLNA casting (`rupnp`), jukebox mode.

### Wave 7 - Cutover
- Packaging (AppImage, AUR, Flatpak), remove Electron/TypeScript, README, release.

## Risks
- UI toolkit maturity for resizable columns, drag and drop and context menus (validated in wave 0).
- Cover art RAM: needs bounded caches from day one.
- Total effort to wave 5 is roughly 4-6 months full time; wave 1 alone covers daily use.
