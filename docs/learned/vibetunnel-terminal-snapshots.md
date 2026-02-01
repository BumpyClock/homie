# VibeTunnel terminal snapshots (reference)

- Server keeps a terminal buffer using ghostty-web (WASM) with scrollback limit 10k lines.
- Buffer snapshots include full cells + cursor + viewport; encoded to a compact binary format.
- WebSocket v3 streams snapshots; client decodes via TerminalRenderer (ghostty-web) to render.
- Sessions API exposes `/sessions/:id/text` by rendering snapshot to plain text.

Implication for Homie: to match UX, need server-side terminal state and snapshot-on-attach (or replay full output stream).
