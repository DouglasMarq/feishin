<img src="assets/icons/icon.png" alt="logo" title="feishin" align="right" height="60px" width="60px" />

# Feishin

A native desktop music player for self-hosted music servers, written in Rust with a
[Slint](https://slint.dev) interface and [mpv](https://mpv.io) for playback. It is a rewrite of
the Electron [Feishin](https://github.com/jeffvli/feishin) that uses a fraction of its memory
(about 90 MB PSS with every view open, against about 530 MB for the Electron app).

Linux only for now.

## Features

- Navidrome, Jellyfin and OpenSubsonic/Subsonic servers (tested against Navidrome, gonic and
  Jellyfin), several servers per install, and import of the servers saved by the Electron app.
- Library: home, tracks, albums, artists, genres, folders, favorites, playlists (create, edit,
  reorder), global search, ratings and context menus. Huge libraries are paged from the server
  while you scroll.
- Playback through libmpv: gapless, crossfade, ReplayGain, output device, maximum bitrate and
  transcoding, sleep timer, full-screen now playing, play queue synced with the server.
- Synced and plain lyrics (server, lrclib, NetEase, Genius), scrobbling, MPRIS media keys,
  Discord Rich Presence, share links, internet radio (server stations and radio-browser.info).
- Discover: YouTube Music recommendations, new releases, charts and moods; songs in your
  library play from your server, others through `yt-dlp`.
- 32 themes with light/dark following the system and an accent color, 39 languages.
- The screen does not lock and the system does not sleep while music is playing.

## Install

Requirements: `mpv` (libmpv), a Wayland or X11 desktop and at least one font.

```sh
rust/scripts/install.sh              # builds and installs into ~/.local
rust/scripts/install.sh --uninstall
```

Arch Linux users can build the `feishin-native-git` package from `rust/packaging/arch`, and
`rust/packaging/appimage/build-appimage.sh` builds a portable AppImage. See
[rust/packaging/README.md](rust/packaging/README.md).

Settings live in `~/.config/feishin-rs`, passwords in the system keyring (Secret Service).

## Development

The Cargo workspace is in [`rust/`](rust). It needs a Rust toolchain, `libmpv`, `fontconfig`,
`xkbcommon` and `dbus` development files.

```sh
cd rust
cargo run -p feishin                         # the app
cargo run -p feishin -- --demo               # a generated library and silent audio, no server needed
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
scripts/live-servers.sh up && scripts/live-servers.sh test   # API tests against real servers in containers
```

Code standards are in [rust/CLAUDE.md](rust/CLAUDE.md) and the history of the rewrite in
[rust/MIGRATION.md](rust/MIGRATION.md).

## Translation

Translations are gettext catalogs in `rust/app/translations/<language>/LC_MESSAGES/feishin.po`.
After adding or changing text in the app, run `rust/scripts/translations.py` to add the new
strings to every catalog, then fill in the `msgstr` entries of your language. Strings that
existed in the Electron app reuse its community translations; the rest were machine translated
and corrections are welcome (`rust/scripts/translations.py check <language>` validates a catalog).

## License

[GNU General Public License v3.0](LICENSE)
