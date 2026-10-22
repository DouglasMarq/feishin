---
status: in-progress
branch: feat/rust-native
ticket: .scratch/tray/spec.md
---

# System tray icon

## Context

The Electron app had a tray icon with playback controls, "minimize to tray" and "close to tray". The native app has none: `app.rs` runs `window.run()`, so closing the window quits.

Servers affected: none

## Assumptions

- Linux uses StatusNotifierItem over D-Bus (KDE, XFCE, Cinnamon show it natively; GNOME needs the AppIndicator extension, as for every tray app).
- macOS uses an `NSStatusItem` on the main thread, next to the winit event loop Slint already runs there.

## Dependencies (to justify)

- Linux: `ksni` (pure Rust StatusNotifierItem on D-Bus, no GTK; Unlicense, GPL-compatible). `tray-icon` was rejected: on Linux it needs GTK and libappindicator and its own GTK main loop.
- macOS: `tray-icon` (tauri-apps, MIT/Apache-2.0) with `muda` for the menu, both already maintained for winit apps.

## Out of scope

- Windows; cover art, rating or volume in the tray.

## Steps

- [x] 1. `storage::settings::WindowSettings { is_tray_enabled: true, is_minimize_to_tray: false, is_close_to_tray: false }` with `#[serde(default)]` → verify: an old `settings.toml` without the section loads with the defaults
- [x] 2. `crates/platform::tray`: a `Tray` with `TrayAction { PlayPause, Next, Previous, Stop, Show, Quit }` sent on a channel and `set_tooltip`; `ksni` on Linux, `tray-icon` on macOS → verify: unit test that menu items map to actions; Linux test that the SNI service registers on a private D-Bus session (like the MPRIS tests)
- [x] 3. App wiring: actions become controller `Command`s or window show/hide/quit; the tooltip follows the current song; the event loop runs with `run_event_loop_until_quit` and `on_close_requested` hides the window when close to tray is on → verify: headless test that close hides instead of quitting with the setting on, and quits with it off
- [x] 4. Settings > Window section with three switches (tooltips, translations) → verify: headless settings test toggles each and a `SaveSettings` command carries it
- [ ] 5. Translations, fmt, clippy, tests, rebuild the Mac app (done, icon shows in the menu bar), try it on Linux (GLaDOS)

## Success criteria

- [x] `cargo fmt --all --check` passes
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [x] `cargo build --workspace` succeeds
- [x] `cargo test --workspace` passes
- [x] Every new or changed branch of logic has a test
- [x] `python3 scripts/translations.py check` reports every language complete
- [ ] RAM idle still measured with `scripts/measure-ram.sh` (tray must not add a GTK runtime)

## Open questions

- None. "Close to tray" defaults to off, as in the Electron app.

## Decisions / deviations

- One crate on both systems: `tray-icon` with its `ksni` backend on Linux (0.25, StatusNotifierItem through ksni's blocking mode, no GTK) and 0.24 on macOS, because 0.25 pulls muda 0.20 whose Objective-C classes clash at link time with the muda 0.19 Slint already uses.
- Menu clicks reuse `MediaControlEvent` (Toggle, Next, Previous, Stop, Raise, Quit) and go through the same `Command::Media` path as MPRIS; Linux left click raises the window.
- The tooltip is the static app name, as in the Electron app; a live song tooltip would need tray state shared with the controller thread.
- "Minimize to tray" dropped: Wayland does not tell clients their window was minimized, so it cannot work on most Linux desktops.
- Settings live in `[ui.tray]` (`enabled`, `close_to_tray`) and survive shutdown like the language. The switches are in Settings > Interface; "Close to the tray" only shows while the tray is on.
- The window runs with `run_event_loop_until_quit`; closing quits unless close to tray is on and the icon exists.
- Icons: `crates/platform/assets/tray-template.png` (the Cadence glyph rasterized from `Icons.cadence`, used as a macOS template image) and `tray-color.png` (the app icon at 64 px for Linux).
- The Linux app build could not be cross-checked from macOS (aws-lc-sys needs a Linux C toolchain); `feishin-platform` passes clippy for `x86_64-unknown-linux-gnu`, CI covers the rest.
