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

### 2) Extend roci: agent loop primitives (OpenClaw-ish)
Deliverables (roci):
- Session manager that persists:
  - session metadata (model, settings, tool policy, permission mode)
  - transcript (turns, items, deltas)
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

### 3) Homie integration: replace Codex CLI with roci loop
Deliverables (homie-core):
- New `chat.loop.*` internal module that wraps roci agent loop and maps to Homie `chat.*` RPC/events:
  - `chat.create`, `chat.thread.read`, `chat.message.send`, `chat.cancel`
  - events: `chat.turn.*`, `chat.item.*`, `chat.message.delta`, `chat.approval.required`, `chat.token.usage.updated`
- Auth UX endpoints:
  - `chat.account.read` (per provider)
  - later: `chat.account.login.start` / callback handlers (server only)
- Provider selection per thread:
  - store provider + model + effort + permission per chat
  - UI already per-thread; keep stable

Decision: where to persist transcript
- Prefer Homie sqlite as source-of-truth for UI resume.
- roci can provide an interface, but Homie owns storage for now; keep minimal duplication.

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
- Exact OAuth flows for Copilot + Claude Code on headless hosts (device code vs browser).
- Whether to store tokens in `~/.homie` vs roci default; likely roci supports custom path, Homie sets it.
- Provider-specific model catalogs + allowlists (source + caching).
