# Contributing to Feishin

Thanks for your interest in contributing!

Check the existing [issues](https://github.com/jeffvli/feishin/issues).

## IMPORTANT

For feature contributions that do not have an existing feature request, please open an issue, discussion, or ask [Feishin discord server](https://discord.gg/FVKpcMDy5f) **BEFORE** beginning to implement it to determine whether or not the feature would be acceptable for the app. Simply submitting a large feature with thousands of new or changed lines of code without asking if the feature is desired for the application will run the risk of instant closure.

Bug fixes are likely to be accepted without discussion.

## Development environment

Feishin is a Rust workspace in `rust/` (Slint UI, libmpv playback). You need a stable Rust toolchain and the `libmpv`, `fontconfig`, `xkbcommon` and `dbus` development files.

- `cargo run -p feishin` - run the app
- `cargo run -p feishin -- --demo` - run against a generated library, without a server
- `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` - must pass before a pull request
- `scripts/live-servers.sh up` then `scripts/live-servers.sh test` - API tests against Navidrome, gonic and Jellyfin containers

Code standards are in `rust/CLAUDE.md`.

## AI Disclosure

AI-generated code **is accepted** under the following conditions:

1. You are a professional or hobbyist developer using agentic coding as a tool rather than a crutch
2. You are manually reviewing the output code before submitting the pull-request
3. You are manually testing the functionality of your code before submitting the pull-request

Pull requests are **manually reviewed** by maintainers which means that submitting any form of slop wastes our precious time that could otherwise go towards fixing bugs or implementing new features.

When communicating with maintainers or other users in issues, discussions, or pull requests, DO NOT respond to others with solely AI generated text. I and other maintainers spend REAL and VALUABLE time maintaining this project, so communicating in your own words is the bare minimum that I ask of you.

## Pull requests

Please include:

- A clear description of the change in your own words
- Screenshots or recordings for UI changes if applicable

## Translations

Translations are gettext catalogs in `rust/app/translations/<language>/LC_MESSAGES/feishin.po`. When you add or change user-visible text, wrap it in `@tr(...)` (Slint) or `tr!`/`tr_plural!` (Rust) and run `rust/scripts/translations.py` to add the new strings to every catalog. To improve a translation, edit the `msgstr` entries of your language and check the result with `rust/scripts/translations.py check <language>`.
