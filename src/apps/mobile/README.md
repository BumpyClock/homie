# Homie Mobile (Expo)

Chat-first React Native app for Homie.

## Status
- Phase: `remotely-8di.1` foundation
- Implemented: polished app shell, tabs (`Chat`, `Terminals`, `Settings`), theme tokens, reduced-motion-aware enter animation
- Pending: gateway RPC wiring, streaming chat parity, approvals/tool rendering

## Run
```bash
cd src/apps/mobile
pnpm start
pnpm android
pnpm ios
pnpm web
pnpm typecheck
```

## Environment
Set gateway URL for mobile runtime:
```bash
EXPO_PUBLIC_HOMIE_GATEWAY_URL=ws://192.168.1.110:9800/ws pnpm start
```

Default fallback:
- `ws://127.0.0.1:9800/ws`

## Structure
- `app/` Expo Router screens
- `theme/` palette, spacing, typography, motion tokens
- `components/ui/` reusable polished UI building blocks
- `hooks/` app theme + reduced-motion hooks
- `config/runtime.ts` environment-backed runtime config

## Design direction
- Personality: utility + trust (dense enough for power users, calm visual hierarchy)
- Touch-first sizing: 44px+ controls
- Motion: short ease-out transitions; disabled when reduced motion is enabled
