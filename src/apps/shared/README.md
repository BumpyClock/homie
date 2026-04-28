# @homie/shared

Shared TypeScript package for Homie web and mobile clients.

## Scope

- Protocol envelopes and typed RPC payloads
- Gateway websocket transport, including the required client hello handshake before RPC
- Chat API helpers, event mapping, approval responses, and cancel calls
- Shared settings, provider-auth copy, and client hooks

## Consumers

- Web: `src/web`
- Mobile: `src/apps/mobile`

Both clients import from `@homie/shared` through the workspace package.

## Commands

- `pnpm --filter @homie/shared build`
- `pnpm --filter @homie/shared typecheck`
- `pnpm --filter @homie/shared test`

## API Notes

- `GatewayTransport` sends the handshake first and only releases queued RPC after `hello`.
- `createChatClient(...).respondApproval(...)` maps to `chat.approval.respond`.
- `createChatClient(...).cancel(...)` maps to `chat.cancel` with `chat_id` and `turn_id`.
