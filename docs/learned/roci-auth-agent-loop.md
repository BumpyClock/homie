---
topic: roci auth + agent loop extensions
date: 2026-02-02
source: local repo review (/home/bumpyclock/Projects/references/roci)
---

# Roci extension notes (auth + agent loop)

## Current capabilities
- Provider abstraction + streaming + tool calls (Rust).
- Agent module (feature `agent`) with in-memory conversation + tool loop.
- Auth config is API key only via env (`RociConfig::from_env`).
- Tool execution is immediate (no approval gate).
- No OAuth/token refresh/store integration.
- No persistence/compaction/memory/hooks.

## Key gaps for Homie use
- OAuth flows for Codex/OpenAI, Claude Code, GitHub Copilot.
- Token refresh + per-provider auth headers.
- Approval gating + tool policy.
- Session persistence + event log + compaction.
- Memory search + recall tools.

## Extension direction
- Add auth module: OAuth provider adapters + token store + refresh.
- Add AuthResolver → inject per-request credentials into provider config.
- Add approval policy hook into tool execution loop.
- Add session store + transcript + compaction hooks.
