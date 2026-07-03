---
id: tko-97nc
status: open
deps: [tko-unag]
links: []
created: 2026-07-03T20:54:29Z
type: epic
priority: 0
assignee: Matthew Pickering
---
# Config & secrets

Parse tko.toml (backend kind/url/owner/repo/token_env), discover it by walking up from cwd, and resolve the backend token from env -> project .env -> ~/.config/tko/secrets.env.

