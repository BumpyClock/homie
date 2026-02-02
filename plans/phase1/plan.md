# Homie — build plan (draft)

## Goal
Build a cross-platform “remote terminal access” system:
- **Agents** run on your machines and expose PTY-backed terminal sessions (portable-pty).
- **Clients** (Web + iOS/Android) can list machines, start/attach to sessions, send input, receive output, resize, and manage auth.
- **Remote connectivity** via Tailscale (preferred), with optional alternatives (SSH tunnel / public TLS).

## Key constraints / preferences
- Prefer **Rust back-end** where possible.
- Gateway supports **two parallel experiences**: terminal passthrough + agentic chat.
- Start with **Codex** as the first agent experience (Codex app-server protocol).
- Reuse **agent-term’s** Rust PTY runtime (portable-pty) if practical.
- Keep client transport low overhead (WebSocket preferred).
- Explore OpenClaw’s **gateway architecture** benefits.
- Minimize duplicated UI code between Web and React Native if possible.

---

## Recommended architecture (two-tier; aligns with OpenClaw patterns)

### 1) "Node/Agent" (runs on each machine; Rust)
A long-running process (systemd/launchd/Windows service):
- Responsible for:
  - Spawn/manage PTYs (portable-pty) + child processes
  - Stream output + accept input
  - Session lifecycle (create/attach/detach/terminate)
  - Machine identity + keys
  - Optional: file transfer, clipboard, audit log
- Exposes a local API (loopback) to avoid accidental LAN exposure.

**Reuse plan from agent-term**
- Lift the existing PTY runtime + session management patterns:
  - `SessionRuntime` (writer/master/child + reader thread)
  - resize/write/shutdown semantics
- Replace “tauri event emit” with a WS stream abstraction:
  - one WS connection per client (or per session)
  - multiplex multiple sessions over one WS if needed

### 2) "Gateway" (OpenClaw-style platform; Rust preferred)
A long-running **WebSocket hub** + **service platform** (not terminal-specific):
- **Clients connect** to the Gateway.
- **Nodes/Agents connect** to the Gateway (outbound), register, heartbeat, and expose capabilities.
- Gateway provides:
  - **Routing/multiplexing**: client ↔ node messaging over channels
  - **Auth + policy**: roles/scopes, per-method authorization, audit log hooks
  - **Service registry**: nodes advertise capabilities (e.g. `terminal`, `files`, `portForward`, `jobs`)
  - **Event bus**: push events to subscribed clients (session output, file watch, job progress)
  - **Optional persistence**: sessions metadata, pairing, tokens, durable queues (later)

**Why this is valuable:**
- Avoids direct inbound connectivity to each node (outbound-only nodes still work).
- Centralizes auth, auditing, policy, discovery, multi-device access.
- Lets Remotely add many features behind one stable endpoint + protocol.

**When you may skip the gateway (initially):**
- If you’re OK with “client connects directly to agent over Tailnet” (simple + robust).
- You can still keep the protocol identical so a gateway can be added later.

### 3) Clients
- Web client (desktop/mobile browsers)
  - Prefer React + Vite.
  - shadcn components
- iOS/Android client (React Native)

---

## Connectivity modes (MVP: Local + Tailscale)

### MVP — Local + Tailscale
- **Local**: Gateway + agent bind to **loopback** for same-machine access (`ws://127.0.0.1:<port>`).
- **Tailscale** (remote): expose the Gateway via one of:
  - **Tailscale Serve** (recommended): keep Gateway on loopback; get HTTPS + `wss://<magicdns>`.
  - **Direct tailnet bind**: Gateway listens on tailnet IP (`ws://<tailscale-ip>:<port>`), still requires auth.

### Later (optional)
- **SSH tunnel** (OpenClaw parity): port-forward loopback gateway.
- **Public internet**: TLS + token/password + optional cert pinning.

---

## Gateway protocol model (OpenClaw-inspired)

**Core idea:** everything is a **namespaced service** over one WS connection using a common envelope:
- `hello/connect` handshake: protocol version range, client identity, auth
- `request/response` frames: `id`, `method`, `params`, `error`
- `event` frames: `topic` + payload, subscribable streams
- `binary` frames for high-volume data (PTY bytes, large logs, file chunks)

### Terminal passthrough (non-chat feature)
Terminal tunneling is its own product surface (not a conversational UX).
- Client uses `terminal.*` APIs to create/attach a PTY session and then streams bytes over binary frames.
- This should work even if the Agent SDK is disabled.

Typical methods/events:
- `terminal.session.start` / `terminal.session.attach` / `terminal.session.resize` / `terminal.session.input` / `terminal.session.kill`
- `terminal.session.output` / `terminal.session.exit`

### Agentic chat (separate, parallel feature)
Conversational agents are long-lived **chat sessions**.

**Codex-first:** for the first implementation, the Gateway runs/hosts a Codex app-server session (per workspace or per node) and forwards Codex app-server events to clients (CodexMonitor is the reference implementation).

**Gateway responsibility:** act as a transport + router.
- Accept user messages from clients.
- Forward them to the Agent SDK runtime.
- Stream back whatever the Agent SDK emits (assistant deltas/finals, tool call requests, approval requests, tool results, status events).

This keeps the Gateway thin: you don’t re-implement “approval logic” or agent semantics; you just faithfully carry structured events.

**Session model**
- `agent.chat.create` → returns `chatId`
- `agent.chat.message.send` → user message into chat
- `agent.chat.event.subscribe` → stream events for that chat
- `agent.chat.cancel` / `agent.chat.close`

**Event types (examples)**
The Agent SDK defines the semantics; the Gateway just forwards.
- `agent.message.delta` / `agent.message.final`
- `agent.tool.call` (structured request to invoke a tool)
- `agent.tool.result`
- **No custom approval subsystem in the Gateway.** The agent provider (Codex app-server) emits approval-required events; clients render prompts and respond using the provider’s existing response method (CodexMonitor pattern).
  - Policy: **always prompt** when Codex requests approval.
- `agent.status`
- `agent.error`

### Method naming
Use `service.method` (examples)
- `terminal.session.start`, `terminal.session.input`, `terminal.session.resize`
- `jobs.start`, `jobs.status`, `jobs.cancel`, `jobs.logs.tail`
- `pairing.request`, `pairing.approve`, `pairing.list`
- `presence.heartbeat`, `presence.list`
- `notifications.register`, `notifications.send`
- `agent.chat.create`, `agent.chat.message.send`, `agent.chat.event.subscribe`, `agent.chat.cancel`

**Capabilities:** nodes advertise `{ service: version, features }` so clients can adapt.

**Agentic experience (Codex App Server-style):** treat the Gateway as a tool-router + event-stream hub.
- Run the Agent SDK runtime server-side (in/alongside the Gateway) so chats can continue in the background.
- The Agent SDK drives the conversation and emits structured events; Gateway streams them to clients.
- When the Agent SDK requests a tool call, the Gateway routes it to the correct service (`terminal.*`, `jobs.*`, etc.) and returns the tool result back into the Agent SDK.
- Note: this doesn’t make the terminal a “chat UI” feature; it’s still a separate experience that can optionally be invoked as an agent tool.
- Persist minimal state for resumability: `chat` metadata + append-only event log pointer (later).

## Transport & protocol plan

### WebSockets
Use WebSockets for both control plane + data plane:
- Low overhead, ubiquitous in browsers + RN.
- Good fit for interactive PTY streams.

### Message format: JSON vs Protobuf (recommendation)
**Recommendation:** start with **JSON for control** + **binary WS frames for PTY bytes**.
- Most bandwidth is terminal output; JSON overhead is negligible compared to escape sequences and raw text.
- Binary frames avoid base64 and keep latency low.
- Strong typing can still exist via:
  - Rust `serde` structs + TS types (zod/io-ts optional)
  - versioned message schema

**Where Protobuf helps:**
- If you expect very high message rates of small structured messages (telemetry, many RPC calls).
- If you need strict schema evolution across multiple languages.

**Tradeoff:** Protobuf adds tooling complexity across:
- Rust ↔ TypeScript ↔ React Native,
- plus debugging friction.

**Compromise option:** CBOR/MessagePack for control messages (binary, but still easy) if JSON becomes an issue.

### Multiplexing suggestion
Two channels over one WS:
- `text/json` frames: RPC envelopes (startSession, resize, listMachines)
- `binary` frames: `[header][pty-bytes]` where header includes `sessionId` + stream type.

---

## Security plan (staged)

### MVP security (Tailscale)
- Rely on tailnet encryption.
- **Auth: Tailscale identity only (no app token for MVP).**
  - Serve the Gateway through Tailscale Serve.
  - Validate the caller identity using Tailscale-provided identity headers + `tailscale whois` (OpenClaw pattern).

### Device pairing via QR (later)
Goal: pair a new client device with the Gateway without typing URLs/tokens.
- Gateway generates a **short-lived pairing session** (nonce), displays QR.
- Client scans QR, connects to the Gateway, and requests pairing using the nonce.
- Gateway requires an **operator approval** (or physical access policy) before minting a device token.
- Client stores the minted **device token** in Keychain/Keystore.

**QR payload (suggested):**
- `ws/wss` URL (or MagicDNS host + port)
- pairing `nonce`
- optional `tlsFingerprint` (for pinned self-signed TLS deployments)

### Non-tailnet security (later)
- TLS with either:
  - real certs (LetsEncrypt) OR
  - self-signed + fingerprint pinning (OpenClaw does a variant of this)

---

## OpenClaw-inspired agent loop enhancements (next steps)
Analysis sources: `openclaw/docs/concepts/agent-loop.md`, `system-prompt.md`, `context.md`, `compaction.md`, `memory.md`, `automation/cron-vs-heartbeat.md`, plus `src/agents/*`.

### A) Make responses more dynamic + helpful
- **System prompt assembly** similar to OpenClaw:
  - Sections: tooling, skills list, workspace docs, runtime, current time, reasoning visibility, heartbeat instructions.
  - Inject workspace bootstrap files (`AGENTS.md`, `TOOLS.md`, etc.) with truncation + markers.
  - Prompt modes (`full`/`minimal`) for subagent runs.
- **Memory-first behavior**:
  - Add `memory_search` + `memory_get` tools (workspace Markdown memory store).
  - System prompt instruction: search memory before answering about prior decisions/preferences.
  - Optional session transcript search (memory sources: `memory`, `sessions`).
- **Reply shaping & suppression**:
  - Tool summaries in assistant reply (opt-in verbose).
  - Suppress duplicate tool confirmations; support `NO_REPLY` for silent steps.
- **Context introspection**:
  - `/status`, `/context list`, `/context detail` equivalents for token budgets, prompt composition, and tool schema sizes.

### B) Long-running quality + continuity
- **Auto-compaction** with retry; persist compacted summary in transcript.
- **Pre-compaction memory flush** (silent agent turn writing durable notes).
- **Session pruning**: trim old tool results in-memory per request (no transcript rewrite).

### C) Reliability + orchestration
- **Session-lane queueing**: serialize runs per chat/session + optional global lane.
- **Lifecycle hooks** (plugin-style): `before_agent_start`, `before_tool_call`, `after_tool_call`, `tool_result_persist`, `agent_end`, `before/after_compaction`.
- **Heartbeats + cron**:
  - Main-session heartbeat for “check-ins” with `HEARTBEAT_OK` suppression.
  - Isolated cron jobs for scheduled tasks with optional delivery.

### D) UX tie-ins (chat)
- Expose lifecycle/stream events to support progress UI (tool steps, reasoning streaming, approvals).
- Show compaction/heartbeat events in chat timeline (collapsed).

---

## Replace Codex CLI app-server with embedded agent loop (next milestone)
Goal: own the loop (tools, prompt, compaction) while keeping Codex OAuth; later add Claude + GitHub Copilot auth.

### Step 1: Extract + embed Codex loop (no CLI)
- Evaluate codex-rs core surfaces (agent loop + app-server protocol).
- Replace app-server process with in-process runner + WS bridge (same chat.* API).
- Keep CLI-compatible JSONL events so UI + gateway stay stable.

### Step 2: Codex OAuth retention
- Reuse codex-rs OAuth flow + token store (per-gateway).
- Expose `chat.account.*` status/refresh in gateway (already planned).
- Ensure token refresh path before each run.

### Step 3: Provider abstraction (future)
- Provider registry: `codex`, `claude`, `copilot` (same loop interface).
- Per-provider auth adapters + model catalogs.
- Store tokens in gateway state dir; reuse account status UX.

### Step 4: Loop customization
- Swap in our system prompt assembly + memory/compaction/hooks.
- Add tool policy per provider.
- Auth options:
  - token (API key)
  - password (shared secret)
  - Tailscale identity headers when using Serve (OpenClaw verifies via `tailscale whois`)

---

## Frontend strategy (MVP: native UX)

### Web
- Web app is a first-class client: xterm.js + session management.

### Mobile (React Native)
- Build a **native RN UI** for navigation, agent list, session list, settings, auth, etc.
- **Terminal view approach:**
  - **Recommended MVP:** WebView embedding xterm.js for the terminal screen only (keeps terminal parity + battle-tested renderer).
  - Later: replace with a true native renderer if needed.

### Shared code (keep parity without duplicating logic)
Use a monorepo `packages/` shared by web + RN:
- `protocol`: message schema + codecs (JSON control + binary PTY frames)
- `client-core`: gateway connection, reconnection, auth, multiplexing, heartbeats
- `terminal-core`: key mapping, paste/clipboard helpers, resize heuristics
- `ui-tokens`: colors/spacing/typography tokens

**Recommendation:** native RN app + shared core packages; start with WebView-for-terminal only, and upgrade later if terminal UX demands it.

---

## Concrete milestones (checkbox workplan)

### Phase 0 — Spike / decisions
- [x] Decide MVP connectivity mode: **Local + Tailscale**
- [x] Ship a Gateway from day one (OpenClaw-style)
- [x] Mobile UX: **separate React Native UI** (share protocol/client-core)
- [x] Mobile terminal screen: **Hybrid WebView (xterm.js for terminal only)**

### Phase 1 — Gateway (Rust)
- [ ] Define protocol envelopes (hello/request/response/event) + versioning
- [ ] Implement Gateway WebSocket server:
  - [ ] auth: **Tailscale identity only** (Serve identity headers + `tailscale whois`)
  - [ ] roles/scopes + per-method authorization
  - [ ] node registration + capability advertisement + heartbeat
  - [ ] routing/multiplexing + subscribe/unsubscribe event streams
  - [ ] service namespaces (general framework):
    - [ ] `terminal.*` (PTY sessions)
    - [ ] `jobs.*` (automations)
    - [ ] `pairing.*` + `presence.*`
    - [ ] `notifications.*`
    - [ ] `agent.codex.*` (Codex app-server backed chat)
- [ ] Codex app-server integration (CodexMonitor reference):
  - [ ] spawn/manage `codex app-server` sessions
  - [ ] forward Codex notifications/events to clients
  - [ ] route client requests to Codex methods (`thread/*`, `turn/*`, `respond_to_server_request`, ...)

### Phase 2 — Node runtime (Rust)
- [ ] Extract portable-pty session runtime from agent-term into a reusable crate/module
- [ ] Implement node ↔ gateway connection:
  - [ ] register + keepalive + reconnect
  - [ ] advertise capabilities (`terminal`, `jobs`, `codex`)
  - [ ] Terminal PTY: spawn/manage sessions; stream output; accept input/resize; cleanup
  - [ ] Jobs runner: run named tasks/recipes; stream logs; report status
  - [ ] Codex runtime option: run Codex on the node and expose to Gateway (CodexMonitor daemon pattern)
  - [ ] Presence: periodic heartbeats + metadata (hostname, os, version)

### Phase 3 — Web client
- [ ] Terminal UI (xterm.js) + session list
- [ ] Agentic chat UI (conversations + streaming events)
- [ ] Connect/disconnect, reconnection, resize
- [ ] Target selection: local (`ws://127.0.0.1`) + tailnet (`wss://<magicdns>`)

### Phase 4 — Mobile client
- [ ] React Native app (native navigation)
- [ ] Terminal feature UI (session list + terminal screen)
- [ ] Agentic chat feature UI (chat list + chat screen)
- [ ] Secure storage of tokens (Keychain/Keystore)
- [ ] Background handling + reconnect
- [ ] Mobile keyboard UX (escape/ctrl/arrows tab bar, quick paste)

### Phase 5 — Remote exposure
- [ ] Tailscale Serve automation for `wss://` to loopback gateway
- [ ] Direct tailnet bind option + docs
- [ ] QR-based device pairing (later)
- [ ] TLS + cert pinning path for non-tailnet use (later)

---

## Notes / open questions
- Discovery: do we want Bonjour/WAB discovery (OpenClaw does this) or start with manual URL entry?
- Session sharing: do we need multi-client attach to same PTY session in MVP?
- Recording: do we want optional scrollback persistence?
