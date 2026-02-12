# OpenClaw browser control endpoint (Homie integration)

Sources: `~/Projects/openclaw/src/browser/client.ts`, `~/Projects/openclaw/src/browser/client-actions-core.ts`, `~/Projects/openclaw/src/browser/routes/*`, `~/Projects/openclaw/docs/tools/browser.md`.

## Control server basics
- Base URL: OpenClaw browser control service (loopback, derived from gateway port).
- Most endpoints accept `?profile=<name>` to select a browser profile.

## HTTP endpoints
- `GET /` status (running, cdp info, profile details).
- `POST /start`, `POST /stop`.
- `GET /profiles`.
- `GET /tabs`.
- `POST /tabs/open` body `{ "url": "..." }`.
- `POST /tabs/focus` body `{ "targetId": "..." }`.
- `DELETE /tabs/{targetId}`.
- `GET /snapshot` query supports `format=ai|aria`, `targetId`, `limit`, `maxChars`, `refs`, `interactive`, `compact`, `depth`, `selector`, `frame`, `labels`, `mode`.
- `POST /screenshot` body `{ targetId, fullPage, ref, element, type }`.
- `POST /navigate` body `{ url, targetId }`.
- `GET /console` query `{ level, targetId }`.
- `POST /pdf` body `{ targetId }`.
- `POST /hooks/file-chooser` body `{ paths, ref, inputRef, element, targetId, timeoutMs }`.
- `POST /hooks/dialog` body `{ accept, promptText, targetId, timeoutMs }`.
- `POST /act` body `{ kind: click|type|press|hover|scrollIntoView|drag|select|fill|resize|wait|evaluate|close, ... }`.

## Notes
- OpenClaw tool schema uses `action` + optional fields (e.g., `snapshotFormat` for snapshots).
- Control server is loopback-only by default; remote access flows through gateway/node proxy.
- No explicit HTTP auth in the control server; secure remote endpoints externally.
