---
summary: "Local bootstrapping and run commands for gateway, web, and mobile"
read_when: "Before changing local dev setup, startup commands, or onboarding flow"
---

# Homie quick start

## Prereqs
- Rust (stable) + cargo
- Node.js 20+ + pnpm (corepack)
- iOS/Android toolchain for mobile (optional)

## Install
```bash
pnpm install
./scripts/install-git-hooks.sh
```

## Configure
```bash
mkdir -p ~/.homie
cp config.toml.example ~/.homie/config.toml
```

Optional local models (vLLM / OpenAI-compatible):
```toml
[providers.openai_compatible]
enabled = true
base_url = "http://127.0.0.1:8000/v1"
api_key = ""
models = []
```

## Run gateway
```bash
cargo run -p homie-gateway
```
Default bind: `127.0.0.1:9800` (sqlite at `homie.db` in repo root).

Allow LAN (optional):
```bash
HOMIE_BIND=0.0.0.0:9800 HOMIE_ALLOW_LAN=1 cargo run -p homie-gateway
```
Use your machine's LAN IP in `VITE_GATEWAY_URL` / `EXPO_PUBLIC_HOMIE_GATEWAY_URL`.

Tailscale Serve (optional, remote access):
```bash
HOMIE_TAILSCALE_SERVE=1 cargo run -p homie-gateway
```
`HOMIE_TAILSCALE=1` remains an alias for compatibility and runs the same startup path.

This starts `tailscale serve` for Homie and exposes the websocket endpoint via your Tailscale hostname.

## Canonical local dev workflow (tmux)

Use one `tmux` session and one window per service:

- `gateway` window: `cargo run -p homie-gateway`
  - Gateway websocket endpoint: `ws://127.0.0.1:9800/ws`
  - Health/debug UI: web and mobile target this ws URL in dev by default.
- `web` window: `VITE_GATEWAY_URL=ws://127.0.0.1:9800/ws pnpm dev`
  - Web served on Vite default `http://127.0.0.1:5173`.
- `mobile` window: `EXPO_PUBLIC_HOMIE_GATEWAY_URL=ws://127.0.0.1:9800/ws pnpm dev:mobile`
  - Expo Metro/bundler URL printed in terminal (commonly `http://127.0.0.1:8081` + tunnel entry points).

Example:
```bash
tmux new-session -d -s homie -n gateway 'cargo run -p homie-gateway'
tmux new-window -t homie: -n web 'VITE_GATEWAY_URL=ws://127.0.0.1:9800/ws pnpm dev'
tmux new-window -t homie: -n mobile 'EXPO_PUBLIC_HOMIE_GATEWAY_URL=ws://127.0.0.1:9800/ws pnpm dev:mobile'
tmux attach -t homie
```

Physical-device caveat:
- Do not use `127.0.0.1` on a phone or tablet.
- Use host LAN IP (same Wi-Fi subnet) in gateway + clients, or a Tailscale endpoint for remote access.
  - LAN: `HOMIE_ALLOW_LAN=1 HOMIE_BIND=0.0.0.0:9800 cargo run -p homie-gateway`
  - Tailscale: use `HOMIE_TAILSCALE_SERVE=1` (or `HOMIE_TAILSCALE=1`) + the routed host endpoint shown by your setup and point clients at `wss://...`.

## Artifact policy (tmux + tasking)

- `ai_agents_session_context/` is ignored in root `.gitignore` (local agent context cache).
- Keep `.tasque/config.json` as durable local config.
- Ignore `.tasque/events.jsonl` in `.tasque/.gitignore` (session event log; regenerated per session).

## Run web
```bash
VITE_GATEWAY_URL=ws://127.0.0.1:9800/ws pnpm dev
```
If unset in dev, web defaults to `ws://127.0.0.1:9800/ws`.

## Run mobile
```bash
EXPO_PUBLIC_HOMIE_GATEWAY_URL=ws://127.0.0.1:9800/ws pnpm dev:mobile
```
Notes:
- First launch requires saving a gateway target in Settings.
- `EXPO_PUBLIC_HOMIE_GATEWAY_URL` is a prefill hint only.
- On a physical device, use your LAN IP (not `127.0.0.1`).

Mobile tests:
```bash
pnpm --filter mobile test
```

## Shared transport/API semantics
- Web and mobile use `@homie/shared` for gateway transport.
- Client hello is the first websocket frame; JSON-RPC requests are sent only after the server returns `{"type":"hello",...}`.
- Approval buttons call `chat.approval.respond` with `codex_request_id` and `decision` (`accept`, `decline`, `accept_for_session`, or `cancel`).
- Cancel calls `chat.cancel` with the active `chat_id` and `turn_id`; cancellation is best-effort and completion still arrives through chat events.

## Provider auth
Sign in to providers (OpenAI Codex, GitHub Copilot) from the **Settings** panel in web or mobile.
See `docs/provider-auth.md` for details.
Latest smoke checklist/results: `docs/provider-auth-smoke-matrix.md`.

## More docs
- Config reference: `docs/config.md`
- Mobile app notes: `src/apps/mobile/README.md`
