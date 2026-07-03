---
id: tko-jxjl
status: closed
deps: [tko-kh41]
links: []
created: 2026-07-03T20:54:48Z
type: task
priority: 2
assignee: Matthew Pickering
parent: tko-9noa
---
# global --json flag

Persistent --json flag producing machine-readable output for all commands, matching tk's JSON field names.


## Notes

**2026-07-03T21:24:00Z**

The --json global flag was already present on Cli; every command handler in commands.rs now branches on it, emitting Ticket::to_json() (a new method covering the body fields that are serde-skipped from YAML) instead of the human-readable line/block format.
