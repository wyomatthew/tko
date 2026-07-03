---
id: tko-vwbb
status: closed
deps: [tko-dnge, tko-97nc]
links: []
created: 2026-07-03T20:54:29Z
type: epic
priority: 0
assignee: Matthew Pickering
---
# Gitea backend

Implement GiteaBackend against the Gitea REST API: map issue body <-> ticket document; best-effort mirror of title/labels/assignee/state/native deps/comments.


## Notes

**2026-07-03T21:17:18Z**

Implemented GiteaBackend in src/gitea.rs: full Backend trait impl over the Gitea REST API (issues CRUD, state, comments, native dependencies), with label-name<->id resolution (creating missing labels). Domain-to-label mapping (tko:type:*, tko:prio:*, tko:in_progress) is left to the caller — this module only talks to Gitea. Added mockito as a dev-dependency and unit-tested every trait method against a mocked server.
