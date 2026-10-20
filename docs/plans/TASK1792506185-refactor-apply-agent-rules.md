---
status: done # draft | approved | in-progress | done
branch: feat/rust-native
ticket: .scratch/apply-agent-rules/audit.md
---

# Bring the native app in line with the `.claude` rules found in the audit

## Context

The new `.claude/rules` (architecture, code-style, security, testing) and `rust/CLAUDE.md` were checked against `rust/` by a read-only audit (`.scratch/apply-agent-rules/audit.md`, item ids S/E/A/U/T/M/N/X). The code is already clean on comments, em dash, lint suppressions, crate direction and clippy. The gaps are security (credentials in error text, URLs built by concatenation, missing timeouts and bounds), errors that lose their cause or are swallowed, views doing I/O on the Slint thread, missing tooltips, stringly/int-typed state, magic numbers, and test hygiene (fixed sleeps, real data in fixtures).

Servers affected: Navidrome, Subsonic (gonic), Jellyfin - error mapping, URL building and timeouts change in every client. Live checks: `scripts/live-servers.sh up && scripts/live-servers.sh test`.

## Assumptions

- Behaviour stays identical except where a rule demands a visible change: failures now reach `AppState.status` (A11), new tooltips (U1-U3), credentials no longer appear in status/log text (S1).
- Persisted formats stay compatible: renamed `settings.toml` fields get `#[serde(alias)]`, lyrics sources and transcode format keep their current serde names, the remote protocol keeps its wire names.
- Each wave is implemented by a subagent in its own git worktree on its own branch, cut from the branch that already holds the previous phase. Waves in the same phase touch disjoint files (overlaps listed per wave). Branches are reviewed and merged into `feat/rust-native` at the end of each phase.
- Agents do not touch `app/translations`; new strings are extracted and translated once in the final step.

## Out of scope

- `crates/core` ids/timestamps newtypes, `Queue::new` entropy, `pub` wire helpers used only by tests, the player tracker boolean state machine (audit "Out of scope").
- Renaming existing test names.
- New features or behaviour beyond the audit items.

## Steps

### Phase 0 - base

- [x] 0. Commit and push `.claude/`, `docs/README.md`, `docs/plans/` and `.gitignore` on `feat/rust-native` so worktree agents see the rules → verify: `git ls-files .claude/rules` lists 4 files

### Phase 1 - parallel (4 agents)

- [x] 1. W1 security in crates: S1-S10, S12 (`without_url`, redacting `Debug`, `url` APIs for Jellyfin/Navidrome/Subsonic URLs and paths, timeouts on every client and lyrics request, pairing `random_bytes` → `Result`, bounded remote channels + pre-handshake frame limit, mDNS cap, genius host check + body caps, MPRIS art URL filter) → verify: new wiremock tests per client (`given_a_network_failure_the_error_has_no_credentials`, `given_an_id_with_reserved_characters_the_path_is_encoded`, `given_a_transcode_format_with_query_characters_it_is_encoded`), remote tests (`given_an_oversized_hello_rejects_the_connection`), live tests pass
- [x] 2. W2 tests: X1-X6 plus S11 (`TtlCache` entry cap, `tokio::time::Instant` in `ttl_cache.rs` and `playlist_cache.rs`) → verify: `cargo test --workspace`; `grep -rn "sleep(" ` only shows the sleeps the audit lists as unavoidable; no real artist/song/video ids left in the X4 files
- [x] 3. W3 app architecture: A1-A12, plus the app side of S3 (`app/src/covers.rs:40`, lyrics client with timeouts behind the injected provider) → verify: app tests for each moved responsibility (`given_an_external_track_lyrics_sources_come_from_the_settings_snapshot`, `given_a_logout_library_scan_polling_stops`, `given_a_failed_seek_the_status_shows_an_error`, `given_select_all_add_to_playlist_streams_in_chunks`); `grep -rn reqwest app/src/ui` is empty
- [x] 4. W4 UI: U1-U4 (tooltips on every clickable/draggable control, reuse `TabButton`, `tr!` placeholders instead of concatenation, server kind label function) → verify: headless test that the new tooltips exist where a view already has headless tests; `cargo build`; manual run `cargo run -p feishin -- --demo`
  - Overlap: W3 and W4 both live in `app/`; W4 touches only `.slint` files and the U4 `.rs` files, which W3 does not.

### Phase 2 - parallel (2 agents), after Phase 1 is merged

- [x] 5. W5 errors: E1 (`#[source]` / `#[from]` with `Arc` for `Clone` in every crate error enum and converter), E2 (14 swallowed errors), E3 (`ApiError::NotAuthorized` mapped in the clients, sharing/radio capability from the trait, views stop branching on `ServerType` and wire code `50`), plus MPRIS art from the cached local cover file (W1 now drops credential-bearing art URLs, so server covers vanished from the Linux media widget) → verify: unit tests per crate that `Error::source()` is the original error; wiremock tests `given_subsonic_error_50_returns_not_authorized` per client; app test that sharing/radio state comes from the trait; live tests pass
- [x] 6. W6 typed state: T1-T7 (Slint enums for discover page kind, sort column, repeat, login server type, ReplayGain, favorites/genres tabs; `LyricsSource` enum and transcode format type with unchanged serde names; mpv names from `ObservedProperty`; sleep timer end-of-track variant) → verify: settings round-trip tests that an old `settings.toml` with string lyrics sources and format still loads (`given_a_settings_file_from_the_previous_release_it_loads_unchanged`); headless tests of the affected views
  - Overlap: `crates/storage/src/settings.rs` (W5 touches the toml error at `:143-145`, W6 the fields) - different hunks.

### Phase 3 - one agent, after Phase 2 is merged

- [x] 7. W7 consistency: M1-M5 (named consts, units in names, shared helpers, `FakeAudio` to `demo/`, split `ui/player.rs`, `sleep_timer.rs`, `remote/service.rs`), plus N1 (`ctx` → `context`) and N2 (boolean prefixes, wire names kept with `#[serde(rename)]`, persisted names with `#[serde(alias)]`) → verify: `cargo test --workspace` unchanged results; `settings.toml` compat test for aliased fields

### Phase 4 - finish

- [x] 8. Run `python3 scripts/translations.py`, translate every new entry in the 39 catalogs → verify: `python3 scripts/translations.py check` reports every language complete
- [x] 9. Full checks, live tests, RAM → verify: success criteria below; `scripts/measure-ram.sh` with `--demo --demo-songs 100000` (covers and caches changed) compared to `feat/rust-native`

## Success criteria

- Tests: the Given-It cases named in each step, plus a reproducing test for each security fix (S1, S2, S4, S6).
- [x] `cargo fmt --all --check` passes
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [x] `cargo build --workspace` succeeds
- [x] `cargo test --workspace` passes
- [x] Every new or changed branch of logic has a test
- [x] `python3 scripts/translations.py check` reports every language complete
- [x] Tests follow Given-It naming and cover the happy path, empty lists and missing items, invalid input and boundaries (paging edges, unpaired or non-local remote devices)

## Open questions

- None.

## Decisions / deviations

- 2026-10-20: plan written from the audit; waves split by file ownership so Phase 1 and Phase 2 agents can run in parallel.
- 2026-10-20: approved by the owner: full scope, N1 and N2 renames included in W7, work lands on `feat/rust-native` (no separate refactor branch), agent branches merged into it.
- 2026-10-20: W1 left Subsonic URL building on `encode_uri_component` (already encodes every value; `url` form encoding would change `%20` to `+`). MPRIS art URLs with credentials are dropped; restoring covers through the local cache file moved to W5.
- 2026-10-20: Phase 1 merged (W1-W4). W2 renamed the fuzzy lyrics cases by letter substitution to keep the fuse.js reference scores; real artists remain in recorded YouTube fixtures outside the audit lines. W3 keeps the settings snapshot in `Session` and reaches the lyrics provider through `Connector`. `crossfade::pausing_during_a_fade_freezes_it` is flaky under heavy load (pre-existing, left for a follow-up).
- 2026-10-20: Phase 2 merged (W5, W6), plus two side fixes: the mixer now reports pause changes it makes itself (the crossfade flake was a player bug), and `scripts/live-servers.sh` runs on macOS bash 3.2 without `--test-threads=1`. W5 kept `can_share()` and `Unsupported` for radio instead of new trait methods; MPRIS art falls back to the cached cover file. W6 chose a `TranscodeFormat` newtype (custom formats keep working). Phase 3 splits W7 into W7a (crates) and W7b (app) in parallel, then W7c renames (N1, N2) alone.
- 2026-10-20: W7a (crates) and W7b (app) merged. Persisted settings fields renamed with `#[serde(rename)]` so `settings.toml` keeps its keys; `lru`/`rand` moved to `[workspace.dependencies]`. RAM (macOS `footprint`, `--demo-songs 100000`, idle): 68 MB before and after; `scripts/measure-ram.sh` reads `/proc` and does not run on macOS. Translations (step 8) run in parallel with W7c since the catalogs carry no source locations.
- 2026-10-20: W7c renamed `ctx`/`rt` and every unprefixed boolean (crates, app, Slint); wire and persisted names kept with `#[serde(rename)]` and covered by literal old-file tests; a few intent flags use `should_` where no is/has/can name reads naturally. Translations: 44 new entries in all 38 catalogs (the script lists 38 languages, not 39), one obsolete entry dropped. Final: fmt, clippy, build clean; 1194 tests pass; live tests 328/328 on the W7c branch (the merge after it only added catalogs); translations 586/586. The UI was smoke-run in `--demo` without errors but not inspected visually.
