import { useState } from 'react';
import { Plus, Trash2, Globe, Server, Check } from 'lucide-react';
import { Target } from '@/hooks/use-targets';

interface TargetSelectorProps {
  targets: Target[];
  activeTargetId: string;
  onSelect: (id: string) => void;
  onAdd: (name: string, url: string) => void;
  onDelete: (id: string) => void;
}

export function TargetSelector({ targets, activeTargetId, onSelect, onAdd, onDelete }: TargetSelectorProps) {
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
      <div className="flex justify-between items-center border-b border-gray-700 pb-2">
        <span className="text-gray-400 font-medium">Target</span>
        {!isAdding && (
          <button 
            onClick={() => setIsAdding(true)}
            className="text-xs flex items-center gap-1 bg-gray-700 hover:bg-gray-600 text-gray-200 px-2 py-1 rounded transition-colors"
          >
            <Plus size={14} /> Add
          </button>
        )}
      </div>

      {isAdding ? (
        <form onSubmit={handleSubmit} className="bg-gray-900/50 p-3 rounded-md border border-gray-700 space-y-3">
          <div>
            <label className="block text-xs text-gray-500 mb-1">Name</label>
            <input
              type="text"
              value={newName}
              onChange={(e) => setNewName(e.target.value)}
              placeholder="Home Server"
              className="w-full bg-gray-800 border border-gray-700 rounded px-2 py-1 text-sm text-gray-200 focus:outline-none focus:border-blue-500"
              autoFocus
            />
          </div>
          <div>
            <label className="block text-xs text-gray-500 mb-1">WS URL</label>
            <input
              type="text"
              value={newUrl}
              onChange={(e) => setNewUrl(e.target.value)}
              placeholder="wss://..."
              className="w-full bg-gray-800 border border-gray-700 rounded px-2 py-1 text-sm text-gray-200 focus:outline-none focus:border-blue-500"
            />
          </div>
          <div className="flex justify-end gap-2 pt-1">
            <button
              type="button"
              onClick={handleCancel}
              className="text-xs px-2 py-1 text-gray-400 hover:text-gray-200"
            >
              Cancel
            </button>
            <button
              type="submit"
              disabled={!newName || !newUrl}
              className="text-xs bg-blue-600 hover:bg-blue-500 disabled:opacity-50 disabled:cursor-not-allowed text-white px-3 py-1 rounded flex items-center gap-1"
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
                group flex items-center justify-between p-2 rounded-md border cursor-pointer transition-all
                ${target.id === activeTargetId 
                  ? 'bg-blue-900/20 border-blue-500/50 ring-1 ring-blue-500/20' 
                  : 'bg-gray-800 border-gray-700 hover:border-gray-600'
                }
              `}
              onClick={() => onSelect(target.id)}
            >
              <div className="flex items-center gap-3 overflow-hidden">
                <div className={`p-1.5 rounded-full ${target.id === activeTargetId ? 'bg-blue-500/20 text-blue-400' : 'bg-gray-700 text-gray-400'}`}>
                  {target.type === 'local' ? <Server size={14} /> : <Globe size={14} />}
                </div>
                <div className="flex flex-col overflow-hidden">
                  <span className={`text-sm font-medium truncate ${target.id === activeTargetId ? 'text-blue-100' : 'text-gray-300'}`}>
                    {target.name}
                  </span>
                  <span className="text-xs text-gray-500 truncate" title={target.url}>
                    {target.url}
                  </span>
                </div>
              </div>

              {target.type === 'custom' && (
                <button
                  onClick={(e) => {
                    e.stopPropagation();
                    onDelete(target.id);
                  }}
                  className="p-1.5 text-gray-500 hover:text-red-400 hover:bg-red-900/20 rounded opacity-0 group-hover:opacity-100 transition-all"
                  title="Remove target"
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
