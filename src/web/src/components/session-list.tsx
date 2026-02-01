import { useState, useEffect, useCallback } from 'react';
import type { SessionInfo } from '@/lib/protocol';
import { Terminal, Play, X, RefreshCw } from 'lucide-react';

interface SessionListProps {
  call: (method: string, params?: unknown) => Promise<unknown>;
  status: string;
}

interface SessionListResponse {
  sessions: SessionInfo[];
}

export function SessionList({ call, status }: SessionListProps) {
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
        await call('terminal.session.start', {
            shell: '/bin/bash', // or default
            cols: 80,
            rows: 24
        });
        fetchSessions();
    } catch (err: unknown) {
        console.error("Failed to start session", err);
        const msg = err instanceof Error ? err.message : String(err);
        
        // If /bin/bash fails, try /bin/sh
        if (msg && msg.includes("spawn")) {
            try {
                 await call('terminal.session.start', {
                    shell: '/bin/sh',
                    cols: 80,
                    rows: 24
                });
                fetchSessions();
                return;
            } catch (retryErr) {
                console.error("Failed to start session with /bin/sh", retryErr);
            }
        }
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

  const handleAttach = (id: string) => {
      // For now, just alert or console log as we don't have the terminal UI yet
      console.log('Attaching to session:', id);
      alert(`Attach to session ${id} coming soon!`);
  };

  if (status !== 'connected') {
      return null;
  }

  return (
    <div className="mt-6 w-full">
      <div className="flex items-center justify-between mb-4">
        <h2 className="text-xl font-semibold flex items-center gap-2 text-gray-200">
           <Terminal className="w-5 h-5" />
           Sessions
        </h2>
        <div className="flex gap-2">
            <button 
                onClick={fetchSessions} 
                disabled={loading}
                className="p-2 bg-gray-700 hover:bg-gray-600 rounded text-gray-200 disabled:opacity-50 transition-colors"
                title="Refresh"
            >
                <RefreshCw className={`w-4 h-4 ${loading ? 'animate-spin' : ''}`} />
            </button>
            <button 
                onClick={handleStart}
                className="flex items-center gap-1 px-3 py-2 bg-blue-600 hover:bg-blue-500 rounded text-white text-sm font-medium transition-colors"
            >
                <Play className="w-4 h-4" />
                New Session
            </button>
        </div>
      </div>

      {error && (
          <div className="mb-4 p-3 bg-red-900/30 border border-red-800 rounded text-red-200 text-sm">
              {error}
          </div>
      )}

      {sessions.length === 0 ? (
          <div className="text-center py-8 text-gray-500 bg-gray-800/50 rounded-lg border border-gray-700 border-dashed">
              No active sessions
          </div>
      ) : (
          <div className="space-y-3">
              {sessions.map(session => (
                  <div key={session.session_id} className="bg-gray-800 p-4 rounded-lg border border-gray-700 flex items-center justify-between shadow-sm">
                      <div>
                          <div className="flex items-center gap-2 mb-1">
                              <span className={`w-2 h-2 rounded-full ${session.status === 'Active' ? 'bg-green-500' : 'bg-gray-500'}`} title={session.status} />
                              <span className="font-mono text-sm text-gray-300" title={session.session_id}>{session.session_id.substring(0, 8)}...</span>
                              <span className="text-xs px-2 py-0.5 bg-gray-700 rounded text-gray-400 font-mono">{session.shell}</span>
                          </div>
                          <div className="text-xs text-gray-500">
                              Started: {session.started_at} | Size: {session.cols}x{session.rows}
                          </div>
                      </div>
                      <div className="flex gap-2">
                           <button 
                                onClick={() => handleAttach(session.session_id)}
                                className="px-3 py-1.5 bg-gray-700 hover:bg-gray-600 rounded text-xs font-medium text-gray-200 transition-colors"
                           >
                               Attach
                           </button>
                           <button 
                                onClick={() => handleKill(session.session_id)}
                                className="p-1.5 text-red-400 hover:bg-red-900/30 hover:text-red-300 rounded transition-colors"
                                title="Kill Session"
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
