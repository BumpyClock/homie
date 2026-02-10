# Homie config

Location: `~/.homie/config.toml` (or `HOMIE_HOME` override).

Example: `config.toml.example` (repo root).

## System prompt
- Default prompt stored in repo: `src/core/system_prompt.md`.
- On first run, Homie writes `~/.homie/system_prompt.md` if missing.
- Override path: `chat.system_prompt_path`.

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
- Built-in `openclaw_browser` provider is dynamic scaffold (disabled by default).
- Dynamic providers are disabled by default until `enabled = true`.
- `tools.providers.<provider_id>.channels` is an optional channel allowlist.
  - omitted or `[]` -> all channels
  - set -> provider loads only when current channel matches one of the listed values
  - channel gating applies after `enabled` and before per-tool allow/deny filters
- Conflict detection:
  - unknown enabled provider -> config error
  - duplicate tool name across enabled providers -> config error
  - unknown tool names in `allow_tools`/`deny_tools` -> config error

### OpenClaw browser scaffold
- Enable provider: `tools.providers.openclaw_browser.enabled = true`
- Configure endpoint: `tools.openclaw_browser.endpoint = "https://..."`
- Optional auth: `tools.openclaw_browser.api_key = "..."`.
- Current status: tool schema + registration exist; execution returns structured `not_configured` or `not_implemented`.

Example:
```toml
[tools.providers.core]
enabled = true
channels = ["web", "discord"]
allow_tools = ["read", "ls", "find", "grep"]
deny_tools = ["exec"]

[tools.providers.openclaw_browser]
enabled = true
channels = ["web"]
```

## Paths
- `paths.credentials_dir` default: `~/.homie/credentials`.
- `paths.execpolicy_path` default: `~/.homie/execpolicy.toml`.

## Debug
- `debug.persist_raw_provider_events` stores raw provider events in sqlite when enabled.
- Runtime env flags: `HOMIE_DEBUG=1` or `HOME_DEBUG=1`.
