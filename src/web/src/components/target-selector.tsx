import { useState } from 'react';
import { Plus, Trash2, Globe, Server, Check } from 'lucide-react';
import type { Target } from '@/hooks/use-targets';

interface TargetSelectorProps {
  targets: Target[];
  activeTargetId: string;
  onSelect: (id: string) => void;
  onAdd: (name: string, url: string) => void;
  onDelete: (id: string) => void;
  hideLocal: boolean;
  onRestoreLocal: () => void;
}

export function TargetSelector({
  targets,
  activeTargetId,
  onSelect,
  onAdd,
  onDelete,
  hideLocal,
  onRestoreLocal
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

  return (
    <div className="space-y-4">
      <div className="flex justify-between items-center border-b border-border pb-2">
        <span className="text-muted-foreground font-medium">Target</span>
        <div className="flex items-center gap-2">
          {hideLocal && (
            <button
              onClick={onRestoreLocal}
              className="text-xs flex items-center gap-1 bg-muted hover:bg-muted/80 text-foreground px-2 py-1 rounded transition-colors"
            >
              <Server size={14} /> Restore local
            </button>
          )}
          {!isAdding && (
            <button 
              onClick={() => setIsAdding(true)}
              className="text-xs flex items-center gap-1 bg-muted hover:bg-muted/80 text-foreground px-2 py-1 rounded transition-colors"
            >
              <Plus size={14} /> Add
            </button>
          )}
        </div>
      </div>

      {isAdding ? (
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
      ) : (
        <div className="space-y-2">
          {targets.map(target => (
            <div 
              key={target.id}
              className={`
                group flex items-center justify-between p-2 rounded-md border cursor-pointer transition-colors
                ${target.id === activeTargetId 
                  ? 'bg-primary/10 border-primary/50 ring-1 ring-primary/20' 
                  : 'bg-card border-border hover:border-muted-foreground'
                }
              `}
              onClick={() => onSelect(target.id)}
            >
              <div className="flex items-center gap-3 overflow-hidden">
                <div className={`p-1.5 rounded-full ${target.id === activeTargetId ? 'bg-primary/20 text-primary' : 'bg-muted text-muted-foreground'}`}>
                  {target.type === 'local' ? <Server size={14} /> : <Globe size={14} />}
                </div>
                <div className="flex flex-col overflow-hidden">
                  <span className={`text-sm font-medium truncate ${target.id === activeTargetId ? 'text-primary' : 'text-card-foreground'}`}>
                    {target.name}
                  </span>
                  <span className="text-xs text-muted-foreground truncate" title={target.url}>
                    {target.url}
                  </span>
                </div>
              </div>

              {(target.type === 'custom' || !hideLocal) && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(target.id);
                  }}
                  className="p-1.5 text-muted-foreground hover:text-destructive hover:bg-destructive/20 rounded opacity-0 group-hover:opacity-100 transition-opacity"
                  title="Remove target"
                  aria-label="Remove target"
                >
                  <Trash2 size={14} />
                </button>
              )}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}
