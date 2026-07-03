---
id: tko-j1bw
status: closed
deps: [tko-vwbb]
links: []
created: 2026-07-03T20:54:48Z
type: task
priority: 1
assignee: Matthew Pickering
parent: tko-9noa
---
# cmd: list/ls/ready/blocked/closed

Listing commands with shared filter/sort flags (-a/--assignee, -T/--tag, -t/--type, -s/--sort, -r/--reverse). list adds --status; closed adds --limit. ready=all deps closed; blocked=unresolved deps.


## Notes

**2026-07-03T21:24:00Z**

Implemented list/ls, ready, blocked, closed against the Backend trait: shared matches_filters/sort_tickets/filter_by_dependency_status helpers mirror tk's Filter/Sort/FilterByDependencyStatus.
