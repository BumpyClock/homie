# Model catalog refresh (Codex + GitHub Copilot)

Date: 2026-02-11

## What changed
- Refreshed static fallback model catalog in `src/core/src/agent/service.rs`.
- Removed provider `/models` probing for Codex/Copilot in `chat.model.list` path (not reliable for our current provider wiring; caused 400s and missing-key warnings).

## Sources checked
- OpenAI release note: GPT-5-Codex available in API/ChatGPT.
  - https://help.openai.com/en/articles/9624314-model-release-notes
- GitHub Copilot docs: model support list (includes GPT-5.x and Codex variants).
  - https://docs.github.com/en/copilot/reference/ai-models/supported-models

## Catalog decisions
- Codex fallback includes:
  - `gpt-5.2-codex`, `gpt-5-codex`, `gpt-5.1-codex`, `gpt-5.1-codex-mini`, `gpt-5.1-codex-max`
- GitHub Copilot fallback (openai-compatible) includes:
  - `gpt-4.1`, `gpt-5`, `gpt-5-mini`, `gpt-5-codex`, `gpt-5.1`, `gpt-5.1-codex`, `gpt-5.1-codex-mini`, `gpt-5.1-codex-max`, `gpt-5.2`, `gpt-5.2-codex`

## Notes
- OpenClaw-style model list path is catalog/registry-driven, not direct provider `/models` probing.
- If we need truly live lists later, implement provider-specific discovery endpoints with robust auth/shape handling per provider.
- GitHub docs currently mark `gpt-5` and `gpt-5-codex` as retiring on 2026-02-17; keep in fallback short-term for continuity, then prune.
- 2026-02-11 update: Added `gpt-5.3-codex` and `gpt-5.2` to Codex fallback catalog; added `gpt-5.3-codex` to Copilot fallback catalog.
- 2026-02-11 update: Expanded GitHub Copilot fallback models to match docs table categories (OpenAI, Anthropic, Google, xAI, Raptor): `gpt-4.1`, `gpt-5`, `gpt-5-mini`, `gpt-5.3-codex`, `gpt-5-codex`, `gpt-5.1`, `gpt-5.1-codex`, `gpt-5.1-codex-mini`, `gpt-5.1-codex-max`, `gpt-5.2`, `gpt-5.2-codex`, `claude-haiku-4.5`, `claude-opus-4.1`, `claude-opus-4.5`, `claude-opus-4.6`, `claude-opus-4.6-fast`, `claude-sonnet-4`, `claude-sonnet-4.5`, `gemini-2.5-pro`, `gemini-3-flash`, `gemini-3-pro`, `grok-code-fast-1`, `raptor-mini`.

## 2026-02-17 follow-up
- Re-checked official Copilot model page:
  - https://docs.github.com/en/copilot/reference/ai-models/supported-models
- Findings:
  - GitHub’s supported-models page changes frequently and model availability depends on org/plan.
  - Static fallback lists alone are insufficient; account-scoped discovery should override fallback whenever possible.
- Implementation direction taken:
  - Keep a docs-derived Copilot fallback list in gateway for cold-start behavior.
  - Prefer runtime discovery from Copilot `/models` using exchanged Copilot token/base URL.
  - Treat `github-copilot` as first-class provider in ROCI config resolution (no fallback to unrelated OpenAI/OpenAI-compatible credentials).
  - Ensure Copilot model requests send IDE headers (`Editor-Version`, `Editor-Plugin-Version`, `Copilot-Integration-Id`, `User-Agent`) to avoid `missing Editor-Version header for IDE auth` errors.
