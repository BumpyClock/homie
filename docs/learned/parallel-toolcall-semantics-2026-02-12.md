# Parallel tool-call semantics (2026-02-12)

Scope: `remotely-5ko.7` and `remotely-5ko.8`.

## Parity notes
- Reference pattern to preserve: assistant tool-call batch should be followed by matching tool-result batch before next model step; avoid per-result re-entry loops.
- Reference guard behavior to preserve: duplicate/missing tool-result cases must be repaired/deduped at transcript boundaries.
- Codex-rs repo was not available in local workspace path during this pass; parity was validated against Homie/ROCI stream semantics and existing Responses-style finalize behavior in `src/infra/roci`.

## Homie/ROCI decisions
- Runner executes parallel-safe tools concurrently, then appends results in call order as one batch before the next provider call.
- Duplicate tool-call deltas are deduped by `tool_call_id` (latest delta wins for arguments).
- Stream-end without explicit `Done` falls back to processing gathered tool calls/results instead of failing immediately.

## QA checklist
- [x] Mixed text + parallel tool-call batch keeps a single follow-up provider request.
- [x] Duplicate tool-call deltas produce one tool execution/result for the shared id.
- [x] Stream end without `Done` still completes when tool call(s) are available.
- [x] Full `roci` test suite passes after changes.

