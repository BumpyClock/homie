---
summary: "Rust modularity guidance for module boundaries and re-export patterns"
read_when: "Before restructuring Rust modules or changing visibility strategy"
---

# Rust modularity notes (2026-02-19)

Sources:
- The Rust Programming Language Book: splitting modules into separate files via `mod` and keeping module tree stable. https://doc.rust-lang.org/stable/book/ch07-05-separating-modules-into-different-files.html
- rustdoc book: re-exports to present a clean public API while keeping internal module structure private. https://doc.rust-lang.org/rustdoc/write-documentation/re-exports.html
- PingCAP Rust style guide: module-level privacy, keep module hierarchy private when it is an implementation detail, and expose via re-exports; avoid complex visibility where possible. https://pingcap.github.io/style-guide/rust/modules.html

Key practices:
- Split large modules into files or submodules using `mod name;` while preserving the same module tree and paths; this is the standard Rust idiom for decomposing big files. (Rust Book)
- Prefer a thin public facade and private submodules, exposing a curated API via `pub use` re-exports. (rustdoc book)
- Default to private items and make visibility explicit only where needed; keep module hierarchy private when it is an implementation detail and use re-exports to avoid leaking internal structure. (PingCAP style guide)

