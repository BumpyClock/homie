import { useState } from 'react';
import { Plus, Trash2, Globe, Server, Check, Settings } from 'lucide-react';
import type { Target } from '@/hooks/use-targets';
import type { ConnectionStatus } from '@/hooks/use-gateway';
import type { HelloReject, ServerHello } from "@homie/shared";

interface TargetSelectorProps {
  targets: Target[];
  activeTargetId: string;
  onSelect: (id: string) => void;
  onAdd: (name: string, url: string) => void;
  onDelete: (id: string) => void;
  onDetails?: (target: Target) => void;
  hideLocal: boolean;
  onRestoreLocal: () => void;
  connectionStatus?: ConnectionStatus;
  serverHello?: ServerHello | null;
  rejection?: HelloReject | null;
  error?: Event | null;
}

export function TargetSelector({
  targets,
  activeTargetId,
  onSelect,
  onAdd,
  onDelete,
  onDetails,
  hideLocal,
  onRestoreLocal,
  connectionStatus,
  serverHello,
  rejection,
  error,
}: TargetSelectorProps) {
  const [isAdding, setIsAdding] = useState(false);
  const [newName, setNewName] = useState('');
  const [newUrl, setNewUrl] = useState('');

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    if (newName && newUrl) {
      onAdd(newName, newUrl);
      setIsAdding(false);
      setNewName('');
      setNewUrl('');
    }
  };

  const handleCancel = () => {
    setIsAdding(false);
    setNewName('');
    setNewUrl('');
  };

  const status = connectionStatus ?? "disconnected";
  const statusLabel =
    status === "connected"
      ? "Connected"
      : status === "connecting"
        ? "Connecting"
        : status === "handshaking"
          ? "Handshaking"
          : status === "rejected"
            ? "Rejected"
            : status === "error"
              ? "Error (retrying)"
              : "Disconnected";

  const dotColor =
    status === "connected"
      ? "bg-green-500"
      : status === "connecting" || status === "handshaking"
        ? "bg-yellow-500"
        : "bg-red-500";

  const dotPulse = status === "connecting" || status === "handshaking";

  return (
    <div className="space-y-4">
      {!isAdding && (
        <button
          type="button"
          onClick={() => setIsAdding(true)}
          className="w-full flex items-center justify-center gap-2 min-h-[44px] bg-muted hover:bg-muted/80 text-foreground px-3 py-2 rounded-md border border-border transition-colors"
        >
          <Plus size={16} /> Add
        </button>
      )}

      {isAdding && (
        <div className="space-y-3">
          {hideLocal && (
            <button
              type="button"
              onClick={() => {
                onRestoreLocal();
                handleCancel();
              }}
              className="w-full flex items-center justify-center gap-2 min-h-[44px] bg-card border border-border hover:bg-muted/40 text-foreground px-3 py-2 rounded-md transition-colors"
            >
              <Server size={16} /> Restore local gateway
            </button>
          )}

          <form onSubmit={handleSubmit} className="bg-muted/50 p-3 rounded-md border border-border space-y-3">
            <div>
              <label className="block text-xs text-muted-foreground mb-1">Name</label>
              <input
                type="text"
                value={newName}
                onChange={(e) => setNewName(e.target.value)}
                placeholder="Home Server"
                className="w-full bg-background border border-border rounded px-2 py-1 text-sm text-foreground focus:outline-none focus:border-primary"
                autoFocus
              />
            </div>
            <div>
              <label className="block text-xs text-muted-foreground mb-1">WS URL</label>
              <input
                type="text"
                value={newUrl}
                onChange={(e) => setNewUrl(e.target.value)}
                placeholder="wss://..."
                className="w-full bg-background border border-border rounded px-2 py-1 text-sm text-foreground focus:outline-none focus:border-primary"
              />
            </div>
            <div className="flex justify-end gap-2 pt-1">
              <button
                type="button"
                onClick={handleCancel}
                className="text-xs px-2 py-1 text-muted-foreground hover:text-foreground"
              >
                Cancel
              </button>
              <button
                type="submit"
                disabled={!newName || !newUrl}
                className="text-xs bg-primary hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed text-primary-foreground px-3 py-1 rounded flex items-center gap-1"
              >
                <Check size={14} /> Save
              </button>
            </div>
          </form>
        </div>
      )}

      <div className="text-xs uppercase tracking-wide text-muted-foreground">Targets</div>

      <div className="space-y-2">
        {targets.map((target) => {
          const isActive = target.id === activeTargetId;

          return (
            <div
              key={target.id}
              className={
                `group flex items-center justify-between p-2 rounded-md border cursor-pointer transition-colors ${
                  isActive
                    ? 'bg-primary/10 border-primary/50 ring-1 ring-primary/20'
                    : 'bg-card border-border hover:border-muted-foreground'
                }`
              }
              onClick={() => onSelect(target.id)}
            >
              <div className="flex items-center gap-3 overflow-hidden">
                <div
                  className={`p-1.5 rounded-full ${
                    isActive ? 'bg-primary/20 text-primary' : 'bg-muted text-muted-foreground'
                  }`}
                >
                  {target.type === 'local' ? <Server size={14} /> : <Globe size={14} />}
                </div>

                <div className="flex flex-col overflow-hidden">
                  <span
                    className={`text-sm font-medium truncate ${
                      isActive ? 'text-primary' : 'text-card-foreground'
                    }`}
                  >
                    {target.name}
                  </span>
                  <span className="text-xs text-muted-foreground truncate" title={target.url}>
                    {target.url}
                  </span>

                  {isActive && (
                    <div className="mt-1">
                      <div className="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
                        <span
                          className={`inline-block rounded-full ${dotColor} ${
                            dotPulse ? 'animate-pulse motion-reduce:animate-none' : ''
                          } h-2.5 w-2.5`}
                          role="img"
                          aria-label={`Connection status: ${status}`}
                        />
                        <span>{statusLabel}</span>
                        {status === 'connected' && serverHello && (
                          <>
                            <span className="text-xs text-muted-foreground">•</span>
                            <span
                              className="font-mono text-[11px] text-foreground/80 truncate max-w-[160px]"
                              title={serverHello.server_id}
                            >
                              {serverHello.server_id}
                            </span>
                            <span className="text-xs text-muted-foreground">• v{serverHello.protocol_version}</span>
                          </>
                        )}
                      </div>

                      {status === 'rejected' && rejection && (
                        <div className="mt-1 text-xs text-muted-foreground">
                          {rejection.reason}{' '}
                          <span className="font-mono opacity-70">({rejection.code})</span>
                        </div>
                      )}
                      {status === 'error' && error && (
                        <div className="mt-1 text-xs text-muted-foreground">Connection failed. Retrying…</div>
                      )}
                    </div>
                  )}
                </div>
              </div>

              <div className="flex items-center gap-1 opacity-100 sm:opacity-0 group-hover:opacity-100 group-focus-within:opacity-100 transition-opacity">
                {onDetails && (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onDetails(target);
                    }}
                    className="p-1.5 text-muted-foreground hover:text-foreground hover:bg-muted/60 rounded"
                    title="Gateway details"
                    aria-label="Gateway details"
                  >
                    <Settings size={14} />
                  </button>
                )}

                {(target.type === 'custom' || !hideLocal) && (
                  <button
                    onClick={(e) => {
                      e.stopPropagation();
                      onDelete(target.id);
                    }}
                    className="p-1.5 text-muted-foreground hover:text-destructive hover:bg-destructive/20 rounded"
                    title="Remove target"
                    aria-label="Remove target"
                  >
                    <Trash2 size={14} />
                  </button>
                )}
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
