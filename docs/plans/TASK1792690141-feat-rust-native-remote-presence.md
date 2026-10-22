---
status: in-progress
branch: feat/rust-native
ticket: .scratch/remote-presence/spec.md
---

# Remote control can skip the approval prompt, and the controlled player shows, disconnects and bans who controls it

## Context

Remote control works both ways, but every new device needs an approval click on the controlled player, and the controlled player gives no sign that another device is driving it (the controlling player shows its target in the Devices button). See the spec.

Servers affected: none

## Assumptions

- "Allow without asking" only replaces the prompt: the LAN-only check (`is_lan_peer`), the handshake cap and the per-device pairing token stay, so a device that was let in once reconnects with its token like an approved one.
- "Being controlled" means a remote control session is open on this player after the handshake, from `Host::run_session` until it returns.
- A ban matches the device id and the address it connected from. Both come from the network, so a hostile device on the LAN can dodge a ban by changing them; the ban is for a known device, while the approval prompt stays the real gate.

## Out of scope

- Encryption.

## Steps

- [x] 1. `storage::settings::IntegrationSettings` gets `is_remote_auto_approved` (`remote_control_without_asking`, `#[serde(default)]`, false) → verify: an old `settings.toml` loads with it off; a saved one loads back
- [x] 2. `Approvals` holds the flag (`Arc<AtomicBool>`); `ask` returns true without showing the prompt while it is on; the settings callback updates it and saves → verify: tests that `ask` approves without a prompt when on, and still waits for an answer when off
- [x] 3. `crates/remote`: `HostDelegate` gets `controllers_changed(&[DeviceInfo])`; `Host` keeps the open sessions (added after the handshake, removed when the session ends, also on error) → verify: crate tests that a connected client is reported and is removed after it disconnects, and that a refused client is never reported
- [x] 4. App: `AppHostDelegate::controllers_changed` publishes to `DevicesState.controllers` (names) on the UI thread; the Devices button turns `is-active` with tooltip "Controlled by {names}"; the devices popup shows a "Controlled by" line → verify: headless test that the button tooltip and popup show the controller and clear when the list empties
- [x] 5. Disconnect and ban: `Host` keeps a cancel handle per open session; `Host::disconnect(device)` ends the session and the trust store forgets the device's token; `Host::ban(device, address)` does the same and adds it to a ban list in `remote-devices.json` (`#[serde(default)]`, so older files load). The handshake refuses a banned id or address with `Denied` before any prompt or auto-approval → verify: crate tests that a disconnected client loses its session and must pair again, that a banned client is refused without `approve` being called (also with auto-approval), and that an unbanned one is asked again
- [x] 6. Devices popup on the controlled player: one row per controller with "Disconnect" and "Ban" actions (tooltips); Settings > Integrations: "Banned devices" list (name, address, "Remove") shown when not empty → verify: headless tests that the actions send the right request and that removing a ban clears the row; an old `remote-devices.json` loads with an empty ban list
- [x] 7. Settings UI: "Allow without asking" switch below "Remote control" (visible only while it is on) with a description that warns any device on the network can then control this player; update the "Remote control" description; update `.claude/rules/security.md` (approval can be turned off by the user, off by default) → verify: headless settings test toggles it and the setting is saved
- [x] 8. Tray menu: a "Connected devices (N)" line with a submenu listing each controller ("name (address)"), shown only while at least one device controls this player (so never while remote control is off) → verify: platform unit test for the label and items; app test that the tray host gets the list from `show_controllers` and drops the line when it empties
- [ ] 9. Translations, fmt, clippy, tests, rebuild and reinstall the Mac app; try both directions with GLaDOS

## Success criteria

- Tests: `given_allow_without_asking_a_new_device_is_paired_without_a_prompt`, `given_allow_without_asking_is_off_the_prompt_still_decides`, `given_a_client_connects_the_host_reports_it_as_a_controller`, `given_the_client_disconnects_it_is_no_longer_reported`, `given_a_refused_client_it_is_never_reported`, `given_a_controller_the_devices_button_shows_it`, `given_a_disconnected_device_it_must_pair_again`, `given_a_banned_device_it_is_refused_without_a_prompt`, `given_a_banned_device_with_allow_without_asking_it_is_still_refused`, `given_a_removed_ban_the_device_is_asked_again`, `given_an_old_device_file_it_loads_with_no_bans`
- [x] `cargo fmt --all --check` passes
- [x] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [x] `cargo build --workspace` succeeds
- [x] `cargo test --workspace` passes
- [x] Every new or changed branch of logic has a test
- [x] `python3 scripts/translations.py check` reports every language complete
- [x] Tests follow Given-It naming and cover the happy path, empty lists and missing items, invalid input and boundaries (paging edges, unpaired or non-local remote devices)

## Open questions

- None blocking.

## Decisions / deviations

- 2026-10-22: the user added disconnect, ban and a removable ban list, then a tray line with the connected devices and a counter (steps 5, 6, 8).
- Disconnect forgets the device's approval, so it is asked again (or let in at once with "Allow without asking"). Ban only applies to a connected device; the ban matches its id or its address.
- Turning remote control off now also closes the open sessions (`Host::disconnect_all`); before, a connected device kept its session until it left.
- The tray menu is rebuilt on every change: the Linux ksni backend takes a snapshot of the menu, so editing the live menu would not reach the panel. Device entries are informational (disabled).
- The tray learns about controllers through a `DevicesState.controllers-shown` callback that `show_controllers` invokes, so the tray host stays on the UI thread without shared state.
- `TestWindow::open_with_remote` lets headless tests observe `RemoteRequest`s.
- Visual check of the controlled side needs two players; left for the GLaDOS test.
