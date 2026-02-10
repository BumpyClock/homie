# Phase 3 Plan: React Native (Expo) App, Chat First

## Scope
- Build mobile app under `src/apps/mobile` using latest stable Expo SDK.
- Deliver chat experience equivalent to web first.
- Terminal support is tracked, but renderer implementation is deferred.

## Principles
- Shared protocol/client between web and mobile (single source of truth).
- Chat parity before terminal rendering.
- Live-provider validation for critical chat paths.
- Keep backend API unchanged (`chat.*`, `terminal.*`).

## Bead Map
- `remotely-8di` (epic): Phase 3 umbrella.
- `remotely-8di.1`: Expo bootstrap + tooling.
- `remotely-8di.2`: Shared gateway/chat client extraction.
- `remotely-8di.4`: RN shell + gateway connection lifecycle.
- `remotely-8di.3`: Thread list/detail + streaming render.
- `remotely-8di.5`: Composer/settings/tools/approvals parity.
- `remotely-8di.6`: Persistence/reconnect/queued-inject parity.
- `remotely-8di.8`: Env-gated live integration tests.
- `remotely-8di.7`: Terminal protocol scaffold + placeholder UX.
- `remotely-8di.9`: Terminal renderer plan (post-chat).

## Dependency Order
1. `8di.1` -> `8di.2` -> `8di.4`
2. `8di.4` -> `8di.3` -> `8di.5` -> `8di.6` -> `8di.8`
3. `8di.4` -> `8di.7` -> `8di.9`

## Subagent Execution Strategy
- Subagent A (mobile foundation): `8di.1`, `8di.4`
- Subagent B (shared client): `8di.2`
- Subagent C (chat UI): `8di.3`, `8di.5`
- Subagent D (reliability/tests): `8di.6`, `8di.8`
- Subagent E (terminal deferred track): `8di.7`, `8di.9`

Rules:
- B starts after A opens scaffold branch.
- C starts after B lands shared package contracts.
- D starts after C lands event/render semantics.
- E can start after A and run independently.

## Milestones
- M1: App boots, connects to gateway, shared client in place.
- M2: Chat thread list/detail + streaming working end-to-end on device.
- M3: Composer + approvals + tool display parity achieved.
- M4: Reconnect/persistence parity + live tests passing.
- M5: Terminal placeholder shipped; renderer plan approved.

## Non-goals (this phase)
- Discord/Telegram/WhatsApp channels in mobile.
- Full terminal renderer implementation.
- New backend protocol changes beyond parity fixes.

## Risks
- Web/mobile divergence if shared client extraction is partial.
- Stream rendering performance on low-end devices.
- Reconnect race conditions without strict session lifecycle state machine.

## Exit Criteria
- Mobile chat can do everything web chat can do today.
- Live mobile chat smoke passes against configured providers.
- Terminal entry point exists with clear placeholder and tracked follow-up.
