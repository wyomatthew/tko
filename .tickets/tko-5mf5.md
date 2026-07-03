---
id: tko-5mf5
status: closed
deps: [tko-vwbb]
links: []
created: 2026-07-03T20:54:48Z
type: task
priority: 2
assignee: Matthew Pickering
parent: tko-9noa
---
# cmd: link/unlink

Symmetric bidirectional links between two or more tickets; unlink removes a link. Stored in the links frontmatter on both sides.


## Notes

**2026-07-03T21:24:00Z**

Implemented link/unlink: link adds bidirectional links across all given ids (rejecting duplicates), unlink removes the link from both sides and errors if neither side had it.
