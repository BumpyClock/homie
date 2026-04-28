# Homie web

React/Vite client for Homie terminal + chat access. It connects to the Homie gateway websocket and uses `@homie/shared` for the JSON-RPC transport, handshake, chat client helpers, and shared settings/auth types.

## Setup

From repo root:

```bash
pnpm install
pnpm --filter @homie/shared build
```

Start the gateway first:

```bash
cargo run -p homie-gateway
```

Default gateway target:

- `ws://127.0.0.1:9800/ws`

## Environment

```bash
VITE_GATEWAY_URL=ws://127.0.0.1:9800/ws
```

If unset in dev, the web app defaults to `ws://127.0.0.1:9800/ws`.

For LAN testing, run the gateway with:

```bash
HOMIE_BIND=0.0.0.0:9800 HOMIE_ALLOW_LAN=1 cargo run -p homie-gateway
VITE_GATEWAY_URL=ws://<host-lan-ip>:9800/ws pnpm --filter web dev
```

For Tailscale remote access, run the gateway with `HOMIE_TAILSCALE_SERVE=1` or compatibility alias `HOMIE_TAILSCALE=1`, then point `VITE_GATEWAY_URL` at the served `wss://.../ws` endpoint.

## Commands

- `pnpm --filter web dev` - start Vite; prebuilds `@homie/shared`
- `pnpm --filter web lint` - run ESLint
- `pnpm --filter web typecheck` - run TypeScript without emit
- `pnpm --filter web build` - build shared package, typecheck web, and build Vite assets
- `pnpm --filter web preview` - serve the production build locally

## Target Behavior

- The first websocket frame is the shared client handshake; RPC starts only after a `hello` response.
- Terminal and chat calls use JSON-RPC envelopes over the gateway websocket.
- Approval responses call `chat.approval.respond` with `codex_request_id` and `decision`.
- Cancel sends `chat.cancel` with the active `chat_id` and `turn_id`.
