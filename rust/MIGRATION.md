# Feishin native Rust migration

Goal: replace the Electron app with a native Rust app that uses a fraction of the RAM.

- Today: ~530 MB PSS / ~977 MB RSS (installed Feishin, Electron 43, measured 2026-10-13 with `scripts/measure-ram.sh`).
- Desired (not mandatory): <= 80 MB idle, <= 150 MB with a 20k-track list open. Measured at the end of every wave to catch regressions, but missing it does not block a wave.

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
