# Homie Mobile (Expo)

Chat-first React Native app for Homie.

## Status
- Phase: `remotely-8di.2.5` mobile shared client integration
- Implemented: live gateway chat tab using `@homie/shared` transport + chat helpers, thread list/sort, thread read, send, stream updates, create chat, target OOBE + persistence
- Pending: richer timeline rendering parity (approvals/tools/settings)

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

Notes:
- No localhost default.
- First launch requires saving a gateway target before chat becomes usable.
- `EXPO_PUBLIC_HOMIE_GATEWAY_URL` is used as a prefill hint only.

## Structure
- `app/` Expo Router screens
- `theme/` palette, spacing, typography, motion tokens
- `components/ui/` reusable polished UI building blocks
- `components/chat/` chat tab primitives (thread list, timeline, composer)
- `components/gateway/` gateway target setup/edit form
- `hooks/` app theme + reduced-motion hooks
- `config/runtime.ts` environment-backed runtime config

## Chat tab behavior
- Connection state shown via gateway transport status badge.
- If no saved target, chat tab shows gateway setup OOBE and blocks chat actions.
- Threads loaded from `chat.list`, sorted by last activity, then hydrated from `chat.thread.read`.
- Selecting a thread loads messages from `chat.thread.read`.
- Composer sends with `chat.message.send`.
- `chat.*` gateway events stream into active thread via `mapChatEvent`.
- New chat button calls `chat.create` and opens thread.

## Target management
- Selected target URL persisted in local async storage.
- Settings tab supports update/clear target.
- Changing target reconnects chat transport automatically.

## Design direction
- Personality: utility + trust (dense enough for power users, calm visual hierarchy)
- Touch-first sizing: 44px+ controls
- Motion: short ease-out transitions; disabled when reduced motion is enabled
