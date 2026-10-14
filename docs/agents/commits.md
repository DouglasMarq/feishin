# Commits

Conventional Commits for history that can feed release notes and scoped changelogs. Enforced by commitlint (`commitlint.config.mjs`) and the Commitlint workflow.

## Format

```
type(scope): subject

[optional body]

[optional footer]
```

Examples:

- `feat(player): add MPV reload control`
- `fix(window): restore minimized window`
- `chore(deps): bump slint`
- `feat!: drop legacy auth path` or footer `BREAKING CHANGE: …`

## Types

Use the conventional set (from `@commitlint/config-conventional`):

`feat` · `fix` · `perf` · `refactor` · `docs` · `style` · `test` · `build` · `ci` · `chore` · `revert`

Prefer `feat` / `fix` / `perf` for user-facing notes; keep chore/build/ci out of highlight sections when generating notes.

## Scopes

Scope = product area a release-note reader cares about - not a crate or module and not “settings” (scope the feature the setting belongs to).

Scope is optional. When present, it must be one of (see `commitlint.config.mjs` - source of truth):

### Product

| Scope | Use for |
|-------|---------|
| `player` | Queue, mpv playback, player bar, now-playing playback |
| `radio` | Radio stations / radio playback |
| `lyrics` | Lyrics fetch, display, lyrics-specific options |
| `library` | Albums, artists, songs, folders, genres, favorites, search, playlists |
| `servers` | Server list, login, connection |
| `theme` | Themes, palettes, accent color |
| `ui` | Pages, styling, and shared UI with no single feature owner |
| `window` | Window identity, minimize/restore, fullscreen |
| `sharing` | Shares |
| `tag-editor` | Tag editing |

### Meta

| Scope | Use for |
|-------|---------|
| `api` | `MusicServer` trait and server clients with no single feature owner |
| `i18n` | Translations and the gettext catalogs |
| `ci` | GitHub Actions / workflows |
| `deps` | Dependency bumps |
| `release` | Versioning, changelogs, publish / packaging config |

Add a new scope in `commitlint.config.mjs` first, then document it here.

## Subject

- Imperative, present tense (“add”, not “added”)
- No trailing period
- Do not put agent attribution in the message (see `AGENTS.md`)

## Local check

```bash
npx --package @commitlint/cli@21.2.2 --package @commitlint/config-conventional@21.2.2 commitlint --last
```

CI runs commitlint over the PR or push range (see `.github/workflows/commitlint.yml`).
