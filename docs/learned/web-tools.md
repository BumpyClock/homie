# Web tools findings

## Reference web_fetch behavior
- SSRF protection: pinned DNS + dispatcher per host (`infra/net/ssrf.ts`).
- Redirect handling: manual, max redirects, loop detection.
- Extraction:
  - HTML -> Readability -> markdown/text.
  - JSON pretty-print.
  - Firecrawl fallback (optional, API key) with cache + proxy mode.
- Cache: in-memory Map with TTL; key includes url + extract mode + max chars.
- Params: `url`, `extractMode` (markdown/text), `maxChars`.
- Output payload: url/finalUrl/status/contentType/title/extractMode/extractor/truncated/length/fetchedAt/tookMs/text (+warning).
- Errors: returns meaningful messages; HTML error body converted to text; max chars on error.

## Reference web_search behavior
- Providers: Brave + Perplexity (direct or via OpenRouter).
- Cache: in-memory TTL.
- Params: `query`, `count`, `country`, `search_lang`, `ui_lang`, `freshness` (Brave only).
- Missing API key returns structured error payload (not thrown).
- Brave: REST GET, returns list of results with title/url/description/published/siteName.
- Perplexity: POST /chat/completions, returns synthesized content + citations.

## codex-rs web_search
- No custom fetch/search implementation.
- Emits OpenAI Responses API tool `{"type":"web_search"}`.
- `external_web_access` flag set based on `WebSearchMode` (Cached/Live).

## SearXNG API notes
- Search endpoints: `/` or `/search`, GET or POST.
- `format=json` required; many instances disable JSON and return 403 until enabled.
- Params: `q` (query), `categories`, `engines`, `language`, `pageno`, `time_range` (day/month/year).
- Some instances may rate‑limit or require auth; behavior depends on server settings.

Sources:
- https://docs.searxng.org/dev/search_api
- https://docs.searxng.org/dev/search_api.html
- WebSearchMode resolved from config + sandbox policy (read-only => cached, danger => live).

## Implication for Homie
- If we keep tools local, follow reference patterns for `web_fetch` + `web_search`.
- Can mirror codex-rs mode semantics: `web_search_mode` mapped from approval/sandbox policy.
- Firecrawl optional; keep disabled unless key set.

## Rust deps picked for Homie web tools
- `readabilityrs` 0.1.2 (Apache-2.0) for Readability extraction.
- `htmd` 0.5.0 (Apache-2.0) for HTML → Markdown.
- `html2text` 0.16.7 (MIT) for HTML → text.
- `pulldown-cmark` 0.13.0 (MIT) for Markdown → text (Firecrawl + error rendering).
