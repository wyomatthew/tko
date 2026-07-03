# tko — agent & contributor guide

`tko` is a CLI tool (written in **Rust**) for managing tickets, built to delight
humans and agents alike. It is a re-implementation of the Go tool **`tk`**
(source: `~/code/ticket`, module `github.com/radutopala/ticket`) with one
fundamental difference:

- **`tk`** stores tickets as markdown files in a local `.tickets/` directory.
- **`tko`** stores tickets in a modern git-service issue tracker — **Gitea**
  (v1), and later GitHub / GitLab — using their issues as the persistence medium.

The CLI surface and the on-the-wire ticket format are **identical to `tk`**. A
user who knows `tk` already knows `tko`; the only setup difference is a committed
`tko.toml` describing the backend, plus a backend access token supplied out of
band.

---

## Core principle: the issue body is the source of truth

Every ticket is stored as the **exact `tk` markdown document** — YAML frontmatter
followed by a markdown body — placed verbatim in the **issue body** on the
backend. `tko` parses and rewrites this document; it never invents a different
serialization.

Native issue features (labels, assignees, open/closed state, title, native
dependencies, comments) are a **best-effort mirror** that each backend keeps
roughly in sync so the tickets look and behave sensibly in the web UI. They are
**never authoritative** — on any disagreement, the frontmatter+body wins. A
backend is free to skip mirroring a field it cannot represent cheaply.

### Ticket document format (identical to `tk`)

Frontmatter (YAML between `---` fences):

| Field          | YAML key       | Type            | Notes |
|----------------|----------------|-----------------|-------|
| ID             | `id`           | string          | Set to the backend issue number after creation (see ID scheme). Always present. |
| Status         | `status`       | enum            | `open` \| `in_progress` \| `closed`. Always present. |
| Type           | `type`         | enum            | `task` \| `bug` \| `feature` \| `epic` \| `chore`. Default `task`. |
| Priority       | `priority`     | int 0–4         | 0 = highest, default 2. Omitted when 0. |
| Assignee       | `assignee`     | string          | Single assignee. |
| Parent         | `parent`       | string (id)     | Parent ticket. |
| External ref   | `external-ref` | string          | e.g. `gh-123`, `JIRA-456`. |
| PR             | `pr`           | string          | e.g. `!123`, a URL. |
| Tags           | `tags`         | list of string  | "labels". |
| Deps           | `deps`         | list of id      | Tickets this one is blocked by. |
| Links          | `links`        | list of id      | Bidirectional peer links. |
| Created        | `created`      | RFC3339 time    | Always present. |

Body sections:

- `# <title>` — Title
- free text after the title — Description
- `## Design` — Design
- `## Acceptance Criteria` — Acceptance
- `## Notes` — timestamped notes, each under a `### <RFC3339>` subheading

Fields with no value are omitted (matching `tk`'s `omitempty`), except `id`,
`status`, and `created`, which are always written.

---

## ID scheme

The **canonical ID is the backend's native issue number** (e.g. `123`). Commands
accept it as `123` or `#123`. Partial matching is numeric prefix / exact match
(unlike `tk`'s substring match over `tic-<hex>` IDs).

The frontmatter `id` field is populated with the issue number once the backend
assigns it on creation. `tko` does **not** generate `tic-<hex>` IDs.

## Status mapping

Issue trackers typically have only `open`/`closed`. `tko`'s three-state status
lives in the body. Best-effort mirror on Gitea:

- `open` → issue open, no status label
- `in_progress` → issue open + `tko:in_progress` label
- `closed` → issue closed

---

## Configuration: `tko.toml`

A project using `tko` **must** have a `tko.toml` committed at its repo root. It is
discovered by walking up from the current directory (like `tk` finds `.tickets/`).
It contains **no secrets**.

```toml
[backend]
kind  = "gitea"                      # gitea | github | gitlab   (v1: gitea only)
url   = "https://gitea.example.com"  # base URL of the instance
owner = "myorg"                      # repo owner / namespace
repo  = "myproject"                  # repo name
token_env = "TKO_TOKEN"              # optional: name of the env var holding the token
```

## Secrets / access token

The token is **never** stored in `tko.toml`. `tko` resolves it in this order
(first hit wins):

1. **Environment variable** — the name given by `backend.token_env`, else the
   default `TKO_TOKEN`, else a backend-specific fallback (`GITEA_TOKEN`).
2. **`.env`** in the project root (same directory as `tko.toml`).
3. **`~/.config/tko/secrets.env`** — a user-global env file.

`.env` and `secrets.env` are simple `KEY=value` files.

---

## Architecture

- **Language / stack:** Rust. `clap` (derive) for the CLI, `reqwest` (blocking,
  rustls) for HTTP, `serde` + `serde_yaml` for frontmatter, `toml` for config,
  `anyhow` / `thiserror` for errors.
- **`Backend` trait** abstracts all persistence: create / read / update issue,
  list issues, set state, add comment, and (best-effort) native dependency wiring.
  The CLI is written entirely against this trait.
- **`GiteaBackend`** is the first and only v1 implementation. GitHub and GitLab
  are deferred behind the same trait.

### Gitea field mapping (best-effort mirror; body is source of truth)

| tk field        | Gitea native slot (best-effort)              |
|-----------------|----------------------------------------------|
| title           | issue title                                  |
| status          | open/closed + `tko:in_progress` label        |
| assignee        | issue assignee                               |
| tags            | issue labels                                 |
| type            | `tko:type:<t>` label                         |
| priority        | `tko:prio:<n>` label                         |
| deps            | native issue dependencies                    |
| notes           | issue comments (also kept in `## Notes`)     |
| description/design/acceptance | issue body (part of the document) |

---

## Coding principles

- **Modern, idiomatic Rust, always.** Prefer current idioms and standard-library
  facilities over clever or dated patterns.
- **Add dependencies with careful discretion.** A new dependency must be reliable,
  well-maintained, and modern. Prefer the standard library or an existing
  dependency before reaching for a new crate.
- **Doc strings, not comments.** Write plain, matter-of-fact doc strings
  frequently. Do **not** litter the codebase with comments — avoid them almost
  entirely, reserving them only for something genuinely unobvious that begs for
  explanation.
- **Understand the ticket before coding.** For any feature, read and understand the
  full ticket context first. Note any ambiguity and resolve it *before* starting
  implementation.
- **Test frequently.** Write tests as you develop, using them to iterate. Tests
  should be robust and comprehensive.

---

## Command scope

**v1 (core CRUD + deps):** `create`, `show`, `list`/`ls`, `ready`, `blocked`,
`closed`, `start`, `close`, `reopen`, `status`, `edit`, `add-note`,
`dep add|remove|tree|check`, `undep`, `link`/`unlink`, plus the global `--json`
flag. Behavior matches `tk` (see `~/code/ticket/internal/cmd/`).

**Deferred (tracked, not in v1):** `bulk`, `export`/`import`, `stats`, `search`,
`query`, `update`/self-update, `migrate-beads`; and the GitHub / GitLab backends.

---

## Reference

- `tk` source of truth for CLI + format: `~/code/ticket`
  (commands in `internal/cmd/`, ticket types in `pkg/ticket/`).
- The project backlog is itself tracked as `tk` tickets in `.tickets/` in this
  repo (dogfooding). Use `tk ls`, `tk dep tree --full`, `tk show <id>`.

> Note: `CLAUDE.md` is a symlink to this file.
