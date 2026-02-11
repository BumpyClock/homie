import { Plus } from "lucide-react";
import type { ChatThreadSummary } from "@/lib/chat-utils";

interface ChatThreadListProps {
  threads: ChatThreadSummary[];
  activeChatId: string | null;
  listMessage: string;
  canCreate: boolean;
  formatRelativeTime: (value?: number) => string;
  onCreate: () => void;
  onSelect: (chatId: string) => void;
  mobileHidden?: boolean;
}

export function ChatThreadList({
  threads,
  activeChatId,
  listMessage,
  canCreate,
  formatRelativeTime,
  onCreate,
  onSelect,
  mobileHidden,
}: ChatThreadListProps) {
  return (
    <aside
      className={`absolute inset-0 sm:static w-full sm:w-[320px] sm:max-w-[40%] border-r border-border bg-card/40 flex flex-col min-h-0 transition-transform duration-200 ease-out motion-reduce:transition-none will-change-transform ${
        mobileHidden
          ? "-translate-x-full pointer-events-none sm:translate-x-0 sm:pointer-events-auto"
          : "translate-x-0 pointer-events-auto"
      }`}
    >
      <div className="p-4 border-b border-border flex items-center justify-between gap-2">
        <div>
          <div className="text-sm font-semibold">Chats</div>
          <div className="text-xs text-muted-foreground">Gateway history</div>
        </div>
        <button
          type="button"
          onClick={onCreate}
          disabled={!canCreate}
          className="inline-flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors disabled:opacity-50"
        >
          <Plus className="w-4 h-4" />
          New
        </button>
      </div>

      <div className="chat-scroll flex-1 min-h-0 overflow-y-auto">
        {listMessage && <div className="p-6 text-sm text-muted-foreground">{listMessage}</div>}
        {!listMessage && (
          <div className="p-1.5 space-y-1">
            {threads.map((thread) => {
              const isActive = thread.chatId === activeChatId;
              return (
                <div key={thread.chatId} className="relative">
                  {isActive ? (
                    <span
                      aria-hidden="true"
                      className="absolute left-0 top-2 bottom-2 w-[3px] rounded-full bg-primary"
                    />
                  ) : null}
                  <button
                    type="button"
                    onClick={() => onSelect(thread.chatId)}
                    className={`w-full rounded-md text-left p-4 transition-colors motion-reduce:transition-none ${
                      isActive ? "bg-primary/5 pl-5" : "hover:bg-muted/30"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <span
                            className={`h-2 w-2 rounded-full ${
                              thread.running
                                ? "bg-green-500 animate-pulse motion-reduce:animate-none"
                                : "bg-muted-foreground/40"
                            }`}
                            aria-label={thread.running ? "Active" : "Idle"}
                          />
                          <div className="text-sm font-semibold truncate">{thread.title}</div>
                        </div>
                        <div
                          className="text-xs text-muted-foreground mt-1"
                          style={{
                            display: "-webkit-box",
                            WebkitLineClamp: 2,
                            WebkitBoxOrient: "vertical",
                            overflow: "hidden",
                          }}
                        >
                          {thread.preview || "No messages yet."}
                        </div>
                      </div>
                      <div className="text-[11px] text-muted-foreground whitespace-nowrap tabular-nums">
                        {formatRelativeTime(thread.lastActivityAt)}
                      </div>
                    </div>
                  </button>
                </div>
              );
            })}
          </div>
        )}
      </div>
    </aside>
  );
}
