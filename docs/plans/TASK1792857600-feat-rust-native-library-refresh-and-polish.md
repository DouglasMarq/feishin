---
status: done
branch: feat/rust-native
ticket: user request 2026-10-24 (no .scratch spec)
---

# Fix stale artist views and missing track headers, add panel shadows

## Context

Four reports from the user:

1. A song added to the library does not show inside its artist. `ArtistDetailState.load` goes through `artist_detail::current::begin`, which returns `None` when the id equals the artist already loaded, so reopening the same artist never refetches. The Artists list has a similar guard (`artists::install_load`, `loaded_for_server`) that loads only once per server. Album detail, Albums, Playlists, Folders and Genres refetch on every open. No server-side cache is involved (`fetch_artist_detail` hits the server every call; the subsonic list caches have a 60 s TTL).
2. Remote control: after a client is kicked (Disconnect) and reconnects, the host ends up with the client's song or paused state. Not found in code yet: a test with a real `Host` and `RemoteService` (`given_a_kicked_device_reconnects_the_speaker_keeps_its_music`) shows the host receives no control or transfer on reconnect. The difference with a first connection is that the host forgot the device, so the reconnect shows the approval dialog on the host.
3. Artist detail "Top songs" renders `TrackRowView` rows with no header at all. Genres/Favorites `TrackTable` has a header, but it always shows Artist/Album/Year/Plays/Time and never Favorite/Rating, while the rows follow `TracksState.is-*-shown`, so columns can misalign.
4. Add a shadow under the panels (sidebar, content, queue) so they float over the player glow.

Servers affected: none (UI and app-only changes).

## Assumptions

- "Inserir uma nova música" means adding a file to the server library (scan), not adding to a playlist.
- Reloading on every open is acceptable for Artists, as Albums already does it (views are recreated on route change, so scroll is lost anyway).

## Out of scope

- Refreshing the open view automatically when a library scan finishes (only Tracks does today).
- Server-side TTL caches.
- Item 2 (remote reconnect): postponed by the user on 2026-10-24.

## Steps

- [x] 1. `artist_detail::current::begin` starts a new generation even for the same id → verify: update `generations_guard_navigation`, new test `given_the_same_artist_is_opened_again_it_reloads`.
- [x] 2. Artists list reloads on every open (drop `loaded_for_server`) → verify: headless test `given_the_artists_page_is_opened_again_it_loads_the_artists_again`.
- [x] 3. `TracksHeader` gets `in property <bool> is-sortable: true`; artist Top songs shows a `TracksHeader { is-sortable: false; }`; `TrackTable` (genres, favorites) uses `TracksHeader` instead of its hand-written header → verify: headless test `given_an_artist_with_top_songs_its_track_columns_are_labelled`.
- [x] 4. `drop-shadow-*` with `Theme.shadow` on `Sidebar`, the content panel in `app.slint` and `QueuePanel` → verify: visual check by the user (the headless software renderer cannot show it and screen capture is not permitted here).

## Success criteria

- Tests: `given_the_same_artist_is_opened_again_it_reloads`, `given_the_artists_page_is_opened_again_it_loads_the_artists_again`, `given_an_artist_with_top_songs_its_track_columns_are_labelled`.
- [x] `cargo fmt --all --check` passes
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [x] `cargo build --workspace` succeeds
- [x] `cargo test --workspace` passes
- [x] Every new or changed branch of logic has a test
- [x] `python3 scripts/translations.py check` reports every language complete (no new strings expected)

## Open questions

- None.

## Decisions / deviations

- 2026-10-24: item 2 postponed; its exploratory reconnect test was reverted.
