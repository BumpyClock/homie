# Learnings

- 2026-02-01: Initialized.
- 2026-02-01: Added backend PRD in prd.md from plan.md; co-located gateway+node, Tailscale auth, Codex integration, persistence.
- 2026-02-01: Added web PRD in web.prd; terminal-only web client, xterm.js, tabs/keybar, themes, saved targets.
- 2026-02-01: Added RN PRD in rn-mobile.prd; terminal-only, WebView xterm.js, tabs, target management.
- 2026-01-31: US-002 WS server: axum 0.8 + tokio-tungstenite. TailscaleWhois trait needs object-safe (Pin<Box<dyn Future>>) for Arc<dyn> in axum state. ConnectInfo requires into_make_service_with_connect_info; use middleware to inject RemoteIp extension. tokio::time::sleep_until in select! for idle timeout (not post-loop check). tokio-tungstenite 0.26 uses Utf8Bytes not String.
- 2026-01-31: US-003 Terminal service: portable-pty 0.9 returns anyhow::Error from MasterPty methods (resize, try_clone_reader) — map to String rather than pulling anyhow dep. Reader thread uses std::thread (blocking I/O) + AtomicBool shutdown flag + oneshot channel bridge. Output forwarding via mpsc → tokio task → WsMessage::Binary. TerminalService is per-connection (not shared), lives in message loop scope — Drop triggers shutdown_all. Reap interval (2s) polls try_wait for exit events.
