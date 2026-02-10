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

## Homie config schema (MVP)
- Config file (gateway-owned): `~/.homie/config.toml`
  - cross-platform home resolution (no `~` expansion on Windows):
    - env override: `HOMIE_HOME` (absolute path)
    - else: use OS directory crates for reliability (preferred), then env fallback:
      - `directories`/`dirs` crate → `HOME` (unix) → `USERPROFILE` (windows) → `HOMEDRIVE`+`HOMEPATH` fallback

Proposed schema (v1):
```toml
version = 1

[debug]
# NOTE: support both for now (historical typo in docs/commands)
homie_debug_env = "HOMIE_DEBUG"
home_debug_env = "HOME_DEBUG"

# Persist raw provider events into the Homie DB (for debugging only).
# Recommended: only enable when HOMIE_DEBUG/HOME_DEBUG=1.
persist_raw_provider_events = false

[models]
# Default TTL for provider model catalogs.
catalog_ttl_secs = 300

[providers.openai_codex]
enabled = true
# Device-code issuer base (codex-rs pattern).
issuer = "https://auth.openai.com"
# Refresh endpoint override (codex-rs supports override env); keep configurable in Homie too.
refresh_token_url_override = ""

[providers.github_copilot]
enabled = true
# MVP: github.com only (no enterprise yet).
github_host = "github.com"
device_code_url = "https://github.com/login/device/code"
token_url = "https://github.com/login/oauth/access_token"
# GitHub Copilot token exchange:
copilot_token_url = "https://api.github.com/copilot_internal/v2/token"

[providers.claude_code]
enabled = true
# MVP: import creds from Claude Code CLI; no OAuth flow yet.
import_from_cli = true

[paths]
# Optional explicit overrides. If empty, derived under homie_home_dir().
credentials_dir = "" # defaults to ~/.homie/credentials
execpolicy_path = "" # defaults to ~/.homie/execpolicy.toml
```

Notes:
- Provider/model settings are gateway defaults. Per-thread settings live in Homie DB and can change mid-thread; changes apply next user message (Codex behavior).
- Debug raw event persistence:
  - only when `HOMIE_DEBUG=1` or `HOME_DEBUG=1` (or config flag forced on)
  - MVP: no redaction; treat as unsafe (may contain secrets); do not enable by default
  - retention: keep raw events for last 10 runs (drop older raw blobs)

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
  - Windows: skip keychain; file-only import from `$CODEX_HOME/auth.json`
- `github_copilot`:
  - device-code = GitHub token (`openclaw/src/providers/github-copilot-auth.ts`)
  - exchange GitHub token -> Copilot token + base URL derivation (`openclaw/src/providers/github-copilot-token.ts`)
  - cache Copilot token w/ expiry; refresh when near expiry (keep 5m safety window)
- `claude_code`:
  - investigate device-code viability (unknown)
  - MVP fallback: import Claude Code creds from keychain/file like OpenClaw (`src/agents/cli-credentials.ts`)
  - optional: write-back refreshed Claude tokens to external store (OpenClaw has `writeClaudeCliCredentials(...)`)
  - Claude Code file location (all OS): `$USERHOME/.claude/`

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

### 2) Tools architecture + essentials (Phase 2)
Goal: mirror codex‑rs tool organization while keeping OpenClaw‑style dynamic loading.

Codex‑rs patterns to mirror:
- Tool specs separate from handlers (`core/src/tools/spec.rs` + `handlers/*`)
- Registry/builder assembles tools by features/model flags
- Dynamic tools surfaced via protocol `dynamic_tools` + MCP tools

Homie plan:
- **ToolSpec + ToolRegistry** (core):
  - `ToolSpec { name, description, json_schema, supports_parallel, category }`
  - `ToolHandler { name -> async fn(args, ctx) }`
  - `ToolRegistryBuilder` assembles enabled tools for a run.
- **Essential local tools** (MVP, Homie‑core):
  - `read`, `ls`, `find`, `grep`, `apply_patch`, `exec`, `process`
  - Strict JSON schema; add tolerant defaults for missing args to avoid tool‑loop failures.
- **Dynamic tool loading (future‑proof now)**:
  - `ToolProvider` trait in Homie core (not roci):
    - `list_specs() -> Vec<ToolSpec>`
    - `call(name, args, ctx) -> ToolResult`
  - Built‑in providers: local tools + (future) MCP + plugin tools.
  - Config‑driven enable/disable by name + allowlist (OpenClaw‑style).

Immediate tasks:
- Audit current tool schemas vs codex‑rs (names, required fields, strictness).
- Decide tool naming/aliases for Codex compatibility.
- Add tests for schema + call flow.

Next after essentials:
- `web.fetch`, `web.search` (OpenClaw‑inspired).

OpenClaw extras (future; dynamic tools by channel):
- browser (page nav/snapshot)
- canvas (render/eval/snapshot)
- nodes (device list/describe/notify/screen/camera)
- cron (schedule reminders / wake)
- sessions_list / sessions_history / sessions_send
- notifications (system + push)
- screenshot (capture + annotate)
- audio (transcribe/tts) — optional
Plan: keep core tools always-on; load channel-specific tools via ToolProvider config.
Next note: add `tools.providers.<id>.channels` allowlist (empty/missing = all channels) and gate OpenClaw extras (`openclaw_browser`, then canvas/nodes/cron) on active chat channel.

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

Execpolicy storage (decision):
- Single global file: `~/.homie/execpolicy.toml`
  - cross-platform: resolve home dir explicitly (Windows has no `~` expansion)
  - implement `homie_home_dir()` helper used by config/credentials/execpolicy (and any future per-user storage):
    - env override (MVP): `HOMIE_HOME` (absolute path)
    - else: prefer OS directory crates, then env fallback (`HOME` / `USERPROFILE` / `HOMEDRIVE`+`HOMEPATH`)
  - store absolute paths on disk; UI can render as `~` for display only
  - avoid logging/storing secrets inside execpolicy file

Execpolicy file format (MVP):
- Goal: allow both exact argv and glob-like rules (`gh *`, `npm test *`, etc).
- Reference: Claude Code `~/.claude/settings.json` patterns like `Bash(gh:*)`, `Bash(git add:*)`, and exact command strings (no `:*`).
- Matching model:
  - evaluate against the *tokenized argv* (not a shell string)
  - per-token glob patterns: `*` matches any chars within a single argv token
  - optional: `**` matches remaining argv tokens (greedy)

```toml
version = 1

[[rule]]
id = "gh-any"
effect = "allow"
argv_glob = ["gh", "*"]

[[rule]]
id = "npm-test-any"
effect = "allow"
argv_glob = ["npm", "test", "*"]

[[rule]]
id = "git-status-exact"
effect = "allow"
argv_exact = ["git", "status"]

# Optional shorthand (Claude-like):
# - `"gh:*"` parses as `["gh","*"]`
# - `"git add:*"` parses as `["git","add","*"]`
[[rule]]
id = "gh-any-shorthand"
effect = "allow"
argv_shorthand = "gh:*"
```

Notes:
- Later: add `cwd_glob`, `sandbox_permissions`, `tty`, OS scoping, and “deny” rules.
- Cross-platform: Windows argv normalization + exe resolution must be consistent with tool runtime.
- Implementation note: parse `argv_shorthand` via shell-words splitting (to handle quotes), then normalize into the same internal token matcher as `argv_glob`/`argv_exact`.
- Windows matching: default to case-insensitive comparisons (paths + argv tokens) unless explicitly configured otherwise.

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
  - allow switching mid-thread; applies next user message (Codex behavior)

Decision: where to persist transcript
- Prefer Homie sqlite as source-of-truth for UI resume.
- roci should avoid bespoke persistence; Homie owns storage.
  - Persist normalized items (user msg, assistant msg, reasoning, tool calls/results, approvals, errors)
  - Also persist `raw_provider_event_json` alongside normalized items (debuggable, optional):
    - enabled only when `HOMIE_DEBUG=1` or `HOME_DEBUG=1` (or config flag forced on)
    - MVP: no redaction; treat as unsafe (may contain secrets); do not enable by default
    - retention: keep last 10 runs globally; drop older raw blobs
    - optional: size cap (ex: 64KB per raw blob) + truncate to bound DB growth
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

## Next phase execution slice (current)
Goal: finish production-safe core tool stack, then move to OpenClaw extras via dynamic providers.

### Slice A: Core tools hardening (do now)
Scope:
- Complete local core tools quality bar:
  - `read`, `ls`, `find`, `grep`, `apply_patch`, `exec`, `process`
  - `web_fetch`, `web_search`
- Argument tolerance:
  - accept missing/partial args with safe defaults
  - normalize aliases and types before handler dispatch
- Failure handling:
  - no panics; structured tool errors only
  - cap tool-loop retries; emit deterministic terminal failure event
- Output policy:
  - keep truncated output in prompt-context window only (last N turns)
  - preserve full output in persisted run items for diagnostics
Acceptance gates:
- live provider tests pass for:
  - plain response
  - tool call success (`ls` minimum)
  - web tool success (`web_search` and `web_fetch`) when configured
- no runaway UI polling loops after tool failures
- no `unwrap()` panics in runner/tool path

Status update (2026-02-09):
- Completed:
  - `chat.tools.list` backend endpoint (Roci + Codex proxy path)
  - Web UI wiring for tool availability (shows `web_fetch` / `web_search` status)
  - Live env-gated integration tests: `ls`, `web_search`, `web_fetch`
  - Dynamic provider scaffold: `openclaw_browser` (disabled by default)

### Slice B: Dynamic tool loading foundation (do next)
Scope:
- Implement `ToolProvider` loading graph in Homie core:
  - static always-on provider (core tools)
  - optional providers from config (disabled by default)
- Add provider registration contract:
  - `provider_id`, `channel_tags`, `tools`, `enabled`
  - startup validation + conflict detection on tool name collisions
- Config shape:
  - explicit enable per provider/tool
  - default deny for unknown providers/tools
Acceptance gates:
- core provider unchanged behavior
- enabling/disabling optional provider requires no code change
- provider conflicts fail fast with actionable config error

### Slice C: OpenClaw extras onboarding (after A+B)
Initial extras (first batch):
- `browser`, `canvas`, `nodes`, `cron`, `sessions_*`
Approach:
- port one provider at a time behind config flags
- add integration tests per provider (contract + happy path + failure path)
- map each extra into existing `chat.item.*` event model, no UI protocol break

### Out of scope for this slice
- keyring encryption at rest
- enterprise GitHub host support
- full plugin marketplace / remote untrusted providers

## Subagent execution strategy (parallel-first)
Goal: maximize parallel work while minimizing merge conflicts.

### Orchestrator rules
- Create one subagent per lane with explicit file ownership.
- Subagents must not edit outside owned paths.
- Integrate in dependency order at sync points only.
- Run lane-local tests before merge; run full smoke suite after each sync point.

### Lane map
1. Lane A — Tool argument normalization + tolerant parsing
   Ownership:
   - `src/core/src/agent/tools/*`
   - `src/core/src/agent/tools/registry.rs`
   Deliverables:
   - normalize missing args/defaults for core + web tools
   - alias/type coercion where safe
   Gate:
   - unit tests for each tool arg parser

2. Lane B — Runner reliability + loop bounds
   Ownership:
   - `src/infra/roci/src/agent_loop/*`
   - `src/core/src/agent/roci_backend.rs` (loop integration only)
   Deliverables:
   - bounded retries for repeated tool failures
   - panic-free error path, deterministic terminal events
   - stale-run cleanup and cancellation robustness
   Gate:
   - runner tests: no `unwrap` panics, bounded-failure behavior

3. Lane C — Web tools behavior + integration
   Ownership:
   - `src/core/src/agent/tools/web.rs`
   - `src/core/src/homie_config.rs` (web tool config bits only)
   Deliverables:
   - stable Firecrawl/SearXNG/Brave execution paths
   - uniform result envelope + clear error semantics
   Gate:
   - tool integration tests with mocked + live endpoints (env-gated)

4. Lane D — Dynamic tool provider foundation
   Ownership:
   - `src/core/src/agent/tools/*provider*`
   - `src/core/src/agent/tools/registry.rs`
   - `src/core/src/homie_config.rs` (provider enable/disable schema)
   Deliverables:
   - `ToolProvider` loading graph
   - provider/tool enable toggles, conflict detection
   Gate:
   - registry tests for enable/disable + collision failures

5. Lane E — UI/tool event rendering + polling stability
   Ownership:
   - `src/web/src/hooks/use-chat.ts`
   - `src/web/src/components/chat-turns.tsx`
   - `src/web/src/lib/chat-utils.ts`
   Deliverables:
   - render tool outputs/errors consistently (core + web)
   - prevent polling storms on failure
   Gate:
   - UI smoke checks for chat send/tool failure/retry states

### Sync points
- Sync 1 (A + B + C):
  - merge arg normalization, runner stability, web tool behavior
  - run live provider smoke (`HOMIE_LIVE_TESTS=1`) + core unit tests
- Sync 2 (D):
  - merge dynamic provider foundation after Sync 1 green
  - re-run full tool and chat smoke tests
- Sync 3 (E):
  - merge UI stabilization + tool rendering last
  - browser smoke + gateway logs verification

### Suggested subagent dispatch order
1. Start A, B, C in parallel.
2. Start D once A shape is stable (registry interfaces finalized).
3. Start E after Sync 1 event/result schemas are stable.

### Risk controls
- Freeze shared contracts before parallel coding:
  - tool result envelope
  - runner terminal event shape
  - provider registration interface
- If contract changes mid-lane, stop and rebroadcast interface diff before continuing.

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

## Debug logging (build-time requirement)
- Use `tracing` structured logs; ensure all chat/agent-loop paths include:
  - `chat_id`, `thread_id`, `turn_id`, `item_id`, `run_id`, `provider`, `model`
  - request ids for RPC + approval ids
- Redaction rules:
  - never log access/refresh tokens, auth codes, PKCE verifier, full headers
  - log safe summaries (provider, expiry timestamps, success/failure, error category)
- Feature flags:
  - `HOMIE_DEBUG=1` increases log verbosity + enables raw provider event persistence (if desired)

## Tool runtime defaults (cross-platform)
- Default shell selection:
  - Windows: `pwsh` → `powershell` → `cmd`
  - Non-Windows: prefer user login shell, fallback `/bin/bash` (existing behavior)
