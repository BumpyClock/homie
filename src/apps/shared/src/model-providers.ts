export interface ModelProviderLike {
  model: string;
  provider?: string | null;
}

export function modelProviderId(model: ModelProviderLike): string {
  const fromProvider = (model.provider ?? "").trim();
  if (fromProvider.length > 0) return fromProvider.toLowerCase();
  const selector = (model.model ?? "").trim();
  if (selector.includes(":")) {
    return selector.split(":", 1)[0].toLowerCase();
  }
  return "other";
}

export function modelProviderLabel(model: ModelProviderLike): string {
  switch (modelProviderId(model)) {
    case "openai-codex":
      return "OpenAI Codex";
    case "github-copilot":
      return "GitHub Copilot";
    case "openai-compatible":
    case "openai_compatible":
      return "OpenAI-Compatible / Local";
    case "openai":
      return "OpenAI";
    case "anthropic":
    case "claude-code":
    case "claude_code":
      return "Claude";
    case "ollama":
      return "Ollama";
    case "lmstudio":
      return "LM Studio";
    default:
      return "Other";
  }
}
