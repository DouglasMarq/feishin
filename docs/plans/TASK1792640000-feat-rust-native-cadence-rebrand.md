---
status: done # draft | approved | in-progress | done
branch: feat/rust-native
ticket: user request (2026-10-22): rebrand the app as Cadence, with a new name, description and icon, plus a Remotion video for the README
---

# Rebrand the native app as Cadence: name, description, icon and a README animation

## Context

The native app still presents itself as "Feishin" everywhere a user looks: the window title, the sidebar and login logo, the macOS bundle, the Linux desktop entry and metainfo, the MPRIS identity, the README and the packaging names. The user picked the name **Cadence**. The scope is the **visible brand** only. They also asked for a **new icon** and a **Remotion video**, rendered to a GIF for the README, built from real screenshots of the app.

Servers affected: the Subsonic `c=` client name and the Jellyfin client name change from "Feishin" to "Cadence". Server admins see the new name. On Navidrome, per-player settings such as transcoding are keyed by client name, so they start fresh. The wiremock tests that assert the client name are updated.

## Assumptions

- **Stays as is**, to avoid migrations and broken data:
  - crate names (`feishin-*`);
  - the binary name `feishin`;
  - the app id `org.jeffvli.feishin.native`;
  - the config, data and cache folders (`feishin-rs`);
  - the mDNS service `_feishin-remote._tcp`;
  - the remote protocol;
  - the persisted `DiscordDisplayType::Feishin` serde name;
  - the Discord application (its name comes from the Discord client id registered upstream).
- Strings that refer to the **Electron Feishin** keep that name, because they describe the import source: "Import from Feishin", "Found {n} servers in Feishin…".
- Strings that refer to **this app** become "Cadence": remote control ("No other Cadence found…", "Let other Cadence players…", "a different version of Cadence") and the yt-dlp cookie note. Each catalog keeps its translation with the proper noun replaced, which is safe because it is not translated.
- Cadence is a fork of Feishin under GPL-3.0. The README keeps the license and credits Feishin and jeffvli prominently.
- **Icon**: an original mark. A white "C" made of a thick open arc, with three rhythm bars of growing height inside the opening, on a macOS-style rounded square filled with the Ambient gradients (indigo → magenta → amber). It is drawn once as SVG (`assets/icons/cadence.svg`), rendered to 1024 px with Remotion's `still`, and resized with `sips` to every size in `assets/icons`. The sidebar and login logo get the same mark as a Slint path.
- **Video**: a Remotion project in `docs/promo/`, kept out of the Rust workspace, with its own `package.json`. Remotion is free for individuals and companies of up to 3 people; larger companies need a license, which is noted in `docs/promo/README.md`.
  - Content: fresh `--demo` screenshots of the renamed app.
  - Length: about 12 s at 960 px wide and 15 fps, rendered to `docs/promo/out/cadence.gif`. It is copied to `assets/readme/cadence.gif` and kept under about 6 MB.
  - Scenes:
    - a logo intro;
    - Now Playing colored by the cover;
    - album page and cards;
    - Discover with the mixes;
    - Devices, controlling another Cadence;
    - light and dark themes;
    - a closing tagline.
  - Each scene uses a slow zoom and pan and a short caption.

## Out of scope

- Renaming crates, the binary, the app id or the config folders (the "full" option the user declined).
- A new Discord application.
- Repository or GitHub settings (repo name, description on GitHub). The user can rename the fork on GitHub.

## Steps

- [x] 1. **Name and description in code**:
  - `identity::APP_NAME`;
  - the window title (`app.slint`), the sidebar and login logo text;
  - the MPRIS identity;
  - the radio-browser user agent;
  - the Subsonic and Jellyfin client names, plus their tests;
  - the five app-referring translatable strings, plus `scripts/translations.py`, with all 38 catalogs updated by replacing the proper noun.

  → verify: `cargo test --workspace`, `translations.py check`.
- [x] 2. **Packaging**:
  - macOS: `Info.plist` (`CFBundleName` and `CFBundleDisplayName` set to Cadence), and `build-app.sh` producing `Cadence.app`, `Cadence.icns` and `Cadence-<version>-macos-<arch>.zip`;
  - Linux: the `.desktop` `Name`/`Comment`, the metainfo name, summary and description, the AppImage file name, the PKGBUILD `pkgdesc`, and the `install.sh` messages;
  - docs: `packaging/README.md`.

  → verify: `build-app.sh` produces `Cadence.app`, which opens and shows "Cadence" in the menu bar and Dock.
- [x] 3. **Icon**:
  - `assets/icons/cadence.svg`;
  - a Remotion `Icon` still rendered at 1024 px, then `sips` to 16…512 plus `icon.png`;
  - the Slint logo path (`Icons.cadence`) used in the sidebar and login.

  → verify: the PNGs look right at 16, 32 and 512 px (checked by eye), and the Dock icon after rebuilding the `.app`.
- [x] 4. **README and docs**:
  - a new title, a description ("Cadence is a fast, native music player for your self-hosted library…"), the GIF at the top, and a credits section ("Cadence is a fork of Feishin by jeffvli");
  - the name updated in `MIGRATION.md`, `CONTRIBUTING.md` and the `.claude` docs where they describe the product.

  → verify: the README renders with the GIF (preview in the browser pane).
- [x] 5. **Video**:
  - `docs/promo/` (Remotion + TypeScript);
  - fresh screenshots in `docs/promo/public/` (dark and light: Now Playing, album, home, Discover, Devices);
  - the scenes from Assumptions;
  - `npx remotion render` to GIF;
  - copied to `assets/readme/`.

  → verify: the GIF plays, is under 6 MB, and its frames are checked by eye.
- [x] 6. **Finish**: run fmt, clippy, build and test, `translations.py check`, then rebuild `Cadence.app` and send the GIF, the icon and the screenshots to the user.

## Success criteria

- Tests: the existing client-name tests assert "Cadence", and the remote and identity tests still pass.
- [x] No user-visible "Feishin" is left except where it names the Electron app (import) or the upstream credits. Checked with `grep` and listed in Decisions.
- [x] `cargo fmt --all --check` passes
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [x] `cargo build --workspace` succeeds
- [x] `cargo test --workspace` passes
- [x] `python3 scripts/translations.py check` reports every language complete

## Open questions

- Description wording (approved with the plan). Proposal: "Cadence is a fast, native music player for your self-hosted library (Navidrome, OpenSubsonic, Jellyfin), with YouTube Music discovery, personal mixes and player-to-player control."

## Decisions / deviations

- 2026-10-22: the user chose the name Cadence, the visible-brand scope, a README video built from real screenshots, and a new icon.
- 2026-10-22: the four app-referring strings were renamed in the source and in all 38 catalogs (msgid and msgstr, only the proper noun). Remaining user-visible "Feishin": "Import from Feishin", its tooltip and the "Found {n} servers in Feishin" plural (the Electron app as import source), plus the credits in the README and metainfo.
- 2026-10-22: the MPRIS bus name follows `APP_NAME`, so it is now `org.mpris.MediaPlayer2.cadence`; the metainfo `<provides>` was updated to match.
- 2026-10-22: Remotion's bundled headless Chrome hangs on "Waiting for root component to load" on this Mac, so renders pass `--browser-executable` with the local Chrome (documented in `docs/promo/README.md`).
- 2026-10-22: Remotion's own GIF was 37.8 MB, so the README GIF is encoded from an MP4 with ffmpeg (palette, Bayer dither, diff rectangles). The slow zoom and pan of every scene was replaced by a short entrance settle, since motion on every frame kept the GIF above 7 MB. Result: 960x600, 15 fps, 14.8 s, 4.8 MB.
- 2026-10-22: no Devices scene. A second demo instance shares the demo store (`$TMPDIR/feishin-rs-demo`), so it cannot be controlled from the first; player-to-player control is named in the closing tagline instead. Light and dark are shown as a wipe between the two Now Playing screenshots.
