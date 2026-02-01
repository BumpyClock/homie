# Dependency review (2026-02-01)

Sources (primary):
- https://tokio.rs/ (Tokio runtime overview)
- https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0 (axum built on tokio/tower/hyper)
- https://github.com/snapview/tokio-tungstenite (tokio websocket bindings)
- https://docs.rs/crate/tokio-tungstenite/latest/source/README.md (tokio-tungstenite README)
- https://docs.rs/crate/portable-pty/latest (portable-pty overview)
- https://github.com/rusqlite/rusqlite (rusqlite sqlite bindings + bundled feature)
- https://github.com/serde-rs/serde (serde + serde_json usage)
- https://docs.rs/uuid (uuid crate overview)
- https://github.com/dtolnay/thiserror (thiserror derive(Error))
- https://github.com/tokio-rs/tracing (tracing framework)
- https://github.com/rust-lang/futures-rs (futures Stream + async foundations)
- https://tower-rs.github.io/tower/tower/ (tower Service/Layer overview)
- https://github.com/tower-rs/tower-http (tower-http middleware)

Findings (summary):
- Tokio + axum + tower + futures + tracing = standard Rust async web stack; compatible by design.
- tokio-tungstenite = Tokio-friendly WebSocket bindings over tungstenite.
- portable-pty = cross-platform PTY API; part of wezterm.
- rusqlite = ergonomic SQLite bindings; bundled feature recommended for portability.
- serde/serde_json + uuid + thiserror = standard serialization, ids, error-derive.
