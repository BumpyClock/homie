import type { SessionInfo, TmuxSessionInfo } from "./protocol";

export interface PartitionedTerminalSessions {
  active: SessionInfo[];
  history: SessionInfo[];
}

export function tmuxSessionName(shell?: string | null): string | undefined {
  if (!shell) return undefined;
  if (!shell.startsWith("tmux:")) return undefined;
  const name = shell.slice("tmux:".length).trim();
  return name.length > 0 ? name : undefined;
}

export function shortSessionId(sessionId: string): string {
  if (!sessionId) return "";
  return sessionId.length > 8 ? `${sessionId.slice(0, 8)}...` : sessionId;
}

export function sessionDisplayName(
  session: Pick<SessionInfo, "session_id" | "shell" | "name">,
): string {
  const tmuxName = tmuxSessionName(session.shell);
  if (tmuxName) return tmuxName;
  const name = typeof session.name === "string" ? session.name.trim() : "";
  if (name) return name;
  return shortSessionId(session.session_id);
}

export function partitionTerminalSessions(
  sessions: readonly SessionInfo[],
): PartitionedTerminalSessions {
  return sessions.reduce<PartitionedTerminalSessions>(
    (acc, session) => {
      if (session.status === "active") {
        acc.active.push(session);
      } else {
        acc.history.push(session);
      }
      return acc;
    },
    { active: [], history: [] },
  );
}

export function activeTmuxNamesFromSessions(
  sessions: readonly Pick<SessionInfo, "shell">[],
): Set<string> {
  const names = new Set<string>();
  for (const session of sessions) {
    const name = tmuxSessionName(session.shell);
    if (name) names.add(name);
  }
  return names;
}

export function filterAvailableTmuxSessions(
  tmuxSessions: readonly TmuxSessionInfo[],
  activeSessions: readonly Pick<SessionInfo, "shell">[],
): TmuxSessionInfo[] {
  const activeNames = activeTmuxNamesFromSessions(activeSessions);
  return tmuxSessions.filter((session) => !activeNames.has(session.name));
}
