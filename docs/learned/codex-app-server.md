# Codex App-Server Protocol Reference

> Sources: [Official docs](https://developers.openai.com/codex/app-server), [GitHub](https://github.com/openai/codex/tree/main/codex-rs/app-server), [Rust crate](https://docs.rs/codex-app-server-protocol/latest/codex_app_server_protocol/), [CLI reference](https://developers.openai.com/codex/cli/reference/)

## Overview

`codex app-server` is a JSON-RPC 2.0 interface for embedding Codex into products.
- **Transport**: stdio (bidirectional JSONL, one JSON object per line)
- **Protocol**: JSON-RPC 2.0, but `"jsonrpc":"2.0"` header is **omitted**
- **Schema tools**: `codex app-server generate-ts`, `codex app-server generate-json-schema`

## Spawning

```bash
codex app-server
```

Reads stdin, writes stdout. All communication is JSONL.

## Message Types

| Type | Has `id`? | Direction |
|------|-----------|-----------|
| Request | Yes | Client->Server or Server->Client |
| Response | Yes (echoes) | Mirrors request direction |
| Notification | No | Server->Client (usually) |

## Initialization Handshake (REQUIRED)

Must happen before any other call. Pre-init requests rejected.

```jsonl
{"method":"initialize","id":0,"params":{"clientInfo":{"name":"my_client","title":"My Client","version":"0.1.0"}}}
```

Server responds with `{id:0, result:{...}}` including user-agent string and `requiresOpenaiAuth` bool. Then client sends:

```jsonl
{"method":"initialized"}
```

## Core Primitives

- **Thread** = conversation (multiple turns)
- **Turn** = one user request + agent work (multiple items)
- **Item** = unit of I/O (message, command, file change, tool call, etc.)

## Thread API

| Method | Purpose |
|--------|---------|
| `thread/start` | Create new thread; params: model, cwd, approvalPolicy, sandbox |
| `thread/resume` | Reopen existing thread by ID |
| `thread/fork` | Branch from existing thread (new ID, copied history) |
| `thread/read` | Read stored thread (no load into memory) |
| `thread/list` | Paginated list; filters: sourceKinds, modelProviders, archived |
| `thread/loaded/list` | List in-memory thread IDs |
| `thread/archive` / `thread/unarchive` | Archive management |
| `thread/rollback` | Drop last N turns |
| `thread/name/set` | Set display name |

## Turn API

### Start a turn
```jsonl
{"method":"turn/start","id":30,"params":{"threadId":"thr_123","input":[{"type":"text","text":"Run tests"}]}}
```

**Input types**: `text`, `image` (URL), `localImage` (path), `skill`, `mention`

**Per-turn overrides** (become defaults for future turns on same thread):
`model`, `effort`, `cwd`, `sandboxPolicy`, `approvalPolicy`, `summary`, `personality`

`outputSchema` applies only to current turn.

### Interrupt a turn
```jsonl
{"method":"turn/interrupt","id":31,"params":{"threadId":"thr_123","turnId":"turn_456"}}
```

Turn finishes with `status: "interrupted"`.

## Event/Notification Types (Server -> Client)

### Turn lifecycle
- `turn/started` - turn initiated
- `turn/completed` - final state: `completed` | `interrupted` | `failed`
- `turn/diff/updated` - aggregated unified diff
- `turn/plan/updated` - plan steps with status (pending/inProgress/completed)
- `thread/tokenUsage/updated` - token usage for active thread

### Item lifecycle
- `item/started` - full item when work begins (use item.id to correlate deltas)
- `item/completed` - authoritative final state

### Streaming deltas
- `item/agentMessage/delta` - streamed agent text (concatenate in order)
- `item/plan/delta` - streamed plan text
- `item/reasoning/summaryTextDelta` - readable reasoning summaries; summaryIndex increments per section
- `item/reasoning/summaryPartAdded` - section boundary marker
- `item/reasoning/textDelta` - raw reasoning (model-dependent)
- `item/commandExecution/outputDelta` - stdout/stderr streaming
- `item/fileChange/outputDelta` - apply_patch tool response

### Auth notifications
- `account/login/completed` - login attempt finished
- `account/updated` - auth mode changed (apikey/chatgpt/null)
- `account/rateLimits/updated` - rate limits changed

## Item Types

- `userMessage` - user input content
- `agentMessage` - agent reply text
- `plan` - proposed plan
- `reasoning` - summary + content
- `commandExecution` - command, cwd, status, exitCode, durationMs
- `fileChange` - changes [{path, kind, diff}], status
- `mcpToolCall` - server, tool, arguments, result/error
- `collabToolCall` - collaboration between threads
- `webSearch` - search query + action
- `imageView` - path to viewed image
- `enteredReviewMode` / `exitedReviewMode` - review lifecycle
- `contextCompaction` - history compaction marker

## Approval Flow (Server -> Client requests)

When commands/file changes need approval, server sends a **request** (has `id`). Client must respond.

### Command execution approval
1. `item/started` (pending commandExecution)
2. `item/commandExecution/requestApproval` - includes itemId, threadId, turnId, reason, risk, parsedCmd
3. Client responds: `{"id":<id>,"result":{"decision":"accept"|"decline","acceptSettings":{...}}}`
4. `item/completed` with final status

### File change approval
1. `item/started` (fileChange with proposed changes)
2. `item/fileChange/requestApproval` - includes itemId, threadId, turnId, reason
3. Client responds with accept/decline
4. `item/completed`

### MCP tool-call approval
- `tool/requestUserInput` with options (Accept/Decline/Cancel)

## Authentication

| Method | Type |
|--------|------|
| `account/read` | Check current auth state |
| `account/login/start` | Start login (apiKey, chatgpt, chatgptAuthTokens) |
| `account/login/cancel` | Cancel pending login |
| `account/logout` | Log out |
| `account/rateLimits/read` | Read rate limits |

### API key login
```jsonl
{"method":"account/login/start","id":2,"params":{"type":"apiKey","apiKey":"sk-..."}}
```

### External auth tokens (for host apps)
```jsonl
{"method":"account/login/start","id":7,"params":{"type":"chatgptAuthTokens","idToken":"<jwt>","accessToken":"<jwt>"}}
```

Server requests refresh via `account/chatgptAuthTokens/refresh` on 401.

## Other APIs

| Method | Purpose |
|--------|---------|
| `command/exec` | Run single command without thread (sandbox applies) |
| `model/list` | List available models |
| `skills/list` | List skills for given cwds |
| `skills/config/write` | Enable/disable skill |
| `app/list` | List available apps/connectors |
| `review/start` | Start automated code review |
| `config/read` | Read effective config |
| `config/value/write` | Write single config key |
| `config/batchWrite` | Atomic multi-key write |
| `mcpServer/oauth/login` | MCP server OAuth |
| `mcpServerStatus/list` | List MCP servers + tools |
| `config/mcpServer/reload` | Reload MCP config from disk |
| `feedback/upload` | Submit feedback |

## Error Handling

Turn failures include `codexErrorInfo`:
- `ContextWindowExceeded`
- `UsageLimitExceeded`
- `HttpConnectionFailed`
- `ResponseStreamDisconnected` (may include httpStatusCode)
- `ResponseTooManyFailedAttempts`
- `BadRequest`, `Unauthorized`, `SandboxError`, `InternalServerError`, `Other`

## Sandbox Policies

- `workspaceWrite` - write within workspace + writable roots
- `externalSandbox` - delegate to external sandbox; networkAccess: "restricted"|"enabled"
- Others: networkAccess is boolean

## App-Server v2 Deprecation

v2 events were deprecated late 2025. Current (v3) protocol uses the notification types listed above.
