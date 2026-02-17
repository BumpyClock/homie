# Provider auth

Provider authentication is managed from the **Settings** panel in both web and mobile clients.
Homie supports three providers; two use OAuth device-code flow and one imports credentials from an existing CLI.

## Supported providers

| Provider | Auth method | Notes |
|----------|-------------|-------|
| `openai-codex` | Device code (OAuth) | RFC 8628 device authorization grant |
| `github-copilot` | Device code (OAuth) | RFC 8628 device authorization grant |
| `claude-code` | CLI import only | Reads existing Claude Code CLI credentials |

## Settings UI flow

### Web

1. Click the **gear icon** in the gateway header (top right).
2. Select the **Provider Accounts** tab in the settings panel.
3. Find the provider and click **Connect**.
4. A device-code card expands inline with a verification URL and user code.
5. Open the URL, enter the code, and approve access.
6. The status pill updates to **Connected** once authorized.

### Mobile

1. Open the **Settings** tab.
2. Switch to the **Providers** segment.
3. Tap **Connect** on the provider row.
4. A device-code card appears with a verification URL and code.
5. Tap the URL to open it in your browser, enter the code, and approve.
6. The provider status updates to **Connected**.

## How it works

Provider login uses the **RFC 8628 device authorization grant**:

1. Client calls `chat.account.login.start` with the provider ID.
2. Server returns a `verification_url`, `user_code`, `device_code`, and polling interval.
3. User opens the URL, enters the code, and authorizes.
4. Client polls `chat.account.login.poll` at the given interval.
5. Server responds with `pending`, `slow_down`, `authorized`, `denied`, or `expired`.
6. On `authorized`, credentials are stored and `onAuthorized` is called.

The shared `useProviderAuth` hook (`src/apps/shared/src/hooks/useProviderAuth.ts`) implements the state machine used by both web and mobile. See `docs/design/shared-auth-architecture.md` for details.

## Credential storage

Default location:
- `~/.homie/credentials`

Override via config:
- `[paths].credentials_dir` in `~/.homie/config.toml`

## Important notes

- `gh auth login` does **not** populate Homie provider credentials. You must use the Homie device-code flow for `github-copilot`.
- Provider IDs accept both dash and underscore forms: `openai-codex` / `openai_codex`, `github-copilot` / `github_copilot`.
- Providers must be enabled in `~/.homie/config.toml` (e.g., `[providers.openai_codex].enabled = true`).

## Claude-code (CLI import)

`claude-code` does not support device-code login. It imports credentials from the Claude Code CLI.

1. Ensure config:
   - `[providers.claude_code].enabled = true`
   - `[providers.claude_code].import_from_cli = true`

2. Complete login in the Claude Code CLI.

3. Trigger import by calling `chat.account.list` or `chat.account.read` (Settings does this automatically).

4. The provider status updates to **Imported** in Settings.

If you call device-code endpoints for `claude-code`, the server returns:
- `claude-code does not support device-code login`

## Troubleshooting

### Advanced: manual CLI flow (wscat)

For debugging auth outside of web/mobile clients, you can drive the flow manually via `wscat`.

1. Start gateway:
```bash
cargo run -p homie-gateway
```

2. Connect:
```bash
npx wscat -c ws://127.0.0.1:9800/ws
```

3. Send handshake (must be the first frame):
```json
{"protocol":{"min":1,"max":1},"client_id":"manual-cli/0.1.0","capabilities":["chat"]}
```

Expected response: `{"type":"hello","protocol_version":1,...}`

4. Check auth status:
```json
{"type":"request","id":"1","method":"chat.account.list"}
```

5. Start device-code login:
```json
{"type":"request","id":"2","method":"chat.account.login.start","params":{"provider":"github-copilot","profile":"default"}}
```

Response includes `session` with `verification_url`, `user_code`, `device_code`, `interval_secs`, `expires_at`. Open the URL and enter the code.

6. Poll until authorized:
```json
{"type":"request","id":"3","method":"chat.account.login.poll","params":{"provider":"github-copilot","profile":"default","session":{"verification_url":"...","user_code":"...","device_code":"...","interval_secs":5,"expires_at":"..."}}}
```

Poll statuses: `pending`, `slow_down`, `authorized`, `denied`, `expired`. Stop on `authorized`.

7. Confirm login:
```json
{"type":"request","id":"4","method":"chat.account.list"}
{"type":"request","id":"5","method":"chat.model.list"}
```

Provider should show `logged_in: true` and models should include provider-prefixed IDs.

> **Note**: If you send RPC before the handshake, the server rejects with `invalid handshake: missing field 'protocol'`.
