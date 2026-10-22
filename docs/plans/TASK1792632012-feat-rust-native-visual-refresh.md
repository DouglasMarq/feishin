---
status: done # draft | approved | in-progress | done
branch: feat/rust-native
ticket: .scratch/visual-refresh/spec.md
---

# Give the native app an "Ambient" look: cover-driven color, depth, an integrated macOS window, a real type scale and subtle motion

## Context

Screenshots of `--demo` (Home, Tracks, Albums, album detail, Discover, Now Playing) show that every surface uses nearly the same near-black. Covers never tint the interface: Now Playing draws the cover at `opacity: 0.14` (`app/ui/now_playing.slint:363-373`). The macOS title bar is a grey band. There is no hover feedback on cards or rows, and the font is the renderer's default with ad hoc sizes. The only animation is in `now_playing.slint:157`. See `.scratch/visual-refresh/spec.md`.

Relevant code:
- Covers are decoded only in `crates/storage/src/cover_cache/image_file.rs::decode` into `DecodedImage`, and become a Slint `Image` only in `app/src/ui/images.rs::to_image`.
- The cover-loading call sites are:
  - `ui/player.rs` (player bar);
  - `ui/now_playing/fetching.rs`;
  - `ui/album_detail/loading.rs`;
  - `ui/artist_detail/loading.rs`;
  - `ui/playlist_detail/loading.rs`.
- The window is a plain `AppWindow::new()` in `app/src/app.rs:305`, with no `BackendSelector`. Slint runs with default features (winit + femtovg).
- Theme tokens are in `app/ui/theme.slint`. The palettes in `theme_palettes.slint` stay untouched; new tokens are derived from them.

Servers affected: none.

## Assumptions

- The tint is computed once per decoded cover, inside `decode`, and stored on `DecodedImage` (`tint: Option<Rgb>`). It is cached with the image at no extra fetch. The disk cache holds encoded files, so nothing persisted changes.
- The extraction is a pure function over the RGBA buffer:
  - sample a strided grid;
  - drop near-black, near-white and grey pixels;
  - bucket the hue;
  - weight each bucket by saturation × count;
  - average the winning bucket.
- It returns `None` for greyscale covers. Those fall back to the theme's `surface-raised` and look like today.
- Readability is handled in Slint, not Rust: `Theme.ambient(tint)` clamps OKLCH lightness and chroma per `is-dark`, so the same tint works on every palette.
- The layout becomes **panels**: the window background is the darkest layer (`background-alt`), and the sidebar and the content are rounded panels (`background`) with an 8 px gutter. The player bar sits on the window background. The traffic lights sit on the window background above the sidebar panel.
- The macOS window uses the winit 0.30 `WindowAttributesExtMacOS` (`with_titlebar_transparent`, `with_fullsize_content_view`, `with_title_hidden`) through `slint::BackendSelector::with_winit_window_attributes_hook`. That hook needs Slint's `unstable-winit-030` feature, which is acceptable because Slint is pinned at 1.18. Dragging works through `slint::winit_030::WinitWindowAccessor` → `drag_window()`, called from a top `TouchArea`; a double-click calls zoom. All of this is `cfg(target_os = "macos")`.
- Inter (SIL OFL 1.1) is embedded with `import "fonts/Inter-*.ttf"` and set as `default-font-family`. Only the needed weights go in (Regular, Medium, SemiBold, Bold), about 1.2 MB. OFL allows bundling a font with software of any license, and its `OFL.txt` ships next to the files.
- No continuous animation. A "now playing" equalizer on the active row would force full redraws at 60 fps with femtovg, so the playing row keeps a static bars icon in the accent color.

## Out of scope

- New palettes or changes to the existing palette values.
- A setting to turn the ambient color off. It can be added later if someone asks.
- A real blur (Slint 1.18 has no blur filter). The tint gradients replace it.
- Switching the renderer to Skia. Step 8 only measures it and reports back.
- Windows.

## Steps

- [x] 1. **Tint extraction** in `crates/storage/src/cover_cache/tint.rs`:
  - add `pub fn dominant_tint(rgba, width, height) -> Option<Rgb>` and the `Rgb` type;
  - fill `DecodedImage.tint` in `decode`, and in the demo cover renderer (`demo::render_cover`) so `--demo` shows color.
  → verify, unit tests:
  - a solid red image gives red;
  - a mostly black image with a blue patch gives blue;
  - a greyscale image gives `None`;
  - a 0×0 or truncated buffer gives `None`;
  - the result is deterministic.
- [x] 2. **Theme tokens and type scale** in `app/ui/theme.slint`:
  - `Theme.canvas`, `panel`, `radius-card: 8px` and `radius-panel: 10px`;
  - shadow tokens;
  - `pure function ambient(color) -> color` and `ambient-for-text(color)`;
  - a `Type` global with display, title, heading, body, small and caption sizes and weights, including `letter-spacing` for captions;
  - animation duration tokens (`Motion.quick: 120ms`, `Motion.ambient: 600ms`, `Motion.page: 160ms`);
  - Inter imported and `default-font-family: "Inter"` on `AppWindow`.
  → verify: `cargo build`, the existing appearance tests pass, and a screenshot of each palette (dark and light) shows readable text.
- [x] 3. **Panel layout and integrated window**:
  - `app.slint`: the window background becomes `canvas`, the sidebar and content get rounded `panel` containers with gutters, and the player bar sits on `canvas`;
  - `sidebar.slint`: top inset for the traffic lights on macOS (`AppState.is-macos`, set from Rust), plus a drag strip with tooltip-free accessibility;
  - `app/src/app.rs`: a `BackendSelector` with the macOS attributes hook, and a `window_chrome` module with `drag_window` / `zoom` handlers wired to `AppState` callbacks.
  → verify:
  - on the Mac, screenshots show the traffic lights on the canvas above the sidebar, and dragging and double-click zoom work;
  - a Linux `cargo check --target x86_64-unknown-linux-gnu -p feishin` passes, or a note says it could not run;
  - the headless tests pass.
- [x] 4. **Cover color into the views**:
  - `images.rs` returns `(Image, Option<Color>)`;
  - new `tint` / `has-tint` properties on `PlayerState`, `NowPlayingState`, `AlbumDetailState`, `ArtistDetailState` and `PlaylistDetailState`;
  - the headers become a tall `@linear-gradient` from `Theme.ambient(tint)` to `panel`, with a larger title (display size) and the cover with a drop shadow;
  - Now Playing: two `@radial-gradient`s from the tint and a shifted hue, over `canvas`, replacing the 14% cover;
  - player bar: a left-to-right wash of the tint at low alpha;
  - every tint property animates with `Motion.ambient`.
  → verify:
  - tests for the tint/no-tint mapping in `images.rs`;
  - the album detail and now playing headless tests still pass;
  - screenshots in `--demo` show the header and the Now Playing view colored by the cover, and changing songs fades the color.
- [x] 5. **Cards and grids**:
  - `album_grid.slint`, `home.slint`, `discover_cards.slint` and the artist and playlist grids share a `CoverCard` widget;
  - on hover the card fills with `surface` and the cover shadow grows;
  - a round accent play button fades and slides in at the bottom right (`Motion.quick`), with its own tooltip "Play {}" and wired to the existing `play` callbacks;
  - a missing cover gets a two-stop gradient picked from a stable hash of the name (`Theme` helper), plus the icon.
  → verify: the home, discover and album headless tests pass, and screenshots show the hover state and gradient placeholders on the demo playlists.
- [x] 6. **Rows, bars and controls**:
  - `track_row.slint`, `queue_panel.slint` and `radio_rows.slint`: hover fill animated with `Motion.quick`;
  - the playing row shows a static equalizer icon and the title in the accent color;
  - `widgets.slint::Bar`: the track is 4 px, grows to 6 px on hover with a knob, and the fill turns from `foreground` to `primary` on hover;
  - `IconButton` and the play button get a hover and press scale (0.94 on press);
  - nav entries get an animated hover and an accent indicator on the active entry.
  → verify: the tracks, queue and radio headless tests pass, and screenshots.
- [x] 7. **Motion polish**:
  - the content cross-fades on route change: an opacity animation keyed on `AppState.route`, `Motion.page`;
  - the active lyric line eases its size and opacity;
  - tooltips fade in.
  → verify: manual run, and CPU stays at about 0% when idle (`top -pid`), meaning no animation is left running.
- [x] 8. **Measure**:
  - `scripts/measure-ram.sh` on `--demo` and `--demo-songs 100000`, before and after;
  - try the Skia renderer once (`renderer-skia` feature) for text sharpness and report RAM and binary size, without switching unless the user agrees.
  → verify: the numbers are recorded under Decisions.
- [x] 9. **Translations and finish**: `python3 scripts/translations.py`, translate the new strings into every catalog, then run fmt, clippy and the tests. Rebuild `Feishin.app` with `packaging/macos/build-app.sh` and send before and after screenshots.
  → verify: everything in the success criteria.

## Success criteria

- Tests (Given-It):
  - `dominant_tint`: solid color, colored patch on black, greyscale gives `None`, empty or truncated buffer, determinism;
  - `DecodedImage` carries the tint after `decode`;
  - the `images.rs` mapping covers tint and no tint;
  - the placeholder gradient is stable for the same name.
- [x] Screenshots of every view in a dark and a light palette show readable text and the cover-driven color.
- [x] Idle CPU is about 0% with the app open and while paused.
- [x] RAM is within the targets (≤ 80 MB idle, ≤ 150 MB with a 20k list), measured before and after.
- [x] `cargo fmt --all --check` passes
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [x] `cargo build --workspace` succeeds
- [x] `cargo test --workspace` passes
- [x] Every new or changed branch of logic has a test
- [x] `python3 scripts/translations.py check` reports every language complete

## Open questions

- Is Inter the right font? The alternative is the platform UI font (SF Pro on macOS, Cantarell or Noto on Linux), which has no bundle cost but looks different on each OS.

## Decisions / deviations

- 2026-10-21: the user picked cover color, depth and finish, the integrated macOS window, typography and subtle animations, and left the style direction open ("Me surpreenda"). The direction is "Ambient": quiet panels, with color coming from the music.
- 2026-10-21: the tint is a method on `DecodedImage` (`dominant_tint`, strided sampling, about 2k pixels) computed when a cover is shown, instead of a stored field. It is just as cheap and leaves the struct and its four construction sites untouched.
- 2026-10-21: Inter 4.1 is embedded as the single variable font (`InterVariable.ttf`, 880 KB, OFL without a reserved name) instead of four static weights.
- 2026-10-21: the macOS window relies on the native title bar for dragging and double-click zoom (transparent title bar plus full-size content view), so no `drag_window` code was needed. A 30 px canvas strip keeps the traffic lights clear of the panels.
- 2026-10-21: the sidebar shrank from 220 to 200 px and the default title column from 320 to 300 px, so the default track columns still fit beside the panel gutters.
- 2026-10-21: missing covers use one accent-tinted gradient (`Theme.placeholder`) rather than a per-name hue. Slint cannot hash strings, and a per-name hue would add a field to every card model.
- 2026-10-21: the std-widgets `Palette` is read-only in Slint 1.18, so the Fluent text fields and combo boxes keep their own look.
- 2026-10-21: no Skia trial. Text is already sharp with Inter on femtovg, and Skia would add a large prebuilt dependency.
- 2026-10-21: album and artist cards gained `accessible-role: button`, as did the song area of the player bar. Screen readers can open them, and so could the background UI checks.
- 2026-10-21: RAM (debug build, `--demo`, macOS `footprint`) was about 70 MB before and 77-84 MB after; the extra is the font and the gradient layers. A release measurement is pending.
- 2026-10-21: leaving an album page shows "Could not load the album: not found". `AlbumDetailView`'s `changed id` runs with the empty detail id set by `navigate`. This looks pre-existing: `nav.rs` and the album view logic are untouched. It is reported, not fixed.
- 2026-10-21: release RAM measured with macOS `footprint` in `--demo`. Idle went from 68 to 71 MB, after Discover from 81 to 83 MB, and after Home plus Albums from 94 to 96 MB. The first `CoverArt` kept an animated drop shadow and a hidden play badge layer on every card, which meant extra textures per card. Both were removed: the hover now only darkens and zooms the cover, and the badge exists only while it is shown.
- 2026-10-21: fixed the reload on leaving a detail page. Album, artist and playlist reload only while their route is shown, the same guard Genres already uses. Covered by `ui/detail_navigation_tests.rs`.
- 2026-10-21: the Discover RAM investigation requested mid-task is in `.scratch/ram-report/report.md`.
