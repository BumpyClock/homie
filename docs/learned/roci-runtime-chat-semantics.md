---
read_when:
  - Changing Homie chat runtime or Roci integration
  - Debugging chat reconnect, cancellation, approvals, plan, or diff events
---

# Roci Runtime Chat Semantics

Homie's Roci chat path treats `roci::agent::AgentRuntime` as the source of
truth. Homie does not reconstruct chat state from raw `agent_loop` events.

Runtime state:
- Full reconnect state comes from Roci `ThreadSnapshot`.
- Incremental updates come from `AgentRuntimeEvent`.
- Homie maps snapshots/events to the existing `chat.*` and `agent.chat.*`
  WebSocket/JSON-RPC contract.

Persistence:
- `chat_thread_states.state_json` stores
  `format = "roci_agent_runtime_thread_snapshot"`.
- The persisted payload contains the semantic `ThreadSnapshot` plus separate
  `model_messages` provider ledger.
- Old Homie `RociThread` snapshots are invalidated instead of backfilled.
- `chat_raw_events` remains a Codex/debug table, not a Roci restore source.

Runtime ownership:
- Roci owns queued-turn IDs, queue serialization, cancel semantics, approvals,
  reasoning, plan, diff, tool, and message lifecycle.
- Homie owns auth, transport, storage policy, tool registry construction, and
  protocol mapping.
- Plan mode is typed through Roci `CollaborationMode::Plan`; Homie must not
  synthesize plan events from assistant text.
