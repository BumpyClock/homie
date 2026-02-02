import { Pencil, Square, Trash2 } from "lucide-react";
import type { ActiveChatThread } from "@/lib/chat-utils";

interface ChatThreadHeaderProps {
  activeThread: ActiveChatThread | null;
  activeTitle: string;
  isEditingTitle: boolean;
  titleDraft: string;
  onChangeTitle: (value: string) => void;
  onStartEdit: () => void;
  onCancelEdit: () => void;
  onSaveTitle: () => void;
  onCancelActive: () => void;
  onArchive: () => void;
}

export function ChatThreadHeader({
  activeThread,
  activeTitle,
  isEditingTitle,
  titleDraft,
  onChangeTitle,
  onStartEdit,
  onCancelEdit,
  onSaveTitle,
  onCancelActive,
  onArchive,
}: ChatThreadHeaderProps) {
  return (
    <div className="border-b border-border p-4 flex items-start justify-between gap-3">
      <div className="min-w-0 flex-1">
        {activeThread ? (
          isEditingTitle ? (
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={titleDraft}
                onChange={(e) => onChangeTitle(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") {
                    e.preventDefault();
                    onSaveTitle();
                  }
                  if (e.key === "Escape") {
                    onCancelEdit();
                  }
                }}
                className="flex-1 min-w-0 bg-background border border-border rounded px-2 py-2 text-sm"
                aria-label="Chat title"
                autoFocus
              />
              <button
                type="button"
                onClick={onSaveTitle}
                className="px-3 py-2 min-h-[44px] rounded-md bg-primary text-primary-foreground text-sm font-medium"
              >
                Save
              </button>
            </div>
          ) : (
            <div className="flex items-center gap-2">
              <div className="text-base font-semibold truncate">{activeTitle}</div>
              <button
                type="button"
                onClick={onStartEdit}
                className="p-2 rounded-md hover:bg-muted/50 text-muted-foreground hover:text-foreground transition-colors"
                aria-label="Rename chat"
              >
                <Pencil className="w-4 h-4" />
              </button>
            </div>
          )
        ) : (
          <div className="text-base font-semibold">Select a chat</div>
        )}
        {activeThread && (
          <div className="text-xs text-muted-foreground mt-1">
            {activeThread.running ? "Active turn running" : "Idle"}
          </div>
        )}
      </div>
      {activeThread && (
        <div className="flex items-center gap-2">
          {activeThread.running && (
            <button
              type="button"
              onClick={onCancelActive}
              className="inline-flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md border border-border text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
            >
              <Square className="w-4 h-4" />
              Stop
            </button>
          )}
          <button
            type="button"
            onClick={onArchive}
            className="inline-flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md border border-border text-destructive hover:bg-destructive/10 transition-colors"
          >
            <Trash2 className="w-4 h-4" />
            Archive
          </button>
        </div>
      )}
    </div>
  );
}
