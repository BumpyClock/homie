# Agent Browser (Vercel) research notes

Date: 2026-02-13

## Sources
- GitHub repo: https://github.com/vercel-labs/agent-browser
- GitHub releases: https://github.com/vercel-labs/agent-browser/releases
- npm package: https://www.npmjs.com/package/agent-browser

## Findings
- Project is active and maintained (recent release activity in Feb 2026).
- MIT license.
- Ships as `agent-browser` package, with local/remote browser automation model.
- Better naming fit for Homie than `openclaw_browser` path.

## Recommendation
- Remove `openclaw_browser` provider/config path now (done in this repo).
- Reintroduce browser automation under neutral provider/tool ids:
  - provider id: `browser`
  - tool name: `browser`
- Keep provider dynamic and disabled by default until auth/session model is implemented.

## Update (2026-02-16)
- Implemented first-class core tool `browser` in Homie backend.
- Runtime execution path:
  - primary: `agent-browser ...`
  - fallback: `npx --yes agent-browser ...`
- Default tool behavior:
  - JSON mode enabled by default (`--json`) for model-readable outputs.
  - Tool response normalized to Homie envelope: `{ok, tool, data|error}`.
  - Default timeout: 90s.
- Optional binary override:
  - `HOMIE_AGENT_BROWSER_BIN=/path/to/agent-browser`
