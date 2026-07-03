---
id: tko-dnge
status: closed
deps: [tko-jhl6]
links: []
created: 2026-07-03T20:54:29Z
type: epic
priority: 0
assignee: Matthew Pickering
---
# Backend abstraction

Define the Backend trait (issue CRUD, list, set-state, comments, best-effort native deps) and the native-issue-number ID resolution (#123 / partial-prefix).


## Notes

**2026-07-03T21:14:10Z**

Implemented Backend trait (create/get/list/update_issue, set_state, comments, best-effort native dep wiring) and resolve_id() for #123/prefix ID resolution in src/backend.rs, with unit tests.
