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
Default bind: `127.0.0.1:9800`.

Allow LAN (optional):
```bash
HOMIE_ALLOW_LAN=1 cargo run -p homie-gateway
```

## Run web
```bash
VITE_GATEWAY_URL=ws://127.0.0.1:9800/ws pnpm dev
```

## Run mobile
```bash
EXPO_PUBLIC_HOMIE_GATEWAY_URL=ws://127.0.0.1:9800/ws pnpm dev:mobile
```
Notes:
- First launch requires saving a gateway target in Settings.
- `EXPO_PUBLIC_HOMIE_GATEWAY_URL` is a prefill hint only.

## Provider auth
Device-code flow for `openai-codex` and `github-copilot`.
See `docs/provider-auth.md`.

## More docs
- Config reference: `docs/config.md`
- Mobile app notes: `src/apps/mobile/README.md`
