# Homie quick start

## Prereqs
- Rust (stable) + cargo
- Node.js 20+ + pnpm (corepack)
- iOS/Android toolchain for mobile (optional)

## Install
```bash
pnpm install
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

Tailscale Serve (optional):
```bash
HOMIE_TAILSCALE_SERVE=1 cargo run -p homie-gateway
```

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

## Provider auth
Sign in to providers (OpenAI Codex, GitHub Copilot) from the **Settings** panel in web or mobile.
See `docs/provider-auth.md` for details.
Latest smoke checklist/results: `docs/provider-auth-smoke-matrix.md`.

## More docs
- Config reference: `docs/config.md`
- Mobile app notes: `src/apps/mobile/README.md`
