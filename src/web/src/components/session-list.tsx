import { useState, useEffect, useCallback } from 'react';
import type { SessionInfo } from '@/lib/protocol';
import { loadPreview, removePreview } from '@/lib/session-previews';
import { Terminal, Play, X, RefreshCw, Trash2 } from 'lucide-react';

interface SessionListProps {
  call: (method: string, params?: unknown) => Promise<unknown>;
  status: string;
  onAttach: (sessionId: string) => void;
  previewNamespace: string;
}

interface SessionListResponse {
  sessions: SessionInfo[];
}

export function SessionList({ call, status, onAttach, previewNamespace }: SessionListProps) {
  const [sessions, setSessions] = useState<SessionInfo[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchSessions = useCallback(async () => {
    if (status !== 'connected') return;
    setLoading(true);
    try {
      const res = await call('terminal.session.list') as SessionListResponse;
      setSessions(res.sessions || []);
      setError(null);
    } catch (err: unknown) {
      console.error('Failed to list sessions:', err);
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || 'Failed to list sessions');
    } finally {
      setLoading(false);
    }
  }, [call, status]);

  useEffect(() => {
    if (status === 'connected') {
      fetchSessions();
      // Poll every 5 seconds to keep the list fresh
      const interval = setInterval(fetchSessions, 5000);
      return () => clearInterval(interval);
    }
  }, [status, fetchSessions]);

  const handleStart = async () => {
    try {
        const session = await call('terminal.session.start', {
            cols: 80,
            rows: 24
        }) as SessionInfo;
        
        // Refresh list
        fetchSessions();
        
        // Auto-attach
        if (session && session.session_id) {
            await call('terminal.session.attach', { session_id: session.session_id });
            onAttach(session.session_id);
        }
    } catch (err: unknown) {
        console.error("Failed to start session", err);
        const msg = err instanceof Error ? err.message : String(err);
        alert('Failed to start session: ' + msg);
    }
  };

  const handleKill = async (id: string) => {
      if (!confirm('Are you sure you want to kill this session?')) return;
      try {
          await call('terminal.session.kill', { session_id: id });
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

  const handleAttach = async (id: string) => {
      try {
          await call('terminal.session.attach', { session_id: id });
          onAttach(id);
      } catch (err: unknown) {
          const msg = err instanceof Error ? err.message : String(err);
          alert('Failed to attach session: ' + msg);
      }
  };

  if (status !== 'connected') {
      return null;
  }

  const activeSessions = sessions.filter((s) => s.status === 'active');
  const inactiveSessions = sessions.filter((s) => s.status !== 'active');

  return (
    <div className="mt-6 w-full">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-semibold flex items-center gap-2 text-foreground">
           <Terminal className="w-5 h-5" />
           Sessions
        </h2>
        <div className="flex gap-2">
            <button 
                onClick={fetchSessions} 
                disabled={loading}
                className="p-2 min-h-[44px] min-w-[44px] bg-muted hover:bg-muted/80 rounded text-muted-foreground disabled:opacity-50 transition-colors"
                title="Refresh"
                aria-label="Refresh sessions"
            >
                <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin motion-reduce:animate-none' : ''}`} />
            </button>
            <button 
                onClick={handleStart}
                className="flex items-center gap-1 px-3 py-2 min-h-[44px] bg-primary hover:bg-primary/90 rounded text-primary-foreground text-sm font-medium transition-colors"
            >
                <Play className="w-4 h-4" />
                New Session
            </button>
        </div>
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
            {activeSessions.length === 0 ? (
              <div className="text-sm text-muted-foreground bg-muted/30 rounded-lg border border-border border-dashed p-4">
                No active sessions
              </div>
            ) : (
              <div className="grid gap-3 md:grid-cols-2">
                {activeSessions.map(session => {
                  const preview = loadPreview(previewNamespace, session.session_id)?.text ?? "";
                  return (
                    <div key={session.session_id} className="bg-card p-4 rounded-lg border border-border shadow-sm flex flex-col gap-3">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <span className="w-2 h-2 rounded-full bg-green-500" title="active" />
                          <span className="font-mono text-sm text-foreground" title={session.session_id}>
                            {session.session_id.substring(0, 8)}...
                          </span>
                          <span className="text-xs px-2 py-0.5 bg-muted rounded text-muted-foreground font-mono">
                            {session.shell}
                          </span>
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
                          onClick={() => handleAttach(session.session_id)}
                          className="flex-1 px-3 py-2 min-h-[44px] bg-muted hover:bg-muted/80 rounded text-xs font-medium text-foreground transition-colors"
                        >
                          Attach
                        </button>
                        <button 
                          onClick={() => handleKill(session.session_id)}
                          className="px-3 py-2 min-h-[44px] min-w-[44px] text-muted-foreground hover:bg-destructive/20 hover:text-destructive rounded transition-colors flex items-center justify-center"
                          title="Kill Session"
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
                {inactiveSessions.map(session => {
                  const preview = loadPreview(previewNamespace, session.session_id)?.text ?? "";
                  return (
                    <div key={session.session_id} className="bg-card p-4 rounded-lg border border-border shadow-sm flex flex-col gap-3">
                      <div className="flex items-center justify-between">
                        <div className="flex items-center gap-2">
                          <span className="w-2 h-2 rounded-full bg-muted-foreground" title={session.status} />
                          <span className="font-mono text-sm text-foreground" title={session.session_id}>
                            {session.session_id.substring(0, 8)}...
                          </span>
                          <span className="text-xs px-2 py-0.5 bg-muted rounded text-muted-foreground font-mono">
                            {session.shell}
                          </span>
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
                          onClick={() => handleAttach(session.session_id)}
                          className="flex-1 px-3 py-2 min-h-[44px] bg-muted hover:bg-muted/80 rounded text-xs font-medium text-foreground transition-colors"
                        >
                          Resume
                        </button>
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
