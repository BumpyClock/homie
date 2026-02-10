export interface ChatAccountStatus {
  ok: boolean;
  message: string;
}

export function resolveChatAccountStatus(account: Record<string, unknown> | null): ChatAccountStatus {
  if (!account) return { ok: false, message: "Not connected to Codex CLI." };
  const raw = account as {
    requiresOpenaiAuth?: boolean;
    requires_openai_auth?: boolean;
    account?: unknown;
  };
  const requires = raw.requiresOpenaiAuth ?? raw.requires_openai_auth ?? false;
  const hasAccount = !!raw.account;

  if (!hasAccount && requires) {
    return {
      ok: false,
      message: "Codex CLI not logged in. Run `codex login` on the gateway host.",
    };
  }

  if (!hasAccount) {
    return {
      ok: false,
      message: "Codex CLI not logged in. Run `codex login` on the gateway host.",
    };
  }

  return { ok: true, message: "" };
}
