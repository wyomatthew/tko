---
id: tko-kh41
status: closed
deps: [tko-vwbb]
links: []
created: 2026-07-03T20:54:48Z
type: task
priority: 1
assignee: Matthew Pickering
parent: tko-9noa
---
# cmd: show

tko show <id>: fetch issue, parse document, render full ticket incl. relationships (blockers/blocking/children/links). Accepts 123 or #123.


## Notes

**2026-07-03T21:24:00Z**

Implemented show(): renders the ticket document, inlines a parent comment line, and appends a Blockers/Blocking/Children/Links relationships block (text and --json).
