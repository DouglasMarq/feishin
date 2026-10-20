# docs

Project documentation that doesn't belong in code comments: agent workflow and implementation plans. The history and scope of the native rewrite live in `rust/MIGRATION.md`.

## Layout

```
docs/
  README.md          this file
  agents/            agent workflow: commits, issue tracker, triage labels, domain docs
  plans/
    _template.md     copy this to start a new plan
    TASK<UnixTimestamp>-<branch-slug>.md one plan per task (replace `<UnixTimestamp>` with the output of `date +%s`)
```

## Specs, issues and plans

- **Specs and issues** live in `.scratch/<feature>/` (gitignored), as described in `agents/issue-tracker.md`. They say _what_ to build.
- **Plans** live in `plans/` and are committed. They say _how_ a multi-step task is implemented and verified, and link the `.scratch` spec or issue they implement.

## Task plans

Any task with more than one step gets a plan before coding starts. One-line fixes don't need one.

### Creating a plan

1. Create the branch from `feat/rust-native`, e.g. `feat/lyrics-offset`.
2. Copy `plans/_template.md` to `plans/TASK<UnixTimestamp>-<branch-slug>.md`, replacing `/` with `-`:
   `feat/lyrics-offset` → `plans/TASK1792500000-feat-lyrics-offset.md`.
3. Fill in Goal, Context, Assumptions, Out of scope, Steps and Success criteria.
4. Ask a reviewer to approve it.

### Lifecycle

The `status` field in the frontmatter goes through these values:

| Status        | Meaning                                        | Who changes it |
| ------------- | ---------------------------------------------- | -------------- |
| `draft`       | Written, open questions not answered yet       | Author         |
| `approved`    | Scope and steps agreed, coding can start       | Reviewer/owner |
| `in-progress` | Being implemented; checkboxes are kept current | Implementer    |
| `done`        | Merged; the file is not edited anymore         | Implementer    |

### Rules

- **Every step needs a check**, e.g. `→ verify: cargo test -p feishin-core given_shuffle_off_restores_the_original_order`.
- **Record changes of plan** under _Decisions / deviations_ with the date and the reason. Don't silently rewrite earlier steps.
- **Commit the plan in the same PR as the code** and link it in the PR description.
- **Leave finished plans alone.** They record why something was built the way it was. If the work changes later, write a new plan.
- **Say which servers are affected.** If the task changes a server client, list the backends (Navidrome, Subsonic, Jellyfin) and the live checks to run in _Context_.
- **Use fictitious data only:** made-up servers (`http://navidrome.example`), users (`feishin`), passwords and song metadata. Never put real server addresses, credentials or library contents in a plan.
- **Never use the em dash** - use "-".

### Success criteria checklist

Each plan's _Success criteria_ keeps the checklist from [`plans/_template.md`](plans/_template.md) unchanged and adds the task's Given-It cases. Run the checks from `rust/` with the commands in `.claude/CLAUDE.md` §6.

- [ ] `cargo fmt --all --check` passes
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` passes
- [ ] `cargo build --workspace` succeeds
- [ ] `cargo test --workspace` passes
- [ ] Every new or changed branch of logic has a test
- [ ] `python3 scripts/translations.py check` reports every language complete
- [ ] Tests follow Given-It naming and cover the happy path, empty lists and missing items, invalid input and boundaries (paging edges, unpaired or non-local remote devices)
