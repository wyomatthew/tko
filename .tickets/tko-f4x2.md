---
id: tko-f4x2
status: closed
deps: [tko-vwbb]
links: []
created: 2026-07-03T20:54:48Z
type: task
priority: 1
assignee: Matthew Pickering
parent: tko-9noa
---
# cmd: create

tko create [title] with -d/--description, --design, --acceptance, -t/--type, -p/--priority, -a/--assignee, --external-ref, --pr, --parent, --tags. Builds the ticket document, creates the backend issue, backfills id with the issue number, prints it.


## Notes

**2026-07-03T21:24:00Z**

Implemented in src/commands.rs: create() resolves parent id, defaults type=task/priority=2/assignee=git user.name, creates the issue, then re-persists once the backend issue number is known (frontmatter id can only be set after creation).
