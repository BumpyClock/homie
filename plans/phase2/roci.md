# Phase 2 Plan: Roci + Custom Agent Loop + OAuth Subscriptions

## Goal
- Own the agent loop (OpenClaw-style) inside Homie, but reuse/extend roci as the shared model+tool SDK.
- Implement OAuth-based auth flows for subscription-backed access:
  - Codex/OpenAI (Codex subscription)
  - GitHub Copilot
  - Claude Code
- Keep `chat.*` API stable for web/mobile.

## Repo layout
- roci submodule: `src/infra/roci`
- homie core runtime: `src/core`
- gateway binary: `src/gateway`

## Focused re-reads (sources + concrete takeaways)

### OpenClaw (patterns to mirror)
Auth store + external CLI sync:
- Auth profiles persisted + locked:
  - `~/Projects/openclaw/src/agents/auth-profiles/store.ts`
  - file: per-agent `auth-profiles.json` (`version`, `profiles`, `order`, `lastGood`, `usageStats`)
  - update uses file lock (proper-lockfile) to avoid concurrent writes.
- External credential import patterns (Codex CLI + Claude Code):
  - `~/Projects/openclaw/src/agents/cli-credentials.ts`
  - Codex CLI: keychain service `"Codex Auth"` account `cli|<sha256(CODEX_HOME)[:16]>` OR `$CODEX_HOME/auth.json`
  - Claude Code: keychain service `"Claude Code-credentials"` OR `~/.claude/.credentials.json`

GitHub Copilot device-code + token exchange:
- Device-code login (GitHub token):
  - `~/Projects/openclaw/src/providers/github-copilot-auth.ts`
  - `POST https://github.com/login/device/code` (client_id, scope)
  - poll `POST https://github.com/login/oauth/access_token` (authorization_pending, slow_down, expired_token, access_denied)
- Exchange GitHub token -> Copilot token (+ base URL derivation):
  - `~/Projects/openclaw/src/providers/github-copilot-token.ts`
  - `GET https://api.github.com/copilot_internal/v2/token` Authorization: Bearer <github token>
  - parse `expires_at`; cache to `stateDir/credentials/github-copilot.token.json`
  - parse `proxy-ep=...` from token, convert `proxy.*` -> `api.*`, default `https://api.individual.githubcopilot.com`

Run queueing + cancel patterns (Homie-owned, but copy mental model):
- Lane-based in-process queue:
  - `~/Projects/openclaw/src/process/command-queue.ts`
  - per-lane FIFO + `maxConcurrent`, warn-after telemetry
- Session lane + global lane nesting:
  - `~/Projects/openclaw/src/agents/pi-embedded-runner/run.ts`
  - `enqueueSession(() => enqueueGlobal(async () => { ... }))`
- Active-run registry + abort:
  - `~/Projects/openclaw/src/agents/pi-embedded-runner/runs.ts`

Event taxonomy (useful for roci RunEvent shape):
- Monotonic per-run sequence numbers + stream categories:
  - `~/Projects/openclaw/src/infra/agent-events.ts`
- Concrete lifecycle + assistant/tool streaming patterns:
  - `~/Projects/openclaw/src/agents/pi-embedded-subscribe.handlers.lifecycle.ts`
  - `~/Projects/openclaw/src/agents/pi-embedded-subscribe.handlers.messages.ts`
  - `~/Projects/openclaw/src/agents/pi-embedded-subscribe.handlers.tools.ts`

Command approval allowlisting (future-proofing):
- Path resolve + pattern matching allowlists:
  - `~/Projects/openclaw/src/infra/exec-approvals.ts`
  - resolves executable path; supports glob-ish matching; good base for future file-based allowlists

### codex-rs (mechanics we must mirror)
Device-code auth flow:
- `~/Projects/references/codex/codex-rs/login/src/device_code_auth.rs`
  - `POST {issuer}/api/accounts/deviceauth/usercode` (client_id) -> {device_auth_id,user_code,interval}
  - poll `POST {issuer}/api/accounts/deviceauth/token` (device_auth_id,user_code) -> {authorization_code,code_challenge,code_verifier}
  - verification URL `{issuer}/codex/device`
  - redirect URI `{issuer}/deviceauth/callback` for PKCE exchange
  - pending responses treated as `403/404` until 15 min timeout

Token storage format + keyring account derivation:
- `~/Projects/references/codex/codex-rs/core/src/auth/storage.rs`
  - `$CODEX_HOME/auth.json` (0600) expects `AuthDotJson { auth_mode?, OPENAI_API_KEY?, tokens?, last_refresh? }`
  - keyring service `"Codex Auth"`; account `"cli|<sha256(canonical CODEX_HOME)[:16]>"`
  - storage modes: file/keyring/auto/ephemeral

Refresh + cancel semantics (for roci API design, not exact impl):
- Refresh behavior:
  - `~/Projects/references/codex/codex-rs/core/src/auth.rs`
  - refresh uses refresh token endpoint `https://auth.openai.com/oauth/token` (override env `CODEX_REFRESH_TOKEN_URL_OVERRIDE`)
  - `refresh_if_stale`: refresh if `last_refresh` older than `TOKEN_REFRESH_INTERVAL` (currently `days(8)`)
- Model catalog caching defaults:
  - `~/Projects/references/codex/codex-rs/core/src/models_manager/manager.rs`
  - default TTL: `DEFAULT_MODEL_CACHE_TTL = 300s`
  - on-disk cache: `models_cache.json` (ETag-aware refresh)
- App-server event + approvals taxonomy (what our embedded loop should feel like):
  - `~/Projects/references/codex/codex-rs/app-server/README.md`
  - `~/Projects/references/codex/codex-rs/app-server-protocol/src/protocol/v2.rs`
    - items: `item/started` → deltas → `item/completed`
    - approvals are **server-initiated JSON-RPC requests** (client must respond):
      - `item/commandExecution/requestApproval` and `item/fileChange/requestApproval`
      - decision enums include `Accept`, `AcceptForSession`, `Decline`, `Cancel`, plus execpolicy amendment option
    - cancel: `turn/interrupt` then `turn/completed(status="interrupted")`
  - `~/Projects/references/codex/codex-rs/app-server/src/codex_message_processor.rs`
    - `turn/interrupt` response is replied when the underlying abort event arrives (async cleanup)

## Workstreams

### 1) Extend roci: auth foundation (OAuth, token store, refresh)
Deliverables (roci):
- `roci::auth` module:
  - `TokenStore` (disk persistence; pluggable path)
  - `Token` model (access/refresh/expiry/scopes/provider metadata)
  - `TokenProvider` trait (get_token, refresh_token, logged_in)
- `roci::auth::providers::*` adapters:
  - `openai_codex` (Codex subscription/OAuth)
  - `github_copilot`
  - `claude_code`
- HTTP auth integration:
  - provider constructors accept `AuthSource` (api key OR oauth token).
  - request middleware builds headers per provider.

Notes:
- Server-side login only (Homie host runs login; clients read status).
- Token paths configurable; default usable across projects.
- Prefer **device code** OAuth flows (headless-friendly).
- Token store: file-only MVP (configure via `~/.homie/config.toml`).
  - credentials path: `~/.homie/credentials/*.toml` (Homie-owned; one file per provider/profile)
  - file perms: 0600
  - filename convention (MVP): `openai-codex.toml`, `github-copilot.toml`, `claude-code.toml`
  - minimal schema (MVP):
    - `version = 1`
    - `provider = "openai-codex" | "github-copilot" | "claude-code"`
    - `profile = "default"` (future: multiple profiles)
    - `access_token`, `refresh_token?`, `id_token?`, `expires_at?` (RFC3339), `scopes?`
  - import bootstrap (one-way): read existing Codex CLI + Claude Code CLI creds (OpenClaw patterns), write into Homie files
- TODO (post-MVP): encryption at rest for token store.
- TODO (post-MVP): keyring-backed store (candidate: `keyring` crate).

Provider specifics (from re-reads):
- `openai_codex`:
  - implement Codex device-code flow like `codex-rs/login/src/device_code_auth.rs`
  - store: include access+refresh+id_token (+ account_id if available)
  - refresh endpoint default `https://auth.openai.com/oauth/token` (support override; see `codex-rs/core/src/auth.rs`)
  - optional: import existing Codex CLI creds (mac keychain + `$CODEX_HOME/auth.json`) like OpenClaw `src/agents/cli-credentials.ts`
- `github_copilot`:
  - device-code = GitHub token (`openclaw/src/providers/github-copilot-auth.ts`)
  - exchange GitHub token -> Copilot token + base URL derivation (`openclaw/src/providers/github-copilot-token.ts`)
  - cache Copilot token w/ expiry; refresh when near expiry (keep 5m safety window)
- `claude_code`:
  - investigate device-code viability (unknown)
  - MVP fallback: import Claude Code creds from keychain/file like OpenClaw (`src/agents/cli-credentials.ts`)
  - optional: write-back refreshed Claude tokens to external store (OpenClaw has `writeClaudeCliCredentials(...)`)

#### Roci API shape (auth)
- `roci::auth::TokenStore`:
  - `load(provider, profile?) -> Option<Token>`
  - `save(provider, profile?, Token)`
  - `clear(provider, profile?)`
- `roci::auth::TokenProvider`:
  - `logged_in(provider, profile?) -> bool`
  - `get_token(provider, profile?) -> Result<AccessToken, AuthError>` (lazy refresh inside)
  - `login_device_code(provider, profile?) -> DeviceCodeSession` (start/poll/complete; surface URL+code+interval+expires_at)
- `roci::auth::AuthError` (normalized):
  - `NotLoggedIn`
  - `AuthorizationPending` (device-code poll)
  - `AccessDenied` (user cancelled)
  - `ExpiredOrInvalidGrant` (refresh token invalid/expired/reused/revoked)
  - `RateLimited { retry_after_ms? }`
  - `ProviderDisabled` (e.g. codex device-code endpoint 404)
  - `WorkspaceNotAllowed` (codex forced workspace mismatch)
  - `Network | Unknown`

### 2) Extend roci: agent loop primitives (OpenClaw-ish)
Deliverables (roci):
- Session/run scaffolding (reusable primitives; avoid Homie-specific storage).
- Tool policy + approvals:
  - `ApprovalPolicy` (never/ask/always)
  - tool allow/deny groups
  - hook point: before tool execute => emit approval request + await decision
- Hooks:
  - before_run, after_run
  - before_tool, after_tool
  - tool_result_persist (transform/redact)
- Compaction + pruning interfaces:
  - compaction trigger + summary insertion
  - pruning of old tool results for context assembly

Keep roci core transport stable:
- Reuse existing `ModelProvider` + `stream_text_with_tools`.
- Add higher-level loop crate/module gated behind feature flag (e.g. `agent_loop`).

#### Roci API shape (run mechanics)
- `roci::agent_loop::Runner`:
  - `start(run: RunRequest) -> RunHandle`
- `RunRequest`:
  - provider+model selector, messages, tool registry, per-run settings
  - callbacks/hooks: `on_event`, `on_tool_call`
  - cancel token
- `RunHandle`:
  - `abort()` (async cancel; run still emits terminal event after cleanup)
  - `wait() -> RunResult`
- `RunEvent` (normalized stream; copy OpenClaw “monotonic seq + stream categories” pattern):
  - envelope: `{ run_id, seq, ts, stream, payload }` (see `openclaw/src/infra/agent-events.ts`)
  - `Lifecycle(start|end|error|canceled)`
  - `AssistantDelta(text)` + `ReasoningDelta(text)` (support raw vs markdown-ish; codex has reasoning summary+content)
  - `ToolCallStarted/Delta/Completed` + `ToolResult`
  - `PlanUpdated` + `DiffUpdated` (match Codex `turn/plan/updated` + `turn/diff/updated`)
  - `ApprovalRequired` (structured like codex approval requests)

ApprovalRequired shape (learned from Codex app-server):
- codex request params include `thread_id`, `turn_id`, `item_id`, optional `reason`
  - `codex-rs/app-server-protocol/src/protocol/v2.rs` structs:
    - `CommandExecutionRequestApprovalParams`
    - `FileChangeRequestApprovalParams`
- decisions include accept/decline/cancel + “for session” variant (and execpolicy amendment for commands)
  - `CommandExecutionApprovalDecision`, `FileChangeApprovalDecision`
Roci primitive proposal:
- `ApprovalRequest { id, kind, reason?, payload, suggested_policy_change? }`
- `ApprovalDecision { accept_mode: Once|ForSession, cancel_run?: bool }`

Approval semantics (confirmed):
- `AcceptForSession` = per-thread cache, in-memory only
  - does NOT need to survive UI reconnect / page refresh
  - keying: use normalized keys like codex-rs (argv/cwd/perms for exec; file paths for patches)
- “Execute mode / always approve” = per-thread + global toggle, persisted (survives gateway restart + UI refresh)
  - lives in Homie storage/config; passed into roci as `ApprovalPolicy::AlwaysAllow` (or equivalent)

Mid-turn user input (Codex pattern; confirmed by code re-read):
- While a run is active: additional user input can be injected into the active turn and processed on the next loop iteration.
  - codex-rs: `Session::inject_input` + `get_pending_input()` inside the agent loop
    - `~/Projects/references/codex/codex-rs/core/src/codex.rs` (`inject_input`, loop fetches pending input)
    - queue mechanics: `TurnState.pending_input` + take-all-per-iteration
      - `~/Projects/references/codex/codex-rs/core/src/state/turn.rs`
- Homie UX: per-thread toggle (collaboration mode) that controls whether “send while running” injects vs blocks.

Approval key canonicalization + future allowlisting (Codex + OpenClaw combo):
- Canonical key shape (Codex-like):
  - shell: argv + cwd + sandbox perms
    - `~/Projects/references/codex/codex-rs/core/src/tools/runtimes/shell.rs`
  - unified exec: shell key + `tty`
    - `~/Projects/references/codex/codex-rs/core/src/tools/runtimes/unified_exec.rs`
- Future file-based allowlist: store normalized keys + optional resolved exe path for matching
  - path resolve + pattern matching inspiration: `~/Projects/openclaw/src/infra/exec-approvals.ts`
- Semantics (MVP):
  - `AcceptForSession`: cache approval key in-memory (per thread)
  - `Execute mode`: persist per-thread + global toggles in Homie (not roci)
  - Later: “accept + add to allowlist file” (Codex execpolicy-style), backed by a Homie-owned allowlist file

### 3) Homie integration: replace Codex CLI with roci loop
Deliverables (homie-core):
- New `chat.loop.*` internal module that wraps roci agent loop and maps to Homie `chat.*` RPC/events:
  - `chat.create`, `chat.thread.read`, `chat.message.send`, `chat.cancel`
  - events: `chat.turn.*`, `chat.item.*`, `chat.message.delta`, `chat.approval.required`, `chat.token.usage.updated`
- Auth UX endpoints:
  - `chat.account.read` (per provider)
  - `chat.account.list` (all providers)
  - include: logged-in status, identity (email/account id), subscription/plan metadata when available
  - include: available models per provider/subscription
  - later: `chat.account.login.start` / callback handlers (server only)
- Auth refresh policy:
  - lazy refresh on request
  - roci normalizes auth errors; homie maps to UI-friendly status
- Provider selection per thread:
  - store provider + model + effort + permission per chat
  - UI already per-thread; keep stable

Decision: where to persist transcript
- Prefer Homie sqlite as source-of-truth for UI resume.
- roci should avoid bespoke persistence; Homie owns storage.
  - Persist normalized items (user msg, assistant msg, reasoning, tool calls/results, approvals, errors)
  - On gateway restart: rebuild roci context from Homie transcript (provider normalization belongs in roci)

### 4) Compatibility + migration
- Keep CLI runner available behind flag for fallback during rollout.
- Golden fixtures:
  - record event streams for a few scenarios; ensure stable UI behavior.

## Milestones
M1: roci auth scaffolding + Codex OAuth adapter + token store
M2: roci approvals + tool policy hooks (ask/approve flow)
M3: homie-core uses roci loop (Codex) for chat threads; CLI fallback retained
M4: add Copilot + Claude adapters; unify account status UX

## Tests
Roci:
- Live provider auth tests (ignored by default) + refresh behavior.
- Unit tests for token store, expiry, header injection.
- Approval flow tests: request -> decision -> tool run.

Homie:
- Integration: chat.create/send streams; approval round-trip.
- Regression: UI expectations for events unchanged.

## Open questions
- Provider-specific model catalogs:
  - decision: TTL default 300s (mirror Codex `DEFAULT_MODEL_CACHE_TTL`)
  - cache layers: memory + on-disk (per gateway); allow config override
  - refresh strategy options: `Online` | `Offline` | `OnlineIfUncached` (Codex pattern)
- Codex issuer details:
  - default issuer: `https://auth.openai.com` (codex-rs login default)
  - initial login: device-code endpoints under `{issuer}/api/accounts/...` (codex-rs)
  - refresh: `https://auth.openai.com/oauth/token` default (codex-rs), allow override/env if needed
- Execpolicy-style allowlisting (later):
  - plan: keep MVP semantics (`AcceptForSession`, per-thread/global execute mode)
  - future: allowlist file (Homie-owned) + optional “approve + add rule” UX (Codex-style)

## Decided (so far)
- Homie owns run queueing (per-thread/session lanes; optional global cap).
- Roci owns per-run mechanics (streaming, tool-call loop, cancel).
- Claude Code MVP: import creds from Claude Code CLI (keychain/file); auth flow later.
- GitHub Copilot MVP: github.com only (no enterprise endpoints yet).
- Codex: device-code via issuer endpoints; refresh via `auth.openai.com/oauth/token` (mirror codex-rs).
- Approvals: implement decision variants now (accept/decline/cancel + accept-for-session), propagate to Homie UI.
- Approval persistence:
  - accept-for-session = ephemeral per thread (in-memory cache; ok to lose on reconnect)
  - always approve / execute mode = persisted per thread + global toggle (survives restart)
