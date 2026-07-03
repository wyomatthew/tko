---
id: tko-1anh
status: open
deps: [tko-vwbb]
links: []
created: 2026-07-03T20:54:48Z
type: task
priority: 1
assignee: Matthew Pickering
parent: tko-9noa
---
# cmd: dep + undep

dep add/remove/tree/check and undep. Store deps in the body frontmatter; reject self-deps, duplicates, cycles (CheckCycle/DFS). Best-effort mirror to Gitea native issue dependencies. tree renders the dependency graph.

