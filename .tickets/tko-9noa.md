---
id: tko-9noa
status: closed
deps: [tko-vwbb]
links: []
created: 2026-07-03T20:54:29Z
type: epic
priority: 0
assignee: Matthew Pickering
---
# CLI commands v1

Implement the v1 command surface against the Backend trait, matching tk behavior: create, show, list/ls/ready/blocked/closed, start/close/reopen/status, edit, add-note, dep, undep, link/unlink, global --json.


## Notes

**2026-07-03T21:24:04Z**

All v1 commands implemented in src/commands.rs and wired up via main.rs (Config::discover -> resolve_token -> GiteaBackend -> commands::dispatch). Added Ticket::to_json() for --json output (body fields are serde-skipped from the YAML frontmatter). 9 new unit tests against an in-memory mock Backend, plus updated the stale cli.rs smoke test that expected 'not yet implemented'. cargo build/test/clippy all clean (41 tests passing).
