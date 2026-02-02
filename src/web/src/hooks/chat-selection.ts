import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import type { ChatThreadSummary, ActiveChatThread } from "@/lib/chat-utils";
import { deriveTitleFromThread, extractLastMessage, itemsFromThread, truncateText } from "@/lib/chat-utils";

type CallFn = (method: string, params?: unknown) => Promise<unknown>;

interface SelectChatArgs {
  chatId: string;
  threads: ChatThreadSummary[];
  call: CallFn;
  overrides: Record<string, string>;
  runningTurnsRef: MutableRefObject<Map<string, string>>;
  applySettings?: (chatId: string, settings: unknown) => void;
  setActiveChatId: Dispatch<SetStateAction<string | null>>;
  setActiveThread: Dispatch<SetStateAction<ActiveChatThread | null>>;
  setThreads: Dispatch<SetStateAction<ChatThreadSummary[]>>;
  setError: Dispatch<SetStateAction<string | null>>;
}

export async function selectChat({
  chatId,
  threads,
  call,
  overrides,
  runningTurnsRef,
  applySettings,
  setActiveChatId,
  setActiveThread,
  setThreads,
  setError,
}: SelectChatArgs) {
  const thread = threads.find((t) => t.chatId === chatId);
  if (!thread) return;
  setActiveChatId(chatId);
  try {
    await call("chat.resume", { chat_id: chatId, thread_id: thread.threadId });
  } catch {
    // ignore resume errors; allow read attempt
  }
  try {
    const res = await call("chat.thread.read", {
      chat_id: chatId,
      thread_id: thread.threadId,
      include_turns: true,
    });
    const settings = (res as Record<string, unknown>)?.settings;
    if (settings) {
      applySettings?.(chatId, settings);
    }
    const threadRes = (res as Record<string, unknown>)?.thread ?? res;
    if (!threadRes || typeof threadRes !== "object") {
      setActiveThread({
        chatId,
        threadId: thread.threadId,
        title: thread.title,
        items: [],
        running: thread.running,
      });
      return;
    }
    const items = itemsFromThread(threadRes as Record<string, unknown>);
    const preview = extractLastMessage(items);
    const updatedAt =
      typeof (threadRes as Record<string, unknown>).updated_at === "number"
        ? ((threadRes as Record<string, unknown>).updated_at as number) * 1000
        : undefined;
    const title = overrides[chatId] ?? deriveTitleFromThread(threadRes as Record<string, unknown>, thread.title);
    setThreads((prev) =>
      prev.map((t) =>
        t.chatId === chatId
          ? { ...t, title, preview: preview ? truncateText(preview, 96) : t.preview, lastActivityAt: updatedAt ?? t.lastActivityAt }
          : t,
      ),
    );
    setActiveThread({
      chatId,
      threadId: thread.threadId,
      title,
      items,
      running: thread.running,
      activeTurnId: runningTurnsRef.current.get(chatId),
    });
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    setError(msg || "Failed to load thread");
  }
}
