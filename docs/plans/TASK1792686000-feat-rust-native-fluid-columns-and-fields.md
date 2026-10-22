---
status: done
branch: feat/rust-native
ticket: chat request 2026-10-22 (track columns cut the rating stars; picker and search box look like stock widgets)
---

# Track columns fit the window, and pickers and search boxes match the app

## Context

Track list columns have fixed pixel widths (`TracksState.w-*` in `app/ui/globals.slint`, persisted as `ColumnSetting`). A narrow window cuts the rating column off; a wide one leaves an empty area on the right. The genre picker and the search box are the stock Fluent `ComboBox` / `LineEdit`, which do not match the pill-shaped `FilterChip` next to them.

Servers affected: none

## Assumptions

- Title, Artist and Album are the flexible columns; #, Year, Plays, Time, Favorite and Rating keep their width.
- The saved column widths keep their meaning as the preferred widths, so existing settings load unchanged.

## Out of scope

- Login and settings text fields (forms, not search boxes).
- Album detail rows, which have their own layout.

## Steps

- [x] 1. `TracksState` gets pure functions `shown-width(column, available)` and `resize(column, width, available)`: flexible columns share the width left after the fixed ones in proportion to their saved widths, with a 80px minimum (below it the list scrolls sideways as today); dragging a flexible column edge keeps the edge under the pointer → verify: headless test that the rating column ends inside the list at 900px and that title, artist and album grow at 2000px
- [x] 2. Tracks, Genres and Artist top songs pass their list width to the header and rows → verify: same tests on the genre view
- [x] 3. `TooltipComboBox` becomes a pill (30px, `radius 15px`, border, chevron) with its own popup list (scrolls past 10 items, keyboard up/down/enter/escape) → verify: headless test that picking an item sets `current-index` and fires `selected`
- [x] 4. New `SearchField` in `widgets.slint`: pill with a search icon, placeholder, clear button and focus border; replaces the search `LineEdit`s of Tracks, Albums, Artists, Genres, Search, Discover and the playlist picker filter → verify: headless test that typing fires `edited` and clear empties the text
- [x] 5. Translations for new tooltips ("Clear search"), fmt, clippy, tests, rebuild the Mac app

## Success criteria

- Tests: `given_a_narrow_window_the_rating_column_stays_visible`, `given_a_wide_window_the_text_columns_fill_it`, `given_a_dragged_title_edge_it_follows_the_pointer`, `given_a_picked_genre_the_picker_selects_it`, `given_a_cleared_search_the_text_is_empty`
- [x] `cargo fmt --all --check` passes
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [x] `cargo build --workspace` succeeds
- [x] `cargo test --workspace` passes
- [x] Every new or changed branch of logic has a test
- [x] `python3 scripts/translations.py check` reports every language complete

## Open questions

- None.

## Decisions / deviations

- The width math lives in `TracksState` (`shown-width`, `resize`) and is tested from Rust through the global's public functions; `HeaderCell` gained a `resized` callback whose default handler keeps the old fixed-width behaviour for Playlist detail and Search.
- The Discover country picker also moved to `TooltipComboBox`; the playlist picker's name field stays a form `LineEdit`.
- The tracks headless test now clicks inside the Title cell: with fluid columns x=400 can land on the Artist link.
- Added `given_the_tracks_page_opens_space_still_toggles_playback` so the focusable picker never steals the Space shortcut.
