# Provider auth runbook

This runbook describes how to log providers in via Homie RPC.

## Supported providers
- `openai-codex` (device-code)
- `github-copilot` (device-code)
- `claude-code` (no device-code; CLI import only)

## Manual CLI flow (`wscat`)
If you are authenticating from terminal (outside web/mobile clients), use this exact order.

1) Start gateway:
```bash
cargo run -p homie-gateway
```

2) Connect:
```bash
npx wscat -c ws://127.0.0.1:9800/ws
```

3) Send handshake as the first frame:
```json
{"protocol":{"min":1,"max":1},"client_id":"manual-cli/0.1.0","capabilities":["chat"]}
```

Expected response:
```json
{"type":"hello","protocol_version":1,"server_id":"homie-gateway/...","services":[...]}
```

4) Only after `type:"hello"`, send RPC frames with `type:"request"`:
```json
{"type":"request","id":"9f9eaa20-28e8-4a53-afbe-f914af6c1f3b","method":"chat.account.list"}
```

If you send RPC before handshake, server rejects with:
- `invalid handshake: missing field 'protocol'`

## 0) Verify provider is enabled
Check `~/.homie/config.toml`:
- `[providers.openai_codex].enabled = true`
- `[providers.github_copilot].enabled = true`

## 1) Inspect auth status
RPC call:
```json
{"type":"request","id":"e0932682-31eb-4d80-915d-d4d3276f7688","method":"chat.account.list"}
```

Expected response:
- `providers[]` with `id`, `enabled`, `logged_in`
- optional: `expires_at`, `scopes`, `has_refresh_token`

## 2) Start device-code login
RPC call:
```json
{
  "type":"request",
  "id":"e4dbf68d-1046-4d42-8fd3-e7ecf7d413c9",
  "method":"chat.account.login.start",
  "params": {
    "provider": "github-copilot",
    "profile": "default"
  }
}
```

OpenAI Codex example:
```json
{
  "type":"request",
  "id":"e4dbf68d-1046-4d42-8fd3-e7ecf7d413ca",
  "method":"chat.account.login.start",
  "params": {
    "provider": "openai-codex",
    "profile": "default"
  }
}
```

Provider aliases accepted:
- `openai-codex` / `openai_codex`
- `github-copilot` / `github_copilot`

Response includes `session`:
- `verification_url`
- `user_code`
- `device_code`
- `interval_secs`
- `expires_at`

Open `verification_url`, enter `user_code`, approve.

## 3) Poll until authorized
RPC call:
```json
{
  "type":"request",
  "id":"2a22f414-a79f-4897-9f68-9a3b674f5bd6",
  "method":"chat.account.login.poll",
  "params": {
    "provider": "github-copilot",
    "profile": "default",
    "session": {
      "verification_url": "...",
      "user_code": "...",
      "device_code": "...",
      "interval_secs": 5,
      "expires_at": "2026-02-11T22:10:00Z"
    }
  }
}
```

OpenAI Codex poll example:
```json
{
  "type":"request",
  "id":"2a22f414-a79f-4897-9f68-9a3b674f5bd7",
  "method":"chat.account.login.poll",
  "params": {
    "provider": "openai-codex",
    "profile": "default",
    "session": {
      "verification_url": "...",
      "user_code": "...",
      "device_code": "...",
      "interval_secs": 5,
      "expires_at": "2026-02-11T22:10:00Z"
    }
  }
}
```

Poll statuses:
- `pending`
- `slow_down`
- `authorized`
- `denied`
- `expired`

Stop when `authorized`.

## 4) Confirm login and model availability
RPC calls:
```json
{"type":"request","id":"2728f40e-bd79-4d90-ae7f-11b62f77939a","method":"chat.account.list"}
{"type":"request","id":"bb95a7be-058f-42fd-b767-8f0d1d5f55b7","method":"chat.model.list"}
```

Expect:
- provider shows `logged_in: true`
- models include provider-prefixed ids (for Copilot: `github-copilot:<model>`)

## Credential storage
Default:
- `~/.homie/credentials`

Override:
- `[paths].credentials_dir` in `~/.homie/config.toml`

## Important notes
- `gh auth login` alone does **not** populate Homie provider credentials.
- You must run Homie device-code flow (`chat.account.login.start` + `chat.account.login.poll`) for `github-copilot`.
- `claude-code` login is imported from CLI creds (`providers.claude_code.import_from_cli = true`).
- For `claude-code`, complete login in Claude Code CLI, then restart gateway and re-check `chat.account.list`.

## Claude-code trigger flow (CLI import)
`claude-code` does not support device-code RPC login.

1) Ensure config:
- `[providers.claude_code].enabled = true`
- `[providers.claude_code].import_from_cli = true`

2) Complete login in Claude Code CLI.

3) Trigger import by calling either:
```json
{"type":"request","id":"cd0618d2-fd1c-47a9-a212-fefb43db602e","method":"chat.account.list"}
```
or
```json
{"type":"request","id":"f6d9f48e-29d4-4336-a26f-e95dd71fa6f4","method":"chat.account.read"}
```

4) Verify provider status is logged in:
```json
{"type":"request","id":"f9c3f8e0-f1b7-4bc0-b2f4-2c67f6da3f5e","method":"chat.account.list"}
```

If you call device-code endpoints for `claude-code`, expected error:
- `claude-code does not support device-code login`
