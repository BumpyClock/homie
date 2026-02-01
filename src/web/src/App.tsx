import { useState } from 'react';
import { useGateway, type ConnectionStatus } from '@/hooks/use-gateway'
import { useTargets } from '@/hooks/use-targets'
import { TargetSelector } from '@/components/target-selector'
import { SessionList } from '@/components/session-list'
import { TerminalView } from '@/components/terminal-view'
import { ArrowLeft } from 'lucide-react';

function StatusBadge({ status }: { status: ConnectionStatus }) {
  const colors: Record<ConnectionStatus, string> = {
    disconnected: "bg-gray-500",
    connecting: "bg-yellow-500",
    handshaking: "bg-blue-500",
    connected: "bg-green-500",
    error: "bg-red-500",
    rejected: "bg-red-700",
  };

  return (
    <div className={`flex items-center gap-2 px-3 py-1 rounded-full text-white text-sm font-medium ${colors[status]}`}>
      <div className="w-2 h-2 rounded-full bg-white animate-pulse" />
      <span className="capitalize">{status}</span>
    </div>
  );
}

function App() {
  const { targets, activeTarget, activeTargetId, setActiveTargetId, addTarget, removeTarget } = useTargets();
  const { status, serverHello, rejection, error, call, onBinaryMessage } = useGateway({ url: activeTarget.url });
  const [attachedSessionIds, setAttachedSessionIds] = useState<string[]>([]);

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
          <div className="h-screen w-screen flex flex-col bg-gray-900 overflow-hidden">
              <div className="flex items-center justify-between p-2 bg-gray-800 border-b border-gray-700 shrink-0">
                  <div className="flex items-center gap-4">
                    <button 
                        onClick={() => setAttachedSessionIds([])}
                        className="p-1 hover:bg-gray-700 rounded text-gray-400 hover:text-white"
                        title="Back to Dashboard"
                    >
                        <ArrowLeft size={20} />
                    </button>
                    <h1 className="text-sm font-bold text-gray-300">Homie Terminal</h1>
                  </div>
                  <StatusBadge status={status} />
              </div>
              <div className="flex-1 min-h-0">
                  <TerminalView 
                    attachedSessionIds={attachedSessionIds}
                    onDetach={handleDetach}
                    call={call}
                    onBinaryMessage={onBinaryMessage}
                  />
              </div>
          </div>
      );
  }

  // Dashboard View
  return (
    <div className="min-h-screen bg-gray-900 text-gray-100 flex flex-col items-center justify-center p-4">
      <div className="max-w-md w-full bg-gray-800 rounded-lg shadow-xl p-6 border border-gray-700">
        <h1 className="text-2xl font-bold mb-6 text-center">Homie Web</h1>
        
        <div className="space-y-6">
          <TargetSelector 
            targets={targets}
            activeTargetId={activeTargetId}
            onSelect={setActiveTargetId}
            onAdd={addTarget}
            onDelete={removeTarget}
          />

          <div className="flex justify-between items-center border-b border-gray-700 pb-4">
             <span className="text-gray-400">Status</span>
             <StatusBadge status={status} />
          </div>

          {serverHello && (
            <>
                <div className="mt-4 p-4 bg-gray-900 rounded-md text-sm">
                  <h3 className="font-semibold mb-2 text-green-400">Gateway Info</h3>
                  <div className="grid grid-cols-2 gap-2">
                     <span className="text-gray-500">ID:</span>
                     <span>{serverHello.server_id}</span>
                     <span className="text-gray-500">Protocol:</span>
                     <span>v{serverHello.protocol_version}</span>
                     {serverHello.identity && (
                         <>
                            <span className="text-gray-500">Identity:</span>
                            <span>{serverHello.identity}</span>
                         </>
                     )}
                  </div>
                  
                  <h4 className="font-semibold mt-3 mb-1 text-gray-300">Services</h4>
                  <ul className="list-disc list-inside text-gray-400">
                      {serverHello.services.map((s, i) => (
                          <li key={i}>{s.service} (v{s.version})</li>
                      ))}
                  </ul>
                </div>
                
                <SessionList call={call} status={status} onAttach={handleAttach} />
            </>
          )}

          {rejection && (
              <div className="mt-4 p-4 bg-red-900/20 border border-red-700 rounded-md text-sm text-red-200">
                  <h3 className="font-semibold mb-1">Connection Rejected</h3>
                  <p>Reason: {rejection.reason}</p>
                  <p className="text-xs mt-1 opacity-70">Code: {rejection.code}</p>
              </div>
          )}
          
          {error && status === 'error' && (
               <div className="mt-4 p-4 bg-red-900/20 border border-red-700 rounded-md text-sm text-red-200">
                   <p>Connection failed. Retrying...</p>
               </div>
          )}
        </div>
      </div>
    </div>
  )
}

export default App
