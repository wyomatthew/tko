---
id: tko-1anh
status: closed
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


## Notes

**2026-07-03T21:24:00Z**

Implemented dep add/remove/tree/check and undep (alias for dep remove): local cycle detection (check_cycle/detect_cycles) mirrors tk's DFS-based algorithm; dep add/remove also best-effort mirror onto backend.add_dependency/remove_dependency.
