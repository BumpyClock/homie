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

## OpenClaw patterns worth copying (code refs)
Auth store + external CLI sync:
- `~/Projects/openclaw/src/agents/auth-profiles/store.ts`
  - `auth-profiles.json` schema: `{ version, profiles, order?, lastGood?, usageStats? }`
  - updates are lock-protected (proper-lockfile) + migrations (legacy auth.json / oauth.json)
- `~/Projects/openclaw/src/agents/cli-credentials.ts`
  - Codex CLI creds: keychain `"Codex Auth"` + account derived from `CODEX_HOME` OR `$CODEX_HOME/auth.json`
  - Claude Code creds: keychain `"Claude Code-credentials"` OR `~/.claude/.credentials.json`
  - supports write-back of refreshed Claude tokens (keychain/file)

Copilot device flow + token exchange:
- `~/Projects/openclaw/src/providers/github-copilot-auth.ts`
  - device-code login to GitHub endpoints; handles `authorization_pending`, `slow_down`, `expired_token`, `access_denied`
- `~/Projects/openclaw/src/providers/github-copilot-token.ts`
  - exchange GitHub token -> Copilot token via `https://api.github.com/copilot_internal/v2/token`
  - cache token w/ expiry; derive API base URL from `proxy-ep=` inside token

Run/event model:
- lane queueing: `~/Projects/openclaw/src/process/command-queue.ts`
- session/global lane nesting: `~/Projects/openclaw/src/agents/pi-embedded-runner/run.ts`
- active run registry + abort: `~/Projects/openclaw/src/agents/pi-embedded-runner/runs.ts`
- monotonic per-run events: `~/Projects/openclaw/src/infra/agent-events.ts`
  - `seq` monotonic per runId; streams: lifecycle/assistant/tool/compaction

Exec allowlist patterns (future Homie allowlist):
- `~/Projects/openclaw/src/infra/exec-approvals.ts`
  - resolves executable path + pattern matching for approvals/allowlists

## codex-rs mechanics to mirror (code refs)
Device-code flow (Codex subscription):
- `~/Projects/references/codex/codex-rs/login/src/device_code_auth.rs`
  - endpoints: `{issuer}/api/accounts/deviceauth/usercode` + `/deviceauth/token`
  - verification URL: `{issuer}/codex/device`
  - PKCE redirect URI: `{issuer}/deviceauth/callback`
  - poll treats `403/404` as pending; 15m timeout

Token storage + keyring mapping:
- `~/Projects/references/codex/codex-rs/core/src/auth/storage.rs`
  - `$CODEX_HOME/auth.json` format (`AuthDotJson`)
  - keyring service `"Codex Auth"` + account `cli|<sha256(canonical CODEX_HOME)[:16]>`

Model catalog caching:
- `~/Projects/references/codex/codex-rs/core/src/models_manager/manager.rs`
  - `DEFAULT_MODEL_CACHE_TTL = 300s`
  - on-disk cache `models_cache.json` (ETag-aware refresh)

Event + approvals taxonomy (what Homie chat UX expects long-term):
- `~/Projects/references/codex/codex-rs/app-server/README.md`
- `~/Projects/references/codex/codex-rs/app-server-protocol/src/protocol/v2.rs`
  - item lifecycle: `item/started` → deltas → `item/completed`
  - approval requests are server→client JSON-RPC requests:
    - `item/commandExecution/requestApproval`
    - `item/fileChange/requestApproval`
  - decisions include accept/decline/cancel + “for session” variants
- cancel semantics: `turn/interrupt` then `turn/completed(status="interrupted")`
  - impl detail: `~/Projects/references/codex/codex-rs/app-server/src/codex_message_processor.rs` queues interrupt replies until abort arrives

Mid-turn user input injection (Codex behavior):
- `~/Projects/references/codex/codex-rs/core/src/state/turn.rs`
  - `TurnState.pending_input: Vec<ResponseInputItem>`
  - take-all per iteration (`take_pending_input`)
- `~/Projects/references/codex/codex-rs/core/src/codex.rs`
  - `inject_input(...)` pushes pending input; loop consumes pending before each sampling call
  - implication: “send while running” does not interrupt; processed next iteration

Approval key shape (Codex behavior):
- shell approval key: argv + cwd + sandbox perms
  - `~/Projects/references/codex/codex-rs/core/src/tools/runtimes/shell.rs`
- unified exec approval key: shell key + `tty`
  - `~/Projects/references/codex/codex-rs/core/src/tools/runtimes/unified_exec.rs`

## Extension direction
- Add auth module: OAuth provider adapters + token store + refresh.
- Add AuthResolver → inject per-request credentials into provider config.
- Add approval policy hook into tool execution loop.
- Add session store + transcript + compaction hooks.

## Implications for roci API design (new)
- Auth:
  - device-code sessions must expose `{verification_url,user_code,interval,expires_at}`
  - AuthError should normalize: pending/slow_down/expired/access_denied/provider_disabled/workspace_not_allowed/rate_limited
- Run:
  - include `seq` + `ts` per event (monotonic ordering, OpenClaw pattern)
  - cancellation: `abort()` async; emit terminal `canceled` after cleanup (Codex pattern)
- Approvals:
  - shape should be rich enough to represent Codex command/file approvals, even if Homie UI only supports “approve/decline” initially

## Decisions (confirmed)
- Claude Code MVP: import creds from Claude Code CLI (keychain/file); real auth flow later.
- Copilot MVP: github.com only.
- Codex: device-code endpoints under issuer; refresh via `https://auth.openai.com/oauth/token` default (codex-rs behavior).
- Approvals: implement accept/decline/cancel + accept-for-session variants in roci + Homie UI.
- Approval persistence: accept-for-session ephemeral per thread; “always approve/execute mode” persisted per thread + global.
- Token store: file-only MVP configured via `~/.homie/config.toml` (keyring later).
- Model catalog TTL: default 300s (mirror Codex); cache memory + disk; allow override.
- Execpolicy: single file `~/.homie/execpolicy.toml` (global, gateway-owned).
- Transcript storage: persist normalized items + optional `raw_provider_event_json` (debug-only; no redaction MVP; last 10 runs).
- Debug logging: `tracing` structured fields (`chat_id/thread_id/turn_id/item_id/provider/model`); never log tokens.
- Execpolicy glob semantics: mirror Claude Code patterns (`Bash(cmd:*)`) but implement token-based argv matching + shorthand parser.
- Raw provider events retention: keep last 10 runs when debug enabled.
