# Homie config

Location: `~/.homie/config.toml` (or `HOMIE_HOME` override).

Example: `config.toml.example` (repo root).

Provider auth runbook: `docs/provider-auth.md`.
Quick start: `docs/quick-start.md`.

## System prompt
- Default prompt stored in repo: `src/core/system_prompt.md`.
- On first run, Homie writes `~/.homie/system_prompt.md` if missing.
- Override path: `chat.system_prompt_path`.
- Resolution behavior:
  - `chat.system_prompt_path` unset/blank -> load `~/.homie/system_prompt.md` (auto-created from repo default on first run).
  - `chat.system_prompt_path` set -> load only that file path (no auto-copy).

## Web tools
Both tools are disabled by default.

### web_fetch
Enable: `tools.web.fetch.enabled = true`
- SSRF guard blocks localhost/private IPs.
- Readability extraction for HTML, JSON pretty-print, optional Firecrawl fallback.
- Cache with TTL.

Firecrawl options:
- `tools.web.fetch.firecrawl.enabled = true`
- `tools.web.fetch.firecrawl.api_key` or `FIRECRAWL_API_KEY`
- `tools.web.fetch.firecrawl.base_url` supports self-hosted.

### web_search
Enable: `tools.web.search.enabled = true`
- Providers: `brave` or `searxng`.
- Cache with TTL.

Brave:
- `tools.web.search.brave.api_key` or `BRAVE_API_KEY`.

SearXNG:
- `tools.web.search.searxng.base_url` or `SEARXNG_BASE_URL`.
- Optional `tools.web.search.searxng.api_key` or `SEARXNG_API_KEY`.
- Optional `tools.web.search.searxng.api_key_header` + extra `headers`.
- Instance must allow JSON (`format=json`).

## Tool providers
- `tools.providers.<provider_id>` controls per-provider tool loading.
- Built-in `core` provider exists by default.
- `tools.providers.<provider_id>.channels` is an optional channel allowlist.
  - omitted or `[]` -> all channels
  - set -> provider loads only when current channel matches one of the listed values
  - channel gating applies after `enabled` and before per-tool allow/deny filters
- Conflict detection:
  - unknown enabled provider -> config error
  - duplicate tool name across enabled providers -> config error
  - unknown tool names in `allow_tools`/`deny_tools` -> config error

### Browser automation (`browser` tool)
- Core provider now includes `browser`, backed by `agent-browser` (`vercel-labs/agent-browser`).
- Install locally:
  - `npm i -g agent-browser && agent-browser install`
  - or `brew install agent-browser && agent-browser install`
- Runtime fallback: if `agent-browser` binary is unavailable, Homie tries `npx --yes agent-browser`.
- Optional override for binary path:
  - `HOMIE_AGENT_BROWSER_BIN=/path/to/agent-browser`
- Tool can be gated using core allow/deny lists like any other core tool.

Example:
```toml
[tools.providers.core]
enabled = true
channels = ["web", "discord"]
allow_tools = ["read", "ls", "find", "grep", "browser"]
deny_tools = ["exec"]
```

## Provider auth flow (Homie)
Detailed step-by-step flow: `docs/provider-auth.md`.

Homie uses device-code auth for providers that support it.

Supported now:
- `openai-codex`
- `github-copilot`

Not supported via device-code:
- `claude-code` (CLI credential import only)

### Check provider status
Call:
- `chat.account.list`

Response shape:
- `providers[]` with:
  - `id` (`openai-codex`, `github-copilot`, `claude-code`)
  - `enabled` (from config)
  - `logged_in`
  - optional: `expires_at`, `scopes`, `has_refresh_token`

### Start login
Call:
- `chat.account.login.start`

Params:
```json
{
  "provider": "github-copilot",
  "profile": "default"
}
```

Notes:
- `provider` accepts either dash or underscore form:
  - `github-copilot` / `github_copilot`
  - `openai-codex` / `openai_codex`
- `profile` is optional; default is `"default"`.

Returns:
- `session` with:
  - `verification_url`
  - `user_code`
  - `device_code`
  - `interval_secs`
  - `expires_at` (RFC3339)

### Poll login
After user completes browser verification, call:
- `chat.account.login.poll`

Params:
```json
{
  "provider": "github-copilot",
  "profile": "default",
  "session": {
    "verification_url": "...",
    "user_code": "...",
    "device_code": "...",
    "interval_secs": 5,
    "expires_at": "2026-02-11T22:10:00Z"
  }
}
```

Poll result status:
- `pending`
- `slow_down`
- `authorized`
- `denied`
- `expired`

Stop when `authorized`, then refresh with:
- `chat.account.list` (or `chat.account.read`)

### Credentials storage
- Default path: `~/.homie/credentials`
- Override: `paths.credentials_dir` in `~/.homie/config.toml`
- Provider files are TOML and profile-scoped.

### Provider-specific notes
- `openai-codex`:
  - Homie can import Codex CLI credentials when available.
- `github-copilot`:
  - Requires Homie device-code flow above.
  - `gh auth login` alone does not create Homie provider credentials.
- `claude-code`:
  - Uses CLI credential import (`providers.claude_code.import_from_cli = true`).

## Local vLLM / OpenAI-compatible models
To surface local models (vLLM, LM Studio proxy, other OpenAI-compatible endpoints) in chat model pickers:

1. Configure in `~/.homie/config.toml`:
   - `[providers.openai_compatible]`
   - `enabled = true`
   - `base_url = "http://<host>:<port>/v1"`
   - optional `api_key = "..."`
   - optional `models = ["model-a","model-b"]` (fallback list)
2. Optional env overrides:
   - `OPENAI_COMPAT_BASE_URL`
   - `OPENAI_COMPAT_API_KEY`
   - `OPENAI_COMPAT_MODELS` (comma-separated fallback)
3. Restart gateway.

Behavior:
- `chat.model.list` queries `<base_url>/models` from config/env and adds entries as `openai-compatible:<model-id>`.
- Web and mobile composer model pickers group these under `OpenAI-Compatible / Local`.

## Paths
- `paths.credentials_dir` default: `~/.homie/credentials`.
- `paths.execpolicy_path` default: `~/.homie/execpolicy.toml`.

## Debug
- `debug.persist_raw_provider_events` stores raw provider events in sqlite when enabled.
- Runtime env flags: `HOMIE_DEBUG=1` or `HOME_DEBUG=1`.

## Client env vars
- Web: `VITE_GATEWAY_URL=ws://<host>:9800/ws`
- Mobile: `EXPO_PUBLIC_HOMIE_GATEWAY_URL=ws://<host>:9800/ws` (prefill hint only)

## Gateway env vars
- `HOMIE_BIND` (default `127.0.0.1:9800`)
- `HOMIE_TAILNET_BIND` (optional second bind)
- `HOMIE_ALLOW_LAN=1` (allow private LAN clients)
- `HOMIE_TAILSCALE=1` (enables Tailscale Serve behavior)
- `HOMIE_TAILSCALE_SERVE=1` (auto `tailscale serve https /` for the bind port)
- `HOMIE_DB_PATH` (override sqlite path; default `homie.db`)
- `HOMIE_LOG` / `RUST_LOG` (logging filter)
