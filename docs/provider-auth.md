# Provider auth runbook

This runbook describes how to log providers in via Homie RPC.

## Supported providers
- `openai-codex` (device-code)
- `github-copilot` (device-code)
- `claude-code` (no device-code; CLI import only)

## 0) Verify provider is enabled
Check `~/.homie/config.toml`:
- `[providers.openai_codex].enabled = true`
- `[providers.github_copilot].enabled = true`

## 1) Inspect auth status
RPC call:
```json
{"id":"1","method":"chat.account.list"}
```

Expected response:
- `providers[]` with `id`, `enabled`, `logged_in`
- optional: `expires_at`, `scopes`, `has_refresh_token`

## 2) Start device-code login
RPC call:
```json
{
  "id":"2",
  "method":"chat.account.login.start",
  "params": {
    "provider": "github-copilot",
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
  "id":"3",
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
{"id":"4","method":"chat.account.list"}
{"id":"5","method":"chat.model.list"}
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
