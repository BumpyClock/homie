# Homie

Cross-platform remote terminal + chat gateway. Rust backend, web + mobile clients.

## Architecture
- Gateway: `src/gateway` (Axum + WS)
- Core: `src/core` (services, persistence, tools)
- Web: `src/web` (Vite + React)
- Mobile: `src/apps/mobile` (Expo)
- Shared TS: `src/apps/shared`

## Quick start
See `docs/quick-start.md`.

## Dev commands (repo root)
```bash
pnpm dev         # shared + web
pnpm dev:mobile  # shared + mobile
pnpm dev:all     # shared + web + mobile
pnpm typecheck
```

## Docs
- Config reference: `docs/config.md`
- Provider auth: `docs/provider-auth.md`
- Mobile app: `src/apps/mobile/README.md`
