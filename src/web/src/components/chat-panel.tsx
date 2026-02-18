import { useEffect, useMemo, useRef, useState } from "react";
import { ArrowDown, ArrowUp, Folder, MessageCircleDashed, Square } from "lucide-react";
import { AUTH_COPY, shouldShowStop, groupTurns } from "@homie/shared";
import { ChatTurns } from "@/components/chat-turns";
import { ChatComposerBar } from "@/components/chat-composer-bar";
import { ChatInlineMenu } from "@/components/chat-inline-menu";
import { ChatThreadList } from "@/components/chat-thread-list";
import { ChatThreadHeader } from "@/components/chat-thread-header";
import { ChatComposerInput } from "@/components/chat-composer-input";
import { AuthRedirectBanner } from "@/components/AuthRedirectBanner";
import { useChat } from "@/hooks/use-chat";
import type { ConnectionStatus } from "@/hooks/use-gateway";
import type { FileOption, SkillOption } from "@/lib/chat-utils";
import { getTextareaCaretPosition } from "@/lib/caret";

interface ChatPanelProps {
  status: ConnectionStatus;
  call: (method: string, params?: unknown) => Promise<unknown>;
  onEvent: (callback: (event: { topic: string; params?: unknown }) => void) => () => void;
  enabled: boolean;
  namespace: string;
  /** Callback to open the settings panel to a specific section */
  onOpenSettings?: (section?: "connection" | "providers" | "preferences" | "about") => void;
}

export function ChatPanel({ status, call, onEvent, enabled, namespace, onOpenSettings }: ChatPanelProps) {
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
    models,
    collaborationModes,
    skills,
    enabledWebTools,
    webToolsAvailable,
    activeSettings,
    updateSettings,
    updateAttachments,
    activeTokenUsage,
    searchFiles,
    formatRelativeTime,
    queuedNotice,
  } = useChat({ status, call, onEvent, enabled, namespace });

  const [draft, setDraft] = useState("");
  const [sending, setSending] = useState(false);
  const [showThreadList, setShowThreadList] = useState(() => {
    if (typeof window === "undefined") return true;
    return window.innerWidth >= 640;
  });
  const [isEditingTitle, setIsEditingTitle] = useState(false);
  const [titleDraft, setTitleDraft] = useState("");
  const [attachOpen, setAttachOpen] = useState(false);
  const [attachDraft, setAttachDraft] = useState("");
  const [trigger, setTrigger] = useState<{
    type: "slash" | "mention";
    start: number;
    cursor: number;
    query: string;
  } | null>(null);
  const [menuIndex, setMenuIndex] = useState(0);
  const [fileOptions, setFileOptions] = useState<FileOption[]>([]);
  const [menuVisible, setMenuVisible] = useState(false);
  const [menuPosition, setMenuPosition] = useState({ x: 0, y: 0 });
  const inputRef = useRef<HTMLTextAreaElement | null>(null);
  const scrollRef = useRef<HTMLDivElement | null>(null);
  const endRef = useRef<HTMLDivElement | null>(null);
  const stickToBottomRef = useRef(true);
  const mentionSearchRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const mentionQueryRef = useRef("");
  const [visibleTurnCount, setVisibleTurnCount] = useState(60);
  const [stickToBottom, setStickToBottom] = useState(true);
  const emptySuggestions = useMemo(
    () => [
      "Explain this repository structure.",
      "Summarize recent chat and terminal activity.",
      "Find likely bugs in the current branch.",
      "Plan the next implementation steps.",
    ],
    [],
  );

  const activeTitle = activeThread?.title ?? "";
  const isMobileViewport = typeof window !== "undefined" ? window.innerWidth < 640 : false;
  const canSend = status === "connected" && !!activeThread;
  const canEditSettings = status === "connected" && !!activeThread;
  const showStop = shouldShowStop(!!activeThread?.running, sending);
  const attachedFolder = activeSettings.attachedFolder;

  const skillOptions = useMemo(() => {
    if (!trigger || trigger.type !== "slash") return [];
    const filter = trigger.query.toLowerCase();
    if (!filter) return skills;
    return skills.filter((skill) => skill.name.toLowerCase().includes(filter));
  }, [skills, trigger]);

  const mentionSkillOptions = useMemo(() => {
    if (!trigger || trigger.type !== "mention") return [];
    const filter = trigger.query.toLowerCase();
    if (!filter) return skills;
    return skills.filter((skill) => skill.name.toLowerCase().includes(filter));
  }, [skills, trigger]);

  const mentionOptions = useMemo(() => {
    if (!trigger || trigger.type !== "mention") return [];
    return fileOptions;
  }, [fileOptions, trigger]);

  const activeMenuItems = useMemo(() => {
    if (!trigger) return [];
    if (trigger.type === "slash") return skillOptions.map((item) => ({ type: "skill" as const, item }));
    return [
      ...mentionSkillOptions.map((item) => ({ type: "skill" as const, item })),
      ...mentionOptions.map((item) => ({ type: item.type, item })),
    ];
  }, [mentionOptions, mentionSkillOptions, skillOptions, trigger]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    const onResize = () => {
      if (window.innerWidth >= 640) {
        setShowThreadList(true);
      }
    };
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, []);

  useEffect(() => {
    return () => {
      if (mentionSearchRef.current) {
        clearTimeout(mentionSearchRef.current);
      }
    };
  }, []);

  const listState = useMemo(() => {
    if (!enabled) return { message: "Chat service not enabled for this gateway." };
    if (status !== "connected") return { message: "Connect to a gateway to view chat history." };
    if (threads.length === 0) return { message: "No chats yet. Start a new chat." };
    return { message: "" };
  }, [enabled, status, threads.length]);

  const handleSend = async () => {
    if (!draft.trim()) return;
    stickToBottomRef.current = true;
    setStickToBottom(true);
    setSending(true);
    try {
      await sendMessage(draft);
    } finally {
      setSending(false);
    }
    setDraft("");
    setTrigger(null);
    setMenuVisible(false);
  };

  const scrollToBottom = (behavior: ScrollBehavior = "auto") => {
    endRef.current?.scrollIntoView({ behavior, block: "end" });
  };

  const lastItemSignature = useMemo(() => {
    if (!activeThread || activeThread.items.length === 0) return "";
    const last = activeThread.items[activeThread.items.length - 1];
    return [
      last.id,
      last.kind,
      last.text?.length ?? 0,
      last.output?.length ?? 0,
      last.content?.length ?? 0,
    ].join(":");
  }, [activeThread]);

  useEffect(() => {
    if (!activeThread) return;
    if (!stickToBottomRef.current) return;
    scrollToBottom("auto");
  }, [activeThread, activeThread?.running, lastItemSignature]);

  const updateMenuPosition = (value: string, cursorPosition: number) => {
    if (!inputRef.current) return;
    if (typeof window === "undefined") return;

    const caret = getTextareaCaretPosition(inputRef.current, value, cursorPosition);
    if (caret) {
      setMenuPosition(caret);
      return;
    }

    const rect = inputRef.current.getBoundingClientRect();
    const style = window.getComputedStyle(inputRef.current);
    const lineHeight =
      Number.parseFloat(style.lineHeight) ||
      Number.parseFloat(style.fontSize) * 1.3;
    const paddingLeft = Number.parseFloat(style.paddingLeft) || 0;
    const paddingTop = Number.parseFloat(style.paddingTop) || 0;
    const linesBefore = value.slice(0, cursorPosition).split("\n").length - 1;
    setMenuPosition({
      x: rect.left + paddingLeft,
      y: rect.top + paddingTop + (linesBefore + 1) * lineHeight,
    });
  };

  const updateTrigger = (value: string, cursorPosition: number) => {
    const textBefore = value.slice(0, cursorPosition);
    const slashMatch = textBefore.match(/(?:^|\s)\/([\w-]*)$/);
    const atMatch = textBefore.match(/@([\w./-]*)$/);
    if (atMatch) {
      const start = textBefore.lastIndexOf("@");
      const charBefore = start <= 0 ? " " : textBefore[start - 1];
      const valid = /\s/.test(charBefore) || /[("']/.test(charBefore) || start === 0;
      if (valid) {
        updateMenuPosition(value, cursorPosition);
        const nextTrigger: {
          type: "mention";
          start: number;
          cursor: number;
          query: string;
        } = {
          type: "mention",
          start,
          cursor: cursorPosition,
          query: atMatch[1] ?? "",
        };
        setTrigger(nextTrigger);
        setMenuIndex(0);
        setMenuVisible(true);
        setFileOptions([]);
        if (nextTrigger.query.trim()) {
          mentionQueryRef.current = nextTrigger.query;
          if (mentionSearchRef.current) {
            clearTimeout(mentionSearchRef.current);
          }
          if (attachedFolder && activeThread) {
            mentionSearchRef.current = setTimeout(() => {
              void searchFiles(activeThread.chatId, mentionQueryRef.current, attachedFolder).then((results) =>
                setFileOptions(results),
              );
            }, 150);
          }
        }
        return;
      }
    }
    if (slashMatch) {
      const start = textBefore.lastIndexOf("/");
      updateMenuPosition(value, cursorPosition);
      setTrigger({
        type: "slash",
        start,
        cursor: cursorPosition,
        query: slashMatch[1] ?? "",
      });
      setMenuIndex(0);
      setMenuVisible(true);
      return;
    }
    setTrigger(null);
    setMenuVisible(false);
    setFileOptions([]);
    if (mentionSearchRef.current) {
      clearTimeout(mentionSearchRef.current);
      mentionSearchRef.current = null;
    }
  };

  const insertAtTrigger = (text: string) => {
    if (!trigger) return;
    const before = draft.slice(0, trigger.start);
    const after = draft.slice(trigger.cursor);
    const next = `${before}${text}${after}`;
    setDraft(next);
    setTrigger(null);
    setMenuVisible(false);
    setFileOptions([]);
    if (mentionSearchRef.current) {
      clearTimeout(mentionSearchRef.current);
      mentionSearchRef.current = null;
    }
    requestAnimationFrame(() => {
      if (!inputRef.current) return;
      const pos = before.length + text.length;
      inputRef.current.focus();
      inputRef.current.setSelectionRange(pos, pos);
    });
  };

  const handleSelectChat = (nextChatId: string) => {
    selectChat(nextChatId);
    setIsEditingTitle(false);
    setTitleDraft("");
    setAttachOpen(false);
    setAttachDraft("");
    setVisibleTurnCount(60);
    stickToBottomRef.current = true;
    setStickToBottom(true);
    if (isMobileViewport) {
      setShowThreadList(false);
    }
  };

  const handleCreateChat = () => {
    void createChat();
    setIsEditingTitle(false);
    setTitleDraft("");
    setAttachOpen(false);
    setAttachDraft("");
    setVisibleTurnCount(60);
    stickToBottomRef.current = true;
    setStickToBottom(true);
    if (isMobileViewport) {
      setShowThreadList(false);
    }
  };

  return (
    <div className="relative h-full min-h-0 overflow-hidden bg-transparent sm:flex">
      <ChatThreadList
        threads={threads}
        activeChatId={activeChatId}
        listMessage={listState.message}
        canCreate={status === "connected"}
        formatRelativeTime={formatRelativeTime}
        onCreate={handleCreateChat}
        onSelect={handleSelectChat}
        mobileHidden={isMobileViewport && !showThreadList}
      />

      <section
        className={`absolute inset-0 sm:static sm:flex flex-1 min-h-0 flex-col transition-transform duration-200 ease-out motion-reduce:transition-none will-change-transform ${
          isMobileViewport
            ? showThreadList
              ? "translate-x-full pointer-events-none"
              : "translate-x-0 pointer-events-auto"
            : "translate-x-0 pointer-events-auto"
        }`}
      >
        <ChatThreadHeader
          activeThread={activeThread}
          activeTitle={activeTitle}
          isEditingTitle={isEditingTitle}
          titleDraft={titleDraft}
          onChangeTitle={setTitleDraft}
          onStartEdit={() => {
            setTitleDraft(activeTitle);
            setIsEditingTitle(true);
          }}
          onCancelEdit={() => setIsEditingTitle(false)}
          onSaveTitle={() => {
            if (!activeThread) return;
            renameChat(activeThread.chatId, titleDraft);
            setIsEditingTitle(false);
          }}
          onCancelActive={cancelActive}
          onArchive={() => {
            if (!activeThread) return;
            archiveChat(activeThread.chatId);
          }}
          onBack={() => {
            setShowThreadList(true);
            setIsEditingTitle(false);
          }}
          showBackButton={isMobileViewport && !showThreadList}
        />

        <div
          className="relative flex-1 min-h-0"
          style={{
            maskImage:
              "linear-gradient(to bottom, transparent 0%, black 32px, black calc(100% - 32px), transparent 100%)",
            WebkitMaskImage:
              "linear-gradient(to bottom, transparent 0%, black 32px, black calc(100% - 32px), transparent 100%)",
          }}
        >
          <div
            ref={scrollRef}
            onScroll={() => {
              const viewport = scrollRef.current;
              if (!viewport) return;
              const distance = viewport.scrollHeight - viewport.scrollTop - viewport.clientHeight;
              const nearBottom = distance < 24;
              stickToBottomRef.current = nearBottom;
              setStickToBottom((prev) => (prev === nearBottom ? prev : nearBottom));
              if (viewport.scrollTop < 120 && activeThread) {
                const totalTurns = groupTurns(activeThread.items).length;
                if (visibleTurnCount < totalTurns) {
                  const prevHeight = viewport.scrollHeight;
                  const prevTop = viewport.scrollTop;
                  setVisibleTurnCount((prev) => Math.min(prev + 20, totalTurns));
                  requestAnimationFrame(() => {
                    const nextHeight = viewport.scrollHeight;
                    viewport.scrollTop = nextHeight - prevHeight + prevTop;
                  });
                }
              }
            }}
            className="chat-scroll h-full overflow-y-auto px-4 sm:px-6 py-4 sm:py-6 space-y-4"
          >
          <AuthRedirectBanner
            visible={!accountStatus.ok}
            message={AUTH_COPY.bannerMessage}
            actionLabel={AUTH_COPY.bannerActionWeb}
            onAction={() => onOpenSettings?.("providers")}
          />

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
              <div key={activeThread.chatId} className="homie-fade-in">
                <ChatTurns
                  items={activeThread.items}
                  activeTurnId={activeThread.activeTurnId}
                  running={activeThread.running}
                  onApprove={respondApproval}
                  visibleTurnCount={visibleTurnCount}
                />
              </div>
            ) : (
              <div className="min-h-[280px] flex flex-col items-center justify-center text-center gap-4 px-4">
                <div className="h-12 w-12 rounded-full border border-border bg-card/40 inline-flex items-center justify-center text-muted-foreground">
                  <MessageCircleDashed className="h-6 w-6" />
                </div>
                <div>
                  <div className="text-base font-semibold tracking-tight">What would you like to work on?</div>
                  <div className="text-sm text-muted-foreground mt-1">Start with a prompt or choose a suggestion.</div>
                </div>
                <div className="flex flex-wrap items-center justify-center gap-2">
                  {emptySuggestions.map((suggestion) => (
                    <button
                      key={suggestion}
                      type="button"
                      onClick={() => {
                        setDraft(suggestion);
                        requestAnimationFrame(() => inputRef.current?.focus());
                      }}
                      className="rounded-full border border-border bg-card/40 px-3 py-1.5 text-sm hover:bg-muted/40 transition-colors motion-reduce:transition-none"
                    >
                      {suggestion}
                    </button>
                  ))}
                </div>
              </div>
            )
          ) : (
            <div className="text-sm text-muted-foreground">
              Select a chat from the left to view its history.
            </div>
          )}
          <div ref={endRef} />
          </div>
          {!stickToBottom && activeThread?.running ? (
            <div className="pointer-events-none absolute right-8 bottom-24 sm:bottom-28">
              <button
                type="button"
                onClick={() => {
                  stickToBottomRef.current = true;
                  setStickToBottom(true);
                  scrollToBottom("smooth");
                }}
                className="pointer-events-auto homie-fab-in inline-flex items-center gap-1.5 rounded-full bg-primary/90 text-primary-foreground shadow-md px-3 py-2 text-xs hover:bg-primary transition-colors motion-reduce:transition-none"
                aria-label="Scroll to latest messages"
              >
                <ArrowDown className="h-3.5 w-3.5" />
                New messages
              </button>
            </div>
          ) : null}
        </div>

        <div className="border-t border-border p-4 bg-card/60 relative">
          <div className="mb-3">
            <ChatComposerBar
              models={models}
              collaborationModes={collaborationModes}
              settings={activeSettings}
              enabledWebTools={enabledWebTools}
              webToolsAvailable={webToolsAvailable}
              tokenUsage={activeTokenUsage}
              running={!!activeThread?.running}
              queuedHint={queuedNotice}
              disabled={!canEditSettings}
              onChangeSettings={(updates) => {
                if (!activeThread) return;
                updateSettings(activeThread.chatId, updates);
              }}
            />
          </div>
          {activeThread && (
            <div className="mb-3 flex flex-wrap items-center gap-2">
              {attachedFolder ? (
                <div className="inline-flex items-center gap-2 rounded-full border border-border bg-muted/40 px-3 py-1 text-xs">
                  <Folder className="h-3.5 w-3.5" />
                  <span className="truncate max-w-[260px]">{attachedFolder}</span>
                  <button
                    type="button"
                    onClick={() => updateAttachments(activeThread.chatId, null)}
                    className="text-muted-foreground hover:text-foreground"
                  >
                    ×
                  </button>
                </div>
              ) : (
                <div className="text-xs text-muted-foreground">No folder attached.</div>
              )}
              <button
                type="button"
                onClick={() =>
                  setAttachOpen((prev) => {
                    const next = !prev;
                    if (next) {
                      setAttachDraft(attachedFolder ?? "");
                    }
                    return next;
                  })
                }
                disabled={!canEditSettings}
                className="inline-flex items-center gap-2 px-2 py-1.5 min-h-[32px] rounded-md border border-border text-xs hover:bg-muted/50 disabled:opacity-50"
              >
                <Folder className="h-4 w-4" />
                {attachedFolder ? "Change folder" : "Attach folder"}
              </button>
            </div>
          )}
          {attachOpen && activeThread && (
            <div className="mb-3 flex flex-wrap items-center gap-2 rounded-md border border-border bg-muted/30 p-3">
              <input
                type="text"
                value={attachDraft}
                onChange={(e) => setAttachDraft(e.target.value)}
                placeholder="/path/to/project"
                className="flex-1 min-w-[220px] bg-background border border-border rounded px-2 py-2 text-sm"
              />
              <button
                type="button"
                onClick={() => {
                  if (!attachDraft.trim()) return;
                  void updateAttachments(activeThread.chatId, attachDraft.trim());
                  setAttachOpen(false);
                }}
                className="px-3 py-2 min-h-[36px] rounded-md bg-primary text-primary-foreground text-xs font-medium"
              >
                Attach
              </button>
              <button
                type="button"
                onClick={() => {
                  setAttachOpen(false);
                  setAttachDraft(attachedFolder ?? "");
                }}
                className="px-3 py-2 min-h-[36px] rounded-md border border-border text-xs"
              >
                Cancel
              </button>
            </div>
          )}
          <form
            className="flex flex-col sm:flex-row gap-2 items-end"
            onSubmit={(e) => {
              e.preventDefault();
              void handleSend();
            }}
          >
            <ChatComposerInput
              value={draft}
              inputRef={inputRef}
              disabled={!canSend}
              placeholder={canSend ? "Send a message…" : "Connect to a gateway to chat."}
              action={
                showStop ? (
                  <button
                    type="button"
                    onClick={cancelActive}
                    className="inline-flex items-center justify-center h-8 w-8 rounded-full bg-destructive text-destructive-foreground hover:bg-destructive/90 transition-colors motion-reduce:transition-none"
                    aria-label="Stop generation"
                  >
                    <Square className="h-3 w-3 fill-current" />
                  </button>
                ) : (
                  <button
                    type="submit"
                    disabled={!canSend || !draft.trim()}
                    className="inline-flex items-center justify-center h-8 w-8 rounded-full bg-primary text-primary-foreground hover:bg-primary/90 transition-colors motion-reduce:transition-none disabled:opacity-40"
                    aria-label="Send message"
                  >
                    <ArrowUp className="h-4 w-4" />
                  </button>
                )
              }
              onChange={(value, cursor) => {
                setDraft(value);
                updateTrigger(value, cursor);
              }}
              onClick={(e) => {
                const value = e.currentTarget.value;
                const cursor = e.currentTarget.selectionStart ?? value.length;
                updateTrigger(value, cursor);
              }}
              onKeyDown={(e) => {
                if (trigger && activeMenuItems.length > 0) {
                  if (e.key === "ArrowDown") {
                    e.preventDefault();
                    setMenuIndex((prev) => (prev + 1) % activeMenuItems.length);
                    return;
                  }
                  if (e.key === "ArrowUp") {
                    e.preventDefault();
                    setMenuIndex((prev) =>
                      prev === 0 ? activeMenuItems.length - 1 : prev - 1,
                    );
                    return;
                  }
                  if (e.key === "Enter" || e.key === "Tab") {
                    e.preventDefault();
                    const selected = activeMenuItems[menuIndex];
                    if (selected?.type === "skill") {
                      insertAtTrigger(`$${(selected.item as SkillOption).name} `);
                    } else if (selected?.type === "file") {
                      const file = selected.item as FileOption;
                      insertAtTrigger(`[file:${file.relativePath || file.name}] `);
                    } else if (selected?.type === "directory") {
                      const file = selected.item as FileOption;
                      insertAtTrigger(`[folder:${file.relativePath || file.name}] `);
                    }
                    return;
                  }
                  if (e.key === "Escape") {
                    e.preventDefault();
                    setTrigger(null);
                    setMenuVisible(false);
                    setFileOptions([]);
                    if (mentionSearchRef.current) {
                      clearTimeout(mentionSearchRef.current);
                      mentionSearchRef.current = null;
                    }
                    return;
                  }
                }
                if (e.key === "Enter" && !e.shiftKey) {
                  e.preventDefault();
                  void handleSend();
                }
              }}
              onKeyUp={(e) => {
                if (!["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(e.key)) {
                  return;
                }
                const value = e.currentTarget.value;
                const cursor = e.currentTarget.selectionStart ?? value.length;
                updateTrigger(value, cursor);
              }}
            />
            {showStop ? (
              <button
                type="button"
                onClick={cancelActive}
                className="w-full sm:hidden inline-flex items-center justify-center gap-2 px-4 py-2 min-h-[44px] rounded-full bg-destructive text-destructive-foreground text-sm font-medium hover:bg-destructive/90 transition-colors motion-reduce:transition-none"
              >
                <Square className="h-3.5 w-3.5 fill-current" />
                Stop
              </button>
            ) : (
              <button
                type="submit"
                disabled={!canSend || !draft.trim()}
                className="w-full sm:hidden inline-flex items-center justify-center gap-2 px-4 py-2 min-h-[44px] rounded-full bg-primary text-primary-foreground text-sm font-medium hover:bg-primary/90 transition-colors motion-reduce:transition-none disabled:opacity-50"
              >
                <ArrowUp className="h-4 w-4" />
                Send
              </button>
            )}
          </form>
          <div className="hidden sm:block mt-2 text-[11px] text-muted-foreground/80">
            Enter to send · Shift+Enter for newline
          </div>
          <ChatInlineMenu
            trigger={trigger}
            visible={menuVisible}
            menuIndex={menuIndex}
            position={menuPosition}
            skillOptions={skillOptions}
            mentionSkillOptions={mentionSkillOptions}
            mentionOptions={mentionOptions}
            attachedFolder={attachedFolder}
            onSelectSkill={(skill) => insertAtTrigger(`$${skill.name} `)}
            onSelectFile={(file) => insertAtTrigger(`[file:${file.relativePath || file.name}] `)}
            onSelectFolder={(file) => insertAtTrigger(`[folder:${file.relativePath || file.name}] `)}
            onHoverIndex={setMenuIndex}
          />
        </div>
      </section>
    </div>
  );
}
