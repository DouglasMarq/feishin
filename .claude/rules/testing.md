---
paths:
  - "rust/**/tests/**"
  - "rust/**/tests.rs"
  - "rust/**/*_tests.rs"
  - "rust/**/test_*.rs"
---

## Stack

- The built-in Rust test harness: `#[test]`, `#[tokio::test]` for async code (`tokio` `test-util` in the app for paused time).
- `wiremock` for server clients and HTTP code: each `api-*` crate's `tests/server/` (or `tests/navidrome/`) starts a mock server per test and asserts the requests the client sends and how it maps the responses.
- `i-slint-backend-testing` and `app/src/ui/test_window.rs` for headless UI tests (`headless_tests.rs`, `ui/<view>/tests.rs`): a software-rendered window driven without a display. Only one test can own the Slint event loop proxy at a time; a test that cannot get it prints a skip line and returns.
- App-level tests (`app/src/tests/`) drive the real `Controller` through `harness.rs`: `FakeServer` and `test_song` from `demo`, `RecordingAudio` instead of mpv, `NoMediaSession`, a `JsonStore` in a temp directory.
- Live tests (`crates/live-tests`) run the same checks against real Navidrome (latest, 0.54, 0.56), gonic and Jellyfin (latest, 10.10) containers on a generated library. They are `#[ignore]`d and run only through `scripts/live-servers.sh up && scripts/live-servers.sh test`.
- Integration tests in `crates/platform/tests` (D-Bus, Discord IPC) and `crates/player/tests` (libmpv) run in CI, which installs libmpv and D-Bus. A test that needs something CI cannot provide is `#[ignore]`d with the reason in the attribute, as `crates/storage/tests/real_keyring.rs` (`#[ignore = "touches the real system keyring ..."]`).
- `unwrap`, `expect`, `panic!` and indexing are allowed in tests (`rust/clippy.toml`).

## Naming

- !IMPORTANT: Use BDD Given-It style - each new test reads as a sentence: `given_<condition>_<observable_outcome>`, e.g. `given_shuffle_off_restores_the_original_order()`, `given_an_unknown_device_rejects_the_connection()`.
- Existing sentence-named tests (`quitting_while_a_list_streams_in_saves_the_whole_list`) are not renamed.
- Describe the observable behavior, not the implementation step.
- Group the tests of one concern in a module (`queue/tests/insertion.rs`, `tests/server/playlists.rs`).

## Test levels

| Level | What | How |
| ----- | ---- | --- |
| Domain | queue operations, shuffle, model mapping | Plain `#[test]` in `crates/core`, no I/O |
| Server client | requests sent, response mapping, paging, caching, auth | `#[tokio::test]` + `wiremock` in the client crate's `tests/` |
| Infrastructure | file formats, keyring, mpv, D-Bus, mDNS/TCP remote | Crate tests with temp dirs; real system tests `#[ignore]`d |
| Controller | playback, queue, persistence, streaming lists, remote | `app/src/tests/` with `harness.rs` fakes |
| UI | a view renders and reacts to input | Headless Slint tests |
| Live | the client against real servers | `crates/live-tests` + `scripts/live-servers.sh` |

## Data

- Each test creates its own data and temp directories (`tempfile`). NEVER share mutable data between tests or depend on files left by another test.
- Create test data with the existing builders and fakes (`demo::test_song`, `FakeServer`, the `support.rs` of each test folder), never with the code under test.
- Use fictitious values only (see `security.md`): made-up server URLs (`http://navidrome.example`), users (`feishin`), passwords and song metadata.
- Server responses in `wiremock` tests follow the shape real servers return; when unsure, check it against a live server instead of inventing fields.

## Dependencies

- Prefer real dependencies (temp files, real codecs) over mocks.
  Fake only what the test cannot control: servers (`wiremock`, `FakeServer`), mpv (`RecordingAudio`), `yt-dlp` (`FakeYtDlp`), the system keyring, D-Bus, clocks and randomness.

## Coverage of behavior

- Every bug fix comes with a test that reproduces the bug.
- Every new or changed branch of logic has a test.
- Cover the happy path, empty lists and missing items, invalid input (malformed server responses, corrupted files on disk) and boundaries (paging edges, remote control from an unpaired device or outside the local network).
- A change to a server client also passes its live checks, or the report says they were not run.

## Anti-patterns

- NEVER disable, `#[ignore]`, skip or weaken a failing test to make the suite pass.
- NEVER depend on test execution order.
- NEVER wait with a fixed `sleep` when a channel, a `oneshot` or paused tokio time can signal the event.
