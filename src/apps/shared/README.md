# @homie/shared

Shared TypeScript package for Homie app clients (`src/web`, `src/apps/mobile`).

## Scope

- Protocol envelopes + typed RPC payloads
- Gateway websocket transport + request lifecycle
- Chat API helpers + event mapping

## Versioning Strategy

- Internal package for now (`private: true`), versioned with semver tags in `package.json`
- Minor version bump (`0.x+1`) for additive APIs
- Patch version bump for internal fixes with no API changes
- Breaking API changes require explicit migration notes in bead updates before merge

## Commands

- `pnpm install`
- `pnpm typecheck`
- `pnpm build`

## Consumers

- Web app (`src/web`) via local path alias/import
- Mobile app (`src/apps/mobile`) via local path alias/import

Wiring is handled in follow-up beads:
- `remotely-8di.2.2`
- `remotely-8di.2.3`
- `remotely-8di.2.4`
- `remotely-8di.2.5`
