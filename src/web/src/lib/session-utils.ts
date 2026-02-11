export {
  sessionDisplayName,
  tmuxSessionName,
  type SessionInfo,
} from "@homie/shared";

export type TmuxCloseBehavior = "detach" | "kill";
export type PreviewRefresh = "10s" | "30s" | "1m" | "5m" | "15m" | "never";

export const PREVIEW_REFRESH_KEY = "homie-preview-refresh";
export const PREVIEW_MAX_BYTES = 65536;

export const PREVIEW_OPTIONS: { label: string; value: PreviewRefresh; ms: number | null }[] = [
  { label: "10s", value: "10s", ms: 10_000 },
  { label: "30s", value: "30s", ms: 30_000 },
  { label: "1m", value: "1m", ms: 60_000 },
  { label: "5m", value: "5m", ms: 300_000 },
  { label: "15m", value: "15m", ms: 900_000 },
  { label: "Never", value: "never", ms: null },
];

const TMUX_CLOSE_KEY = "homie-tmux-close-behavior";

export function normalizeRpcError(err: unknown): { code: number; message: string } | undefined {
  if (err && typeof err === "object") {
    const code = (err as { code?: number }).code;
    const message = (err as { message?: string }).message;
    if (typeof code === "number") {
      return { code, message: typeof message === "string" ? message : "" };
    }
  }
  return undefined;
}

export function shortSessionId(sessionId: string): string {
  if (!sessionId) return "";
  return sessionId.length > 8 ? `${sessionId.slice(0, 8)}...` : sessionId;
}

export function resolveTmuxCloseBehavior(): TmuxCloseBehavior {
  if (typeof window === "undefined") return "detach";
  try {
    const stored = window.localStorage.getItem(TMUX_CLOSE_KEY);
    if (stored === "detach" || stored === "kill") {
      return stored;
    }
    const kill = window.confirm(
      "When closing a tmux session, do you want to kill the tmux session? OK = kill tmux session, Cancel = detach (leave running)."
    );
    const choice: TmuxCloseBehavior = kill ? "kill" : "detach";
    window.localStorage.setItem(TMUX_CLOSE_KEY, choice);
    return choice;
  } catch {
    return "detach";
  }
}
