# Terminal Renderer: Decision Record

## Status

Decided — xterm.js via WebView

## Context

The Remotely mobile app (React Native) needs a fully functional terminal emulator to display remote shell sessions. The renderer must handle ANSI escape sequences, cursor positioning, scrollback, and color themes while accepting binary-framed output from the gateway over WebSocket. React Native has no built-in terminal widget, so we need to either embed a proven terminal library or build one from scratch.

## Options Considered

### 1. xterm.js in a WebView (chosen)

Load [xterm.js](https://xtermjs.org/) plus its `addon-fit` via CDN inside a `react-native-webview`. Communication between native and web layers happens through `postMessage` / `injectJavaScript`.

### 2. Native terminal renderer (react-native-terminal or custom native module)

Build or adopt a native View that interprets VT sequences directly in the RN render tree, potentially using a library like `react-native-terminal-component`.

### 3. Custom Canvas / Skia-based renderer

Render glyphs on a `@shopify/react-native-skia` canvas or Expo GL context, implementing VT parsing in JS/TS.

## Decision

**xterm.js via CDN-loaded WebView** — the terminal runs inside an inline HTML document served to `react-native-webview`. xterm.js and `addon-fit` are loaded from `cdn.jsdelivr.net`. The RN layer communicates with the WebView through a structured JSON message protocol.

## Rationale

- **Proven rendering engine**: xterm.js powers VS Code's integrated terminal and handles the full VT100/VT220/xterm spec (colors, cursor modes, alternate screen, mouse events). Writing a comparable native renderer would be a multi-month effort.
- **Minimal native code**: The entire integration is a single React component (`MobileTerminalPane.tsx`) with zero native modules beyond `react-native-webview` (already in the dependency tree).
- **Rapid iteration**: Theme, font, and behavior changes are CSS/JS tweaks inside the HTML template — no native rebuilds required.
- **Fit addon**: `FitAddon` auto-calculates cols/rows from the WebView dimensions and reports them back to the RN layer, which forwards them to the server for proper PTY resizing.
- **Ecosystem maturity**: xterm.js has 17k+ GitHub stars, active maintenance, and extensive addon ecosystem (search, image, webgl renderer) available for future enhancement.

## Tradeoffs

| Aspect | Impact | Mitigation |
|---|---|---|
| **WebView overhead** | Extra process / memory for the WebView (~15-30 MB). Slightly higher latency than a native text renderer. | Acceptable on modern devices; single WebView instance per terminal pane. |
| **CDN dependency** | First render requires network fetch of xterm.js assets (~180 KB gzipped). Offline startup fails. | Could bundle assets in the app binary in future; CDN is cache-friendly for repeat launches. |
| **Keyboard UX** | WebView keyboard input can conflict with RN's keyboard handling. Special keys (Tab, Ctrl+C, Esc, arrows) may not propagate correctly. | Implemented a custom accessory key bar (`ACCESSORY_KEYS`) with 7 common terminal keys that inject input directly via the `onInput` callback, bypassing WebView keyboard limitations. Keyboard show/hide is detected and the bar is animated in/out. |
| **Bridge serialization** | All terminal output must be serialized from binary frames to strings, then injected as JS. Large bursts could cause jank. | Writes are batched (`pendingWritesRef` queues writes until the WebView signals `ready`, then flushed). Output is decoded with `TextDecoder` in streaming mode for efficiency. |
| **Platform differences** | iOS and Android handle WebView focus, scrolling, and keyboard events differently. | `scrollEnabled={false}`, `bounces={false}`, and platform-conditional keyboard event names (`keyboardWillShow` vs `keyboardDidShow`) are already handled. |

## Implementation Summary

### Key Files

| File | Purpose |
|---|---|
| `src/apps/mobile/components/terminal/MobileTerminalPane.tsx` | Main component: WebView host, message bridge, keyboard accessory bar, session lifecycle |
| `src/apps/mobile/components/terminal/terminal-binary.ts` | Binary frame parser for WebSocket messages (16-byte UUID + 1-byte stream type + payload) |

### Architecture

```
Gateway (WebSocket binary frames)
        │
        ▼
  parseBinaryFrame()          ← terminal-binary.ts
  [sessionId | stream | payload]
        │
        ▼
  TextDecoder.decode(payload)  ← streaming mode
        │
        ▼
  injectJavaScript()           ← write to xterm.js via __homieTerminal.write()
        │
        ▼
  WebView (xterm.js)           ← renders ANSI output
        │
        ▼
  postMessage({ type, data })  ← user input / resize events back to RN
        │
        ▼
  onInput() / onResize()       ← forwarded to gateway
```

### Binary Frame Protocol

The server sends WebSocket binary frames with a 17-byte header:
- **Bytes 0-15**: Session UUID (16 bytes, converted to string via `uuidFromBytes`)
- **Byte 16**: Stream type (`0` = stdout, `1` = stderr, `2` = stdin)
- **Bytes 17+**: Raw payload (terminal output bytes)

### WebView ↔ RN Message Protocol

Messages are JSON objects with a `type` discriminator:

| Direction | Type | Fields | Purpose |
|---|---|---|---|
| WebView → RN | `ready` | — | xterm.js initialized, flush pending writes |
| WebView → RN | `input` | `data: string` | User typed in terminal |
| WebView → RN | `resize` | `cols, rows` | Terminal dimensions changed |
| WebView → RN | `error` | `message: string` | xterm.js initialization failed |

### Accessory Key Bar

A floating bar with 7 keys (Tab, ^C, ^V, ^B, Esc, ↑, ↓) appears above the keyboard. Each key directly sends its escape sequence via `onInput`, working around WebView keyboard limitations on mobile.

## Future Considerations

- **Bundle xterm.js assets**: Embed the CSS and JS in the app binary to eliminate CDN dependency and enable offline terminal use. This is the highest-priority improvement.
- **WebGL renderer addon**: xterm.js offers `addon-webgl` for GPU-accelerated rendering. If users report scroll jank on long output, this is a drop-in upgrade.
- **Native bridge for binary data**: Currently binary frames are decoded to strings in JS. A native module could pass `Uint8Array` directly into the WebView, reducing GC pressure for high-throughput sessions.
- **When to reconsider**: If WebView memory overhead becomes problematic on low-end devices (< 2 GB RAM), or if a mature React Native terminal component emerges that handles the full VT spec, revisit this decision.
- **Performance threshold**: If terminal output latency exceeds ~50ms p95 under normal usage (measured from binary frame arrival to pixel render), investigate the native bridge or WebGL addon paths.
