---
id: tko-af78
status: closed
deps: [tko-vwbb]
links: []
created: 2026-07-03T20:54:48Z
type: task
priority: 1
assignee: Matthew Pickering
parent: tko-9noa
---
# cmd: start/close/reopen/status

Status transitions. start->in_progress, close->closed, reopen->open, status <id> <status>. Update the body status field and best-effort mirror to issue open/closed + tko:in_progress label.


## Notes

**2026-07-03T21:24:00Z**

Implemented start/close/reopen/status via a shared set_status() that updates ticket.status, re-renders the body, and mirrors open/closed state plus tko:in_progress label onto the issue.
