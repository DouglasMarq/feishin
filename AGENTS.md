## General guidelines

- Never use the em dash "—". Use plain dash "-" instead.
- The app is the native Rust/Slint app in `rust/`. Its code standards are in `rust/CLAUDE.md`; run `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings` and `cargo test --workspace` from `rust/` before committing.
- On an explicit commit request: `git add` the changes, then commit. Do not stage outside a commit request.

## Agent skills

### Issue tracker

Issues live as markdown under `.scratch/<feature>/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default vocabulary: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `CONTEXT.md` + `docs/adr/`. See `docs/agents/domain.md`.

### Translations

User-visible text is translatable: `@tr(...)` in Slint, `tr!`/`tr_plural!` in Rust. After adding or changing such text, run `rust/scripts/translations.py` and translate the new entries of every catalog in `rust/app/translations`; `rust/scripts/translations.py check` must report every language complete.

### Commits

Conventional Commits + allowed scopes: read `docs/agents/commits.md` when writing a commit message. Enforced by commitlint in CI.

## Working mode

Ponytail (lazy senior / YAGNI ladder) on coding tasks. See the `ponytail` skill.
