# chrono dep health check (2026-02-03)

Sources:
- https://docs.rs/crate/chrono/latest
- https://docs.rs/crate/chrono/latest/builds
- https://github.com/chronotope/chrono/releases

Findings (2026-02-03):
- docs.rs latest page shows chrono 0.4.43 (recent), MSRV 1.61, Apache-2.0/MIT. citeturn0search1
- docs.rs build page lists 0.4.42 builds (Sep 2025). citeturn0search2
- GitHub releases list 0.4.42 as latest tag (Sep 2025). citeturn0search0
- lib.rs shows high adoption (29k+ crates, 13M+ downloads/month). citeturn0search6

Decision: ok to add chrono = "0.4" (+ serde feature if needed).
