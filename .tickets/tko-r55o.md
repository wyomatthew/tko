---
id: tko-r55o
status: closed
deps: [tko-vwbb]
links: []
created: 2026-07-03T20:54:48Z
type: task
priority: 2
assignee: Matthew Pickering
parent: tko-9noa
---
# cmd: add-note

tko add-note <id> [text] (or stdin): append a timestamped entry under ## Notes in the body; best-effort also post a Gitea comment.


## Notes

**2026-07-03T21:24:00Z**

Implemented add-note(): reads text from the argument or stdin, appends a Note{timestamp: now_utc, content}, and persists.
