import { useEffect, useMemo, useState } from "react";
import { Plus, Pencil, Trash2, Square } from "lucide-react";
import { ChatItemView } from "@/components/chat-item";
import { useChat } from "@/hooks/use-chat";
import type { ConnectionStatus } from "@/hooks/use-gateway";

interface ChatPanelProps {
  status: ConnectionStatus;
  call: (method: string, params?: unknown) => Promise<unknown>;
  onEvent: (callback: (event: { topic: string; params?: unknown }) => void) => () => void;
  enabled: boolean;
  namespace: string;
}

export function ChatPanel({ status, call, onEvent, enabled, namespace }: ChatPanelProps) {
  const {
    threads,
    activeChatId,
    activeThread,
    error,
    clearError,
    selectChat,
    createChat,
    sendMessage,
    cancelActive,
    archiveChat,
    renameChat,
    respondApproval,
    accountStatus,
    formatRelativeTime,
  } = useChat({ status, call, onEvent, enabled, namespace });

  const [draft, setDraft] = useState("");
  const [isEditingTitle, setIsEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");

  const activeTitle = activeThread?.title ?? "";
  const canSend = status === "connected" && !!activeThread;

  useEffect(() => {
    setIsEditingTitle(false);
    setTitleDraft(activeTitle);
  }, [activeThread?.chatId, activeTitle]);

  const listState = useMemo(() => {
    if (!enabled) return { message: "Chat service not enabled for this gateway." };
    if (status !== "connected") return { message: "Connect to a gateway to view chat history." };
    if (threads.length === 0) return { message: "No chats yet. Start a new chat." };
    return { message: "" };
  }, [enabled, status, threads.length]);

  const handleSend = async () => {
    if (!draft.trim()) return;
    await sendMessage(draft);
    setDraft("");
  };

  return (
    <div className="h-full min-h-0 flex border border-border rounded-lg overflow-hidden bg-card/20">
      <aside className="w-[320px] max-w-[40%] border-r border-border bg-card/40 flex flex-col min-h-0">
        <div className="p-4 border-b border-border flex items-center justify-between gap-2">
          <div>
            <div className="text-sm font-semibold">Chats</div>
            <div className="text-xs text-muted-foreground">Gateway history</div>
          </div>
          <button
            type="button"
            onClick={createChat}
            disabled={status !== "connected"}
            className="inline-flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors disabled:opacity-50"
          >
            <Plus className="w-4 h-4" />
            New
          </button>
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto">
          {listState.message && (
            <div className="p-6 text-sm text-muted-foreground">{listState.message}</div>
          )}
          {!listState.message && (
            <div className="divide-y divide-border">
              {threads.map((thread) => {
                const isActive = thread.chatId === activeChatId;
                return (
                  <button
                    key={thread.chatId}
                    type="button"
                    onClick={() => selectChat(thread.chatId)}
                    className={`w-full text-left p-4 transition-colors motion-reduce:transition-none ${
                      isActive ? "bg-muted/50" : "hover:bg-muted/30"
                    }`}
                  >
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <div className="flex items-center gap-2">
                          <span
                            className={`h-2 w-2 rounded-full ${
                              thread.running ? "bg-green-500" : "bg-muted-foreground/40"
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
                      <div className="text-[11px] text-muted-foreground whitespace-nowrap">
                        {formatRelativeTime(thread.lastActivityAt)}
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </div>
      </aside>

      <section className="flex-1 min-h-0 flex flex-col">
        <div className="border-b border-border p-4 flex items-start justify-between gap-3">
          <div className="min-w-0 flex-1">
            {activeThread ? (
              isEditingTitle ? (
                <div className="flex items-center gap-2">
                  <input
                    type="text"
                    value={titleDraft}
                    onChange={(e) => setTitleDraft(e.target.value)}
                    onKeyDown={(e) => {
                      if (e.key === "Enter") {
                        e.preventDefault();
                        renameChat(activeThread.chatId, titleDraft);
                        setIsEditingTitle(false);
                      }
                      if (e.key === "Escape") {
                        setIsEditingTitle(false);
                      }
                    }}
                    className="flex-1 min-w-0 bg-background border border-border rounded px-2 py-2 text-sm"
                    aria-label="Chat title"
                    autoFocus
                  />
                  <button
                    type="button"
                    onClick={() => {
                      renameChat(activeThread.chatId, titleDraft);
                      setIsEditingTitle(false);
                    }}
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
                    onClick={() => {
                      setTitleDraft(activeTitle);
                      setIsEditingTitle(true);
                    }}
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
                  onClick={cancelActive}
                  className="inline-flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md border border-border text-muted-foreground hover:text-foreground hover:bg-muted/50 transition-colors"
                >
                  <Square className="w-4 h-4" />
                  Stop
                </button>
              )}
              <button
                type="button"
                onClick={() => archiveChat(activeThread.chatId)}
                className="inline-flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md border border-border text-destructive hover:bg-destructive/10 transition-colors"
              >
                <Trash2 className="w-4 h-4" />
                Archive
              </button>
            </div>
          )}
        </div>

        <div className="flex-1 min-h-0 overflow-y-auto px-6 py-6 space-y-4">
          {!accountStatus.ok && (
            <div className="rounded-lg border border-amber-400/60 bg-amber-50/70 dark:bg-amber-500/10 p-3 text-sm text-amber-800 dark:text-amber-200">
              {accountStatus.message}
            </div>
          )}

          {error && (
            <div className="rounded-lg border border-destructive/30 bg-destructive/10 p-3 text-sm text-destructive flex items-center justify-between gap-3">
              <span>{error}</span>
              <button
                type="button"
                onClick={clearError}
                className="px-2 py-1 text-xs rounded border border-destructive/40 hover:bg-destructive/10"
              >
                Dismiss
              </button>
            </div>
          )}

          {activeThread ? (
            activeThread.items.length > 0 ? (
              activeThread.items.map((item) => (
                <ChatItemView key={item.id} item={item} onApprove={respondApproval} />
              ))
            ) : (
              <div className="text-sm text-muted-foreground">
                No messages yet. Start the conversation below.
              </div>
            )
          ) : (
            <div className="text-sm text-muted-foreground">
              Select a chat from the left to view its history.
            </div>
          )}
        </div>

        <div className="border-t border-border p-4 bg-card/60">
          <form
            className="flex gap-2 items-end"
            onSubmit={(e) => {
              e.preventDefault();
              void handleSend();
            }}
          >
            <textarea
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              onKeyDown={(e) => {
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  void handleSend();
                }
              }}
              disabled={!canSend}
              placeholder={canSend ? "Send a message…" : "Connect to a gateway to chat."}
              className="flex-1 min-h-[56px] max-h-[180px] resize-y rounded-md border border-border bg-background px-3 py-2 text-sm focus:outline-none focus:border-primary disabled:opacity-60"
            />
            <button
              type="submit"
              disabled={!canSend || !draft.trim()}
              className="px-4 py-2 min-h-[44px] rounded-md bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors disabled:opacity-50"
            >
              Send
            </button>
          </form>
        </div>
      </section>
    </div>
  );
}
