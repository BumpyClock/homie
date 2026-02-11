import type { SessionInfo, TmuxSessionInfo } from "./protocol";
export interface PartitionedTerminalSessions {
    active: SessionInfo[];
    history: SessionInfo[];
}
export declare function tmuxSessionName(shell?: string | null): string | undefined;
export declare function shortSessionId(sessionId: string): string;
export declare function sessionDisplayName(session: Pick<SessionInfo, "session_id" | "shell" | "name">): string;
export declare function partitionTerminalSessions(sessions: readonly SessionInfo[]): PartitionedTerminalSessions;
export declare function activeTmuxNamesFromSessions(sessions: readonly Pick<SessionInfo, "shell">[]): Set<string>;
export declare function filterAvailableTmuxSessions(tmuxSessions: readonly TmuxSessionInfo[], activeSessions: readonly Pick<SessionInfo, "shell">[]): TmuxSessionInfo[];
//# sourceMappingURL=terminal-sessions.d.ts.map