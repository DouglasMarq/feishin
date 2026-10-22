---
status: done
branch: feat/rust-native
ticket: .scratch/remote-hardening/spec.md
---

# Harden remote control before users open its firewall port

## Context

Remote control moved to the fixed TCP port 47390 so it can be opened in a firewall. The security review in `.scratch/remote-hardening/spec.md` found a spoofable approval prompt, unbounded connections before pairing and trusted remote thumbnail URLs. Code: `crates/remote/src/host.rs`, `protocol.rs`, `client.rs`, `discovery.rs`, `app/src/remote/{approval,transfer,link}.rs`, `app/ui/devices.slint`.

Servers affected: none

## Assumptions

- The approval approved by the user explicitly in chat ("fix the items") counts as plan approval.
- Rebuilding thumbnails from the video id is acceptable even though YouTube Music square art becomes the video frame for tracks received from another device.

## Out of scope

- Encryption or a key exchange at pairing.
- Image decode limits for server covers.

## Steps

- [x] 1. Clean device names from the network (`DeviceInfo::cleaned`) in the host, the client and discovery → verify: `given_a_name_with_control_characters_...` tests in `protocol.rs`
- [x] 2. Pass the peer address to `HostDelegate::approve` and show it in the dialog → verify: `remote::tests`, translations check
- [x] 3. Cool down an address for 2 minutes after a denied or unanswered approval (`refusals.rs`, bounded) → verify: `refusals` unit tests + host test that a denied peer is not asked again
- [x] 4. Cap handshakes at 8 with a semaphore → verify: host test that the 9th connection is closed without a challenge
- [x] 5. Rebuild YouTube thumbnails of tracks from another device from the video id (host adapt + client mirror) → verify: `transfer.rs` and `link.rs` tests
- [x] 6. README firewall commands scoped to the local subnet → verify: read
- [x] 7. fmt, clippy, workspace tests, translations check, rebuild the Mac app

## Success criteria

- Tests: `given_a_name_with_control_characters_they_are_removed`, `given_an_empty_name_the_id_is_shown`, `given_a_refused_address_it_cools_down_until_the_cooldown_ends`, `given_a_refused_address_it_is_not_asked_again`, `given_more_handshakes_than_the_cap_the_extra_connection_is_closed`, `given_a_track_from_another_device_its_thumbnail_comes_from_the_video_id`
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

- 2026-10-22: thumbnails from another device are always rebuilt from the video id instead of allowing YouTube image hosts, because `*.googleusercontent.com` and `yt3.ggpht.com` serve user-uploaded images and would still allow cache poisoning.
