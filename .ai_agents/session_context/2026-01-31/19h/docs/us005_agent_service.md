# US-005: Agent Service (Codex App-Server Integration)

## Summary

Implemented the Codex app-server integration as a new `agent` module in `homie-core`. This bridges the Codex CLI's stdio JSONL protocol to the Homie WebSocket protocol, enabling AI agent chat sessions over the existing WS infrastructure.

## Files Created

- `/home/bumpyclock/Projects/remotely/crates/homie-core/src/agent/mod.rs` - Module declaration, re-exports `AgentService`
- `/home/bumpyclock/Projects/remotely/crates/homie-core/src/agent/process.rs` - `CodexProcess` manages the `codex app-server` child process (spawn, JSONL stdin/stdout communication, request correlation, event channel)
- `/home/bumpyclock/Projects/remotely/crates/homie-core/src/agent/service.rs` - `AgentService` implements `ServiceHandler` trait, maps Homie RPC methods to Codex protocol, forwards events

## Files Modified

- `/home/bumpyclock/Projects/remotely/crates/homie-core/src/lib.rs` - Added `pub mod agent` and `pub use agent::AgentService`
- `/home/bumpyclock/Projects/remotely/crates/homie-core/src/connection.rs` - Registered `AgentService` alongside `TerminalService` in message loop; cloned `outbound_tx` for both services
- `/home/bumpyclock/Projects/remotely/crates/homie-core/src/server.rs` - Registered `"agent"` in `ServiceRegistry`
- `/home/bumpyclock/Projects/remotely/LEARNINGS.md` - Added US-005 learnings entry

## Architecture

### CodexProcess (process.rs, ~260 LOC)
- Spawns `codex app-server` with stdin/stdout pipes, stderr discarded
- Background reader task reads JSONL from stdout, routes responses to pending `oneshot` waiters via HashMap<u64, oneshot::Sender>, notifications to mpsc event channel
- Background writer task serializes lines to stdin
- Request IDs are atomic u64 counters (Codex uses integer IDs)
- `initialize()` performs the Codex handshake (initialize request + initialized notification)
- `send_request()` correlates request/response via oneshot channels
- `send_response()` used for approval replies back to Codex
- `kill_on_drop(true)` ensures child cleanup

### AgentService (service.rs, ~310 LOC)
- Implements `ServiceHandler` with namespace `"agent"`
- Lazily spawns CodexProcess on first `agent.chat.create`
- RPC method mapping:
  - `agent.chat.create` -> Codex `thread/start`
  - `agent.chat.message.send` -> Codex `turn/start`
  - `agent.chat.cancel` -> Codex `turn/interrupt`
  - `agent.chat.approval.respond` -> sends response back to Codex process
- Event forwarder task maps 12 Codex notification types to Homie event topics
- Approval requests from Codex include `codex_request_id` in event params
- Backpressure via `try_send` on outbound channel (matches terminal pattern)

## Tests

- 6 unit tests in process.rs (dispatch_line routing: responses, notifications, requests, empty lines, malformed JSON, orphan responses)
- 15 unit tests in service.rs (topic mapping, param parsing, namespace, reap, unknown method handling)
- All 92 workspace tests pass, zero clippy warnings

## Key Design Decisions

1. **Lazy process spawn**: CodexProcess only starts on first `agent.chat.create`, not on connection open
2. **Integer ID correlation**: Codex uses u64 IDs, Homie uses UUIDs -- kept separate, no conversion
3. **Method string clone**: `method` param must be `.to_string()` before the async block to satisfy the borrow checker (lifetime issue with `Pin<Box<dyn Future + Send + '_>>`)
4. **outbound_tx clone**: connection.rs now clones `outbound_tx` so both terminal and agent services each get their own sender
5. **No new dependencies**: Uses only existing workspace deps (tokio::process is in tokio "full" features)

## Issues / Notes

- No integration tests with a real Codex process (would require `codex` binary on PATH)
- The event forwarder maps unknown Codex methods to a debug log and skips them -- forward-compatible
- Thread ID tracking: maintains a HashMap<chat_id, thread_id> for the Codex threadId correlation
