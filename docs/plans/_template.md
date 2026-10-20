---
status: draft # draft | approved | in-progress | done
branch: feat/<slug> # feat | fix | perf | refactor | chore | or other types of Conventional Commits
ticket: <link> # .scratch/<feature>/spec.md or issues/NN-<slug>.md, or a GitHub issue
---

# <Goal in one sentence>

## Context

Why this is needed; links to the spec or issue, `rust/MIGRATION.md` sections, related code.

Servers affected: <none | Navidrome, Subsonic, Jellyfin, and the live checks to run>

## Assumptions

- ...

## Out of scope

- ...

## Steps

- [ ] 1. <step> → verify: <test / command>
- [ ] 2. <step> → verify: <test / command>

## Success criteria

- Tests: <Given-It cases that must exist>
- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] Every new or changed branch of logic has a test
- [ ] `python3 scripts/translations.py check` reports every language complete
- [ ] Tests follow Given-It naming and cover the happy path, empty lists and missing items, invalid input and boundaries (paging edges, unpaired or non-local remote devices)

## Open questions

- ...

## Decisions / deviations

- <date>: changed X because Y
