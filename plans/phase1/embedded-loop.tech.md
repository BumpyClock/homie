# Embedded Agent Loop — Tech Plan

## Scope
Swap Codex CLI app-server with an embedded loop while keeping the `chat.*` API stable. Retain Codex OAuth, add provider abstraction for Claude/Copilot later.

## Architecture
### Current
- Gateway spawns `codex app-server` (JSONL over stdin/stdout).
- `AgentService` bridges `chat.*` ↔ Codex app-server.

### Target
- Replace process spawn with **in-process runner** from codex-rs.
- Keep a **protocol adapter** that emits the same event payloads currently used by the UI.
- Maintain `chat.*` contract in homie-core.

## Components
1) **CodexRunner trait** (homie-core)
   - `create_thread`, `send_message`, `cancel`, `read_thread`, `subscribe_events`.
   - Two impls:
     - `CliAppServerRunner` (fallback/feature flag).
     - `EmbeddedCodexRunner` (primary).

2) **Event Adapter**
   - Normalize codex-rs events → existing `chat.*` events.
   - Ensure `turn_id` + item ids always present.

3) **OAuth Store**
   - Use codex-rs OAuth flow + token refresh.
   - Store under `~/.homie/codex/` (or compat alias).
   - Expose `chat.account.read` + `chat.account.refresh`.

4) **Session Persistence**
   - Keep homie DB as source of truth.
   - Store provider thread id in chat metadata.
   - `chat.thread.read` reconstructs items from stored transcript + provider read.

5) **Background Runner**
   - Ephemeral embedded loop for title generation + other automation.
   - Not persisted; spin up on demand and discard.

## Data Flow
1. `chat.message.send`
2. AgentService calls EmbeddedCodexRunner.
3. Embedded runner streams events → adapter → `chat.*` event bus.
4. Persistence layer writes turn + items.

## API Contract (unchanged)
- `chat.create`, `chat.thread.read`, `chat.message.send`, `chat.cancel`
- `chat.approval.required`, `chat.approval.respond`
- `chat.account.read` (login status)

## Storage
- `~/.homie/` as global config root.
- `~/.homie/codex/` for OAuth tokens + codex settings.
- Homie sqlite for chat metadata, turns, items.

## Configuration
- `HOMIE_AGENT_BACKEND=embedded|cli` (default embedded).
- `HOMIE_CODEX_DIR=~/.homie/codex` (override).

## Testing
- Integration tests:
  - Embedded runner: create → send → stream → complete.
  - Approval round-trip.
  - `chat.account.read` with/without OAuth tokens.
- Keep existing CLI runner tests as fallback.

## Migration Plan
1) Introduce `CodexRunner` trait + CLI runner behind it.
2) Implement embedded runner with adapter.
3) Toggle default to embedded; keep env to force CLI.
4) Remove CLI fallback when embedded proven stable.

## Future Provider Abstraction
- Provider registry: `codex`, `claude`, `copilot`.
- Shared loop interface + provider-specific auth + model catalog.
- UI surface remains `chat.*`, with provider metadata in settings.
