import { useState, useEffect, useCallback } from 'react';
import type { SessionInfo } from '@/lib/protocol';
import { Terminal, Play, X, RefreshCw } from 'lucide-react';

interface SessionListProps {
  call: (method: string, params?: unknown) => Promise<unknown>;
  status: string;
  onAttach: (sessionId: string) => void;
}

interface SessionListResponse {
  sessions: SessionInfo[];
}

export function SessionList({ call, status, onAttach }: SessionListProps) {
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
                className="p-2 bg-muted hover:bg-muted/80 rounded text-muted-foreground disabled:opacity-50 transition-colors"
                title="Refresh"
                aria-label="Refresh sessions"
            >
                <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin motion-reduce:animate-none' : ''}`} />
            </button>
            <button 
                onClick={handleStart}
                className="flex items-center gap-1 px-3 py-2 bg-primary hover:bg-primary/90 rounded text-primary-foreground text-sm font-medium transition-colors"
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
              No active sessions
          </div>
      ) : (
          <div className="space-y-3">
              {sessions.map(session => (
                  <div key={session.session_id} className="bg-card p-4 rounded-lg border border-border flex items-center justify-between shadow-sm">
                      <div>
                          <div className="flex items-center gap-2 mb-1">
                              <span className={`w-2 h-2 rounded-full ${session.status === 'active' ? 'bg-green-500' : 'bg-muted-foreground'}`} title={session.status} />
                              <span className="font-mono text-sm text-foreground" title={session.session_id}>{session.session_id.substring(0, 8)}...</span>
                              <span className="text-xs px-2 py-0.5 bg-muted rounded text-muted-foreground font-mono">{session.shell}</span>
                          </div>
                          <div className="text-xs text-muted-foreground">
                              Started: {session.started_at} | Size: {session.cols}x{session.rows}
                          </div>
                      </div>
                      <div className="flex gap-2">
                           <button 
                                onClick={() => handleAttach(session.session_id)}
                                className="px-3 py-1.5 bg-muted hover:bg-muted/80 rounded text-xs font-medium text-foreground transition-colors"
                           >
                               Attach
                           </button>
                           <button 
                                onClick={() => handleKill(session.session_id)}
                                className="p-1.5 text-muted-foreground hover:bg-destructive/20 hover:text-destructive rounded transition-colors"
                                title="Kill Session"
                                aria-label="Kill session"
                           >
                               <X className="w-4 h-4" />
                           </button>
                      </div>
                  </div>
              ))}
          </div>
      )}
    </div>
  );
}
