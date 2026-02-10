import { useState, useEffect, useCallback, useRef } from 'react';
import type { SessionInfo, SessionPreviewResponse, TmuxListResponse, TmuxSessionInfo } from "@homie/shared";
import { loadPreview, removePreview, savePreview } from '@/lib/session-previews';
import {
  normalizeRpcError,
  PREVIEW_MAX_BYTES,
  PREVIEW_OPTIONS,
  resolveTmuxCloseBehavior,
  sessionDisplayName,
  tmuxSessionName,
  type PreviewRefresh,
} from '@/lib/session-utils';
import { Pencil, Terminal, Trash2, X } from 'lucide-react';

// Low-frequency reconcile; real-time updates come from gateway events.
const SESSION_LIST_POLL_MS = 60_000;
const PREVIEW_TICK_MAX = 2;
const PREVIEW_CATCHUP_DELAY_MS = 1_000;

interface SessionListProps {
  call: (method: string, params?: unknown) => Promise<unknown>;
  status: string;
  onAttach: (session: SessionInfo) => void;
  onRename?: (sessionId: string, name: string | null) => void;
  previewNamespace: string;
  previewRefresh: PreviewRefresh;
  refreshToken?: number;
}

interface SessionListResponse {
  sessions: SessionInfo[];
}

export function SessionList({ call, status, onAttach, onRename, previewNamespace, previewRefresh, refreshToken }: SessionListProps) {
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const sessionsRef = useRef<SessionInfo[]>([]);
  const statusRef = useRef(status);
  const previewRefreshRef = useRef(previewRefresh);
  const previewNamespaceRef = useRef(previewNamespace);

  const previewTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const previewNextAtRef = useRef<number | null>(null);
  const previewRunningRef = useRef(false);
  const runPreviewTickRef = useRef<() => void>(() => {});

  const [error, setError] = useState<string | null>(null);
  const [tmuxSessions, setTmuxSessions] = useState<TmuxSessionInfo[]>([]);
  const [tmuxSupported, setTmuxSupported] = useState(false);
  const [tmuxError, setTmuxError] = useState<string | null>(null);

  const fetchSessions = useCallback(async () => {
    if (status !== 'connected') return;
    try {
      const res = await call('terminal.session.list') as SessionListResponse;
      setSessions(res.sessions || []);
      setError(null);
    } catch (err: unknown) {
      console.error('Failed to list sessions:', err);
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Failed to list sessions');
    }
  }, [call, status]);

  const fetchTmux = useCallback(async () => {
    if (status !== 'connected') return;
    try {
      const res = await call('terminal.tmux.list') as TmuxListResponse;
      setTmuxSupported(res.supported);
      setTmuxSessions(res.sessions || []);
      setTmuxError(null);
    } catch (err: unknown) {
      const rpc = normalizeRpcError(err);
      if (rpc?.code === -32601) {
        setTmuxSupported(false);
        setTmuxSessions([]);
        setTmuxError(null);
      } else {
        const msg = err instanceof Error ? err.message : String(err);
        setTmuxError(msg || 'Failed to list tmux sessions');
      }
    }
  }, [call, status]);

  const schedulePreviewTick = (delayMs: number) => {
    if (statusRef.current !== "connected") return;
    const at = Date.now() + delayMs;
    const currentAt = previewNextAtRef.current;
    if (currentAt !== null && currentAt <= at) return;

    previewNextAtRef.current = at;
    if (previewTimerRef.current) clearTimeout(previewTimerRef.current);
    previewTimerRef.current = setTimeout(() => {
      previewNextAtRef.current = null;
      runPreviewTickRef.current();
    }, Math.max(0, at - Date.now()));
  };

  const fetchPreview = useCallback(async (namespace: string, sessionId: string) => {
    try {
      const res = await call('terminal.session.preview', { session_id: sessionId, max_bytes: PREVIEW_MAX_BYTES }) as SessionPreviewResponse;
      if (typeof res?.text === "string") {
        const text = res.text.trimEnd();
        savePreview(namespace, sessionId, text);
      }
    } catch {
      return;
    }
  }, [call]);

  const runPreviewTick = useCallback(async () => {
    if (previewRunningRef.current) return;
    if (statusRef.current !== "connected") return;

    previewRunningRef.current = true;
    try {
      const cadence = PREVIEW_OPTIONS.find((o) => o.value === previewRefreshRef.current)?.ms ?? null;
      const namespace = previewNamespaceRef.current;
      const items = sessionsRef.current;
      const now = Date.now();

      const missing: string[] = [];
      const due: string[] = [];

      for (const session of items) {
        if (session.status !== "active") continue;
        const existing = loadPreview(namespace, session.session_id);
        if (!existing) {
          missing.push(session.session_id);
          continue;
        }
        if (cadence === null) continue;
        if (now - existing.capturedAt >= cadence) {
          due.push(session.session_id);
        }
      }

      const toFetch = [...missing, ...due].slice(0, PREVIEW_TICK_MAX);
      for (const id of toFetch) {
        if (statusRef.current !== "connected") return;
        await fetchPreview(namespace, id);
      }

      const remaining = missing.length + due.length - toFetch.length;
      if (remaining > 0) {
        schedulePreviewTick(PREVIEW_CATCHUP_DELAY_MS);
      } else if (cadence !== null) {
        schedulePreviewTick(cadence);
      }
    } finally {
      previewRunningRef.current = false;
    }
  }, [fetchPreview]);

  useEffect(() => {
    runPreviewTickRef.current = () => {
      void runPreviewTick();
    };
  }, [runPreviewTick]);

  useEffect(() => {
    statusRef.current = status;
  }, [status]);

  useEffect(() => {
    previewRefreshRef.current = previewRefresh;
  }, [previewRefresh]);

  useEffect(() => {
    previewNamespaceRef.current = previewNamespace;
  }, [previewNamespace]);

  useEffect(() => {
    if (status !== 'connected') return;
    let cancelled = false;
    let timer: ReturnType<typeof setTimeout> | null = null;

    const poll = async () => {
      if (cancelled) return;
      await fetchSessions();
      await fetchTmux();
      if (cancelled) return;
      timer = setTimeout(poll, SESSION_LIST_POLL_MS);
    };

    void poll();
    return () => {
      cancelled = true;
      if (timer) clearTimeout(timer);
    };
  }, [status, fetchSessions, fetchTmux]);

  useEffect(() => {
    sessionsRef.current = sessions;
    schedulePreviewTick(0);
  }, [sessions]);

  useEffect(() => {
    if (status !== "connected") return;
    schedulePreviewTick(0);
    return () => {
      if (previewTimerRef.current) clearTimeout(previewTimerRef.current);
      previewTimerRef.current = null;
      previewNextAtRef.current = null;
      previewRunningRef.current = false;
    };
  }, [status, previewRefresh, previewNamespace]);

  const handleTmuxAttach = async (name: string) => {
    try {
      const session = await call('terminal.tmux.attach', {
        session_name: name,
        cols: 80,
        rows: 24,
      }) as SessionInfo;
      fetchSessions();
      if (session && session.session_id) {
        onAttach(session);
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      alert('Failed to attach tmux session: ' + msg);
    }
  };

  const handleTmuxKill = async (name: string) => {
    if (!confirm(`Kill tmux session "${name}"?`)) return;
    try {
      await call('terminal.tmux.kill', { session_name: name });
      fetchTmux();
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      alert('Failed to kill tmux session: ' + msg);
    }
  };

  const handleKill = async (session: SessionInfo) => {
      const tmuxName = tmuxSessionName(session.shell);
      if (tmuxName) {
          const behavior = resolveTmuxCloseBehavior();
          if (behavior === "kill") {
              try {
                  await call('terminal.tmux.kill', { session_name: tmuxName });
                  fetchTmux();
              } catch (err: unknown) {
                  const msg = err instanceof Error ? err.message : String(err);
                  alert('Failed to kill tmux session: ' + msg);
                  return;
              }
          }
          try {
              await call('terminal.session.kill', { session_id: session.session_id });
              fetchSessions();
              fetchTmux();
          } catch (err: unknown) {
              const msg = err instanceof Error ? err.message : String(err);
              alert('Failed to close session: ' + msg);
          }
          return;
      }
      if (!confirm('Are you sure you want to kill this session?')) return;
      try {
          await call('terminal.session.kill', { session_id: session.session_id });
          fetchSessions();
      } catch (err: unknown) {
          const msg = err instanceof Error ? err.message : String(err);
          alert('Failed to kill session: ' + msg);
      }
  };

  const handleRemove = async (id: string) => {
      if (!confirm('Remove this session from history?')) return;
      try {
          await call('terminal.session.remove', { session_id: id });
          removePreview(previewNamespace, id);
          fetchSessions();
      } catch (err: unknown) {
          const msg = err instanceof Error ? err.message : String(err);
          alert('Failed to remove session: ' + msg);
      }
  };

  const handleRename = async (session: SessionInfo) => {
      if (tmuxSessionName(session.shell)) return;
      const current = typeof session.name === "string" ? session.name : "";
      const next = window.prompt("Rename session", current);
      if (next === null) return;
      const trimmed = next.trim();
      const name = trimmed.length > 0 ? trimmed : null;
      try {
          await call('terminal.session.rename', { session_id: session.session_id, name });
          setSessions((prev) =>
              prev.map((item) =>
                  item.session_id === session.session_id ? { ...item, name } : item
              )
          );
          onRename?.(session.session_id, name);
      } catch (err: unknown) {
          const msg = err instanceof Error ? err.message : String(err);
          alert('Failed to rename session: ' + msg);
      }
  };

  const handleAttach = async (session: SessionInfo) => {
      onAttach(session);
  };

  const handleRefresh = useCallback(() => {
    fetchSessions();
    fetchTmux();
    schedulePreviewTick(0);
  }, [fetchSessions, fetchTmux]);

  useEffect(() => {
    if (refreshToken === undefined) return;
    handleRefresh();
  }, [refreshToken, handleRefresh]);

  if (status !== 'connected') {
      return null;
  }

  const activeSessions = sessions.filter((s) => s.status === 'active');
  const inactiveSessions = sessions.filter((s) => s.status !== 'active');
  const activeTmuxNames = new Set(
    activeSessions
      .map((s) => tmuxSessionName(s.shell))
      .filter((v): v is string => !!v)
  );
  const availableTmux = tmuxSupported
    ? tmuxSessions.filter((s) => !activeTmuxNames.has(s.name))
    : [];

  return (
    <div className="mt-6 w-full">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-semibold flex items-center gap-2 text-foreground">
           <Terminal className="w-5 h-5" />
           Sessions
        </h2>
      </div>

      {error && (
          <div className="mb-4 p-3 bg-destructive/20 border border-destructive rounded text-destructive-foreground text-sm">
              {error}
          </div>
      )}

      {sessions.length === 0 ? (
        <div className="text-center py-8 text-muted-foreground bg-muted/30 rounded-lg border border-border border-dashed">
          No sessions yet
        </div>
      ) : (
        <div className="space-y-6">
          <div>
            <div className="text-xs uppercase tracking-wide text-muted-foreground mb-3">Active Sessions</div>
            {tmuxSupported && tmuxError && (
              <div className="mb-3 p-3 bg-destructive/20 border border-destructive rounded text-destructive-foreground text-sm">
                {tmuxError}
              </div>
            )}
            {(activeSessions.length + availableTmux.length) === 0 ? (
              <div className="text-sm text-muted-foreground bg-muted/30 rounded-lg border border-border border-dashed p-4">
                No active sessions
              </div>
            ) : (
              <div className="grid gap-3 md:grid-cols-2">
                {availableTmux.map((session) => (
                  <div key={`tmux:${session.name}`} className="bg-card p-4 rounded-lg border border-border shadow-sm flex flex-col gap-3">
                    <div className="flex items-center justify-between">
                      <div className="flex items-center gap-2">
                        <span className={`w-2 h-2 rounded-full ${session.attached ? 'bg-green-500' : 'bg-muted-foreground'}`} />
                        <span className="font-mono text-sm text-foreground" title={session.name}>
                          {session.name}
                        </span>
                        <span className="text-xs px-2 py-0.5 bg-muted rounded text-muted-foreground font-mono">tmux</span>
                      </div>
                      <div className="text-xs text-muted-foreground">{session.windows} win</div>
                    </div>
                    <div className="bg-muted/40 border border-border rounded-md p-3 text-xs font-mono text-muted-foreground whitespace-pre-wrap max-h-28 overflow-hidden">
                      Preview available after attach.
                    </div>
                    <div className="flex gap-2">
                      <button
                        onClick={() => handleTmuxAttach(session.name)}
                        className="flex-1 px-3 py-2 min-h-[44px] bg-muted hover:bg-muted/80 rounded text-xs font-medium text-foreground transition-colors"
                      >
                        Attach
                      </button>
                      <button
                        onClick={() => handleTmuxKill(session.name)}
                        className="px-3 py-2 min-h-[44px] min-w-[44px] text-muted-foreground hover:bg-destructive/20 hover:text-destructive rounded transition-colors flex items-center justify-center"
                        title="Kill tmux session"
                        aria-label="Kill tmux session"
                      >
                        <X className="w-4 h-4" />
                      </button>
                    </div>
                  </div>
                ))}
                {activeSessions.map((session) => {
                  const preview = loadPreview(previewNamespace, session.session_id)?.text ?? "";
                  const tmuxName = tmuxSessionName(session.shell);
                  const displayName = sessionDisplayName(session);
                  return (
                    <div key={session.session_id} className="bg-card p-4 rounded-lg border border-border shadow-sm flex flex-col gap-3">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <span className="w-2 h-2 rounded-full bg-green-500" title="active" />
                          <span className="font-mono text-sm text-foreground max-w-[220px] truncate" title={session.session_id}>
                            {displayName}
                          </span>
                          {tmuxName ? (
                            <span className="text-xs px-2 py-0.5 bg-muted rounded text-muted-foreground font-mono">
                              tmux
                            </span>
                          ) : (
                            <span className="text-xs px-2 py-0.5 bg-muted rounded text-muted-foreground font-mono">
                              {session.shell}
                            </span>
                          )}
                        </div>
                        <div className="text-xs text-muted-foreground">{session.cols}x{session.rows}</div>
                      </div>
                      <div className="text-xs text-muted-foreground">
                        Started: {session.started_at}
                      </div>
                      <div className="bg-muted/40 border border-border rounded-md p-3 text-xs font-mono text-muted-foreground whitespace-pre-wrap max-h-28 overflow-hidden">
                        {preview.trim().length > 0 ? preview : "Preview available after first detach."}
                      </div>
                      <div className="flex gap-2">
                        <button
                          onClick={() => handleAttach(session)}
                          className="flex-1 px-3 py-2 min-h-[44px] bg-muted hover:bg-muted/80 rounded text-xs font-medium text-foreground transition-colors"
                        >
                          Attach
                        </button>
                        {!tmuxName && (
                          <button
                            onClick={() => handleRename(session)}
                            className="px-3 py-2 min-h-[44px] min-w-[44px] text-muted-foreground hover:bg-muted/60 hover:text-foreground rounded transition-colors flex items-center justify-center"
                            title="Rename session"
                            aria-label="Rename session"
                          >
                            <Pencil className="w-4 h-4" />
                          </button>
                        )}
                        <button
                          onClick={() => handleKill(session)}
                          className="px-3 py-2 min-h-[44px] min-w-[44px] text-muted-foreground hover:bg-destructive/20 hover:text-destructive rounded transition-colors flex items-center justify-center"
                          title="Kill session"
                          aria-label="Kill session"
                        >
                          <X className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  )
                })}
              </div>
            )}
          </div>

          <div>
            <div className="text-xs uppercase tracking-wide text-muted-foreground mb-3">History</div>
            {inactiveSessions.length === 0 ? (
              <div className="text-sm text-muted-foreground bg-muted/30 rounded-lg border border-border border-dashed p-4">
                No inactive sessions
              </div>
            ) : (
              <div className="grid gap-3 md:grid-cols-2">
                {inactiveSessions.map((session) => {
                  const preview = loadPreview(previewNamespace, session.session_id)?.text ?? "";
                  const tmuxName = tmuxSessionName(session.shell);
                  const displayName = sessionDisplayName(session);
                  return (
                    <div key={session.session_id} className="bg-card p-4 rounded-lg border border-border shadow-sm flex flex-col gap-3">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <span className="w-2 h-2 rounded-full bg-muted-foreground" title={session.status} />
                          <span className="font-mono text-sm text-foreground max-w-[220px] truncate" title={session.session_id}>
                            {displayName}
                          </span>
                          {tmuxName ? (
                            <span className="text-xs px-2 py-0.5 bg-muted rounded text-muted-foreground font-mono">
                              tmux
                            </span>
                          ) : (
                            <span className="text-xs px-2 py-0.5 bg-muted rounded text-muted-foreground font-mono">
                              {session.shell}
                            </span>
                          )}
                        </div>
                        <div className="text-xs text-muted-foreground capitalize">{session.status}</div>
                      </div>
                      <div className="text-xs text-muted-foreground">
                        Started: {session.started_at} | Size: {session.cols}x{session.rows}
                      </div>
                      <div className="bg-muted/40 border border-border rounded-md p-3 text-xs font-mono text-muted-foreground whitespace-pre-wrap max-h-28 overflow-hidden">
                        {preview.trim().length > 0 ? preview : "No preview captured."}
                      </div>
                      <div className="flex gap-2">
                        <button
                          onClick={() => handleAttach(session)}
                          className="flex-1 px-3 py-2 min-h-[44px] bg-muted hover:bg-muted/80 rounded text-xs font-medium text-foreground transition-colors"
                        >
                          Resume
                        </button>
                        {!tmuxName && (
                          <button
                            onClick={() => handleRename(session)}
                            className="px-3 py-2 min-h-[44px] min-w-[44px] text-muted-foreground hover:bg-muted/60 hover:text-foreground rounded transition-colors flex items-center justify-center"
                            title="Rename session"
                            aria-label="Rename session"
                          >
                            <Pencil className="w-4 h-4" />
                          </button>
                        )}
                        <button
                          onClick={() => handleRemove(session.session_id)}
                          className="px-3 py-2 min-h-[44px] min-w-[44px] text-muted-foreground hover:bg-destructive/20 hover:text-destructive rounded transition-colors flex items-center justify-center"
                          title="Remove from history"
                          aria-label="Remove from history"
                        >
                          <Trash2 className="w-4 h-4" />
                        </button>
                      </div>
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
