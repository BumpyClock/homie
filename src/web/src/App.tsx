import { useEffect, useRef, useState } from 'react';
import { useGateway, type ConnectionStatus } from '@/hooks/use-gateway'
import { useTargets } from '@/hooks/use-targets'
import { TargetSelector } from '@/components/target-selector'
import { SessionList } from '@/components/session-list'
import { TerminalView } from '@/components/terminal-view'
import { ThemeSelector } from '@/components/theme-selector'
import { ArrowLeft } from 'lucide-react';

function StatusBadge({ status }: { status: ConnectionStatus }) {
  const colors: Record<ConnectionStatus, string> = {
    disconnected: "bg-gray-500",
    connecting: "bg-yellow-500",
    handshaking: "bg-blue-500",
    connected: "bg-green-500",
    error: "bg-destructive",
    rejected: "bg-destructive",
  };

  return (
    <div className={`flex items-center gap-2 px-3 py-1 rounded-full text-white text-sm font-medium ${colors[status]}`}>
      <div className="w-2 h-2 rounded-full bg-white animate-pulse motion-reduce:animate-none" />
      <span className="capitalize">{status}</span>
    </div>
  );
}

function App() {
  const {
    targets,
    activeTarget,
    activeTargetId,
    setActiveTargetId,
    addTarget,
    removeTarget,
    hideLocal,
    restoreLocal
  } = useTargets();
  const { status, serverHello, rejection, error, call, onBinaryMessage } = useGateway({ url: activeTarget?.url ?? "" });
  const [attachedSessionIds, setAttachedSessionIds] = useState<string[]>([]);
  const prevAttachedRef = useRef<string[]>([]);
  const previewNamespace = activeTargetId ?? "default";

  useEffect(() => {
    setAttachedSessionIds([]);
  }, [activeTargetId]);

  useEffect(() => {
    const prev = prevAttachedRef.current;
    const removed = prev.filter((id) => !attachedSessionIds.includes(id));
    if (removed.length > 0 && status === "connected") {
      removed.forEach((id) => {
        void call("terminal.session.detach", { session_id: id }).catch(() => {});
      });
    }
    prevAttachedRef.current = attachedSessionIds;
  }, [attachedSessionIds, status, call]);

  const handleAttach = (sessionId: string) => {
    if (!attachedSessionIds.includes(sessionId)) {
        setAttachedSessionIds([...attachedSessionIds, sessionId]);
    }
  };

  const handleDetach = (sessionId: string) => {
      setAttachedSessionIds(prev => prev.filter(id => id !== sessionId));
  };

  // If we have attached sessions, show the Terminal View (Full Screen)
  if (attachedSessionIds.length > 0) {
      return (
          <div className="h-screen w-screen flex flex-col bg-background overflow-hidden">
              <div className="flex items-center justify-between p-2 bg-muted/50 border-b border-border shrink-0">
                  <div className="flex items-center gap-4">
                    <button 
                        onClick={() => setAttachedSessionIds([])}
                        className="p-1 hover:bg-muted rounded text-muted-foreground hover:text-foreground"
                        title="Back to Dashboard"
                        aria-label="Back to dashboard"
                    >
                        <ArrowLeft size={20} />
                    </button>
                    <h1 className="text-sm font-bold text-foreground">Homie Terminal</h1>
                  </div>
                  <div className="flex items-center gap-4">
                      <ThemeSelector />
                      <StatusBadge status={status} />
                  </div>
              </div>
              <div className="flex-1 min-h-0">
                  <TerminalView 
                    attachedSessionIds={attachedSessionIds}
                    onDetach={handleDetach}
                    call={call}
                    onBinaryMessage={onBinaryMessage}
                    previewNamespace={previewNamespace}
                  />
              </div>
          </div>
      );
  }

  // Dashboard View
  return (
    <div className="min-h-screen bg-background text-foreground">
      <header className="border-b border-border bg-card/40 backdrop-blur">
        <div className="flex items-center justify-between px-6 py-4">
          <div className="flex items-center gap-3">
            <div className="text-lg font-semibold">Homie Web</div>
            <div className="text-xs text-muted-foreground">Gateway Console</div>
          </div>
          <div className="flex items-center gap-4">
            <ThemeSelector />
            <StatusBadge status={status} />
          </div>
        </div>
      </header>

      <main className="px-6 py-6 grid gap-6 lg:grid-cols-[320px_1fr]">
        <section className="space-y-6">
          <div className="bg-card/60 border border-border rounded-lg p-4 shadow-sm">
            <TargetSelector 
              targets={targets}
              activeTargetId={activeTargetId}
              onSelect={setActiveTargetId}
              onAdd={addTarget}
              onDelete={removeTarget}
              hideLocal={hideLocal}
              onRestoreLocal={restoreLocal}
            />
          </div>

          {serverHello && (
            <div className="bg-card/60 border border-border rounded-lg p-4 text-sm shadow-sm">
              <h3 className="font-semibold mb-2 text-primary">Gateway Info</h3>
              <div className="grid grid-cols-2 gap-2">
                <span className="text-muted-foreground">ID:</span>
                <span>{serverHello.server_id}</span>
                <span className="text-muted-foreground">Protocol:</span>
                <span>v{serverHello.protocol_version}</span>
                {serverHello.identity && (
                  <>
                    <span className="text-muted-foreground">Identity:</span>
                    <span>{serverHello.identity}</span>
                  </>
                )}
              </div>
              <h4 className="font-semibold mt-3 mb-1 text-foreground">Services</h4>
              <ul className="list-disc list-inside text-muted-foreground">
                {serverHello.services.map((s, i) => (
                  <li key={i}>{s.service} (v{s.version})</li>
                ))}
              </ul>
            </div>
          )}

          {rejection && (
            <div className="p-4 bg-destructive/20 border border-destructive rounded-md text-sm text-destructive-foreground">
              <h3 className="font-semibold mb-1">Connection Rejected</h3>
              <p>Reason: {rejection.reason}</p>
              <p className="text-xs mt-1 opacity-70">Code: {rejection.code}</p>
            </div>
          )}
          
          {error && status === 'error' && (
            <div className="p-4 bg-destructive/20 border border-destructive rounded-md text-sm text-destructive-foreground">
              <p>Connection failed. Retrying...</p>
            </div>
          )}
        </section>

        <section className="min-h-[400px]">
          {serverHello && (
            <SessionList call={call} status={status} onAttach={handleAttach} previewNamespace={previewNamespace} />
          )}
        </section>
      </main>
    </div>
  )
}

export default App
