export function tmuxSessionName(shell) {
    if (!shell)
        return undefined;
    if (!shell.startsWith("tmux:"))
        return undefined;
    const name = shell.slice("tmux:".length).trim();
    return name.length > 0 ? name : undefined;
}
export function shortSessionId(sessionId) {
    if (!sessionId)
        return "";
    return sessionId.length > 8 ? `${sessionId.slice(0, 8)}...` : sessionId;
}
export function sessionDisplayName(session) {
    const tmuxName = tmuxSessionName(session.shell);
    if (tmuxName)
        return tmuxName;
    const name = typeof session.name === "string" ? session.name.trim() : "";
    if (name)
        return name;
    return shortSessionId(session.session_id);
}
export function partitionTerminalSessions(sessions) {
    return sessions.reduce((acc, session) => {
        if (session.status === "active") {
            acc.active.push(session);
        }
        else {
            acc.history.push(session);
        }
        return acc;
    }, { active: [], history: [] });
}
export function activeTmuxNamesFromSessions(sessions) {
    const names = new Set();
    for (const session of sessions) {
        const name = tmuxSessionName(session.shell);
        if (name)
            names.add(name);
    }
    return names;
}
export function filterAvailableTmuxSessions(tmuxSessions, activeSessions) {
    const activeNames = activeTmuxNamesFromSessions(activeSessions);
    return tmuxSessions.filter((session) => !activeNames.has(session.name));
}
//# sourceMappingURL=terminal-sessions.js.map