import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import {
  shortId,
  type ActiveChatThread,
  type ChatThreadSummary,
} from "@homie/shared";

type CallFn = (method: string, params?: unknown) => Promise<unknown>;

interface CreateChatThreadArgs {
  call: CallFn;
  overrides: Record<string, string>;
  setThreads: Dispatch<SetStateAction<ChatThreadSummary[]>>;
  setActiveChatId: Dispatch<SetStateAction<string | null>>;
  setActiveThread: Dispatch<SetStateAction<ActiveChatThread | null>>;
  setError: Dispatch<SetStateAction<string | null>>;
}

export async function createChatThread({
  call,
  overrides,
  setThreads,
  setActiveChatId,
  setActiveThread,
  setError,
}: CreateChatThreadArgs) {
  try {
    const res = (await call("chat.create")) as { chat_id?: string; thread_id?: string };
    if (!res?.chat_id || !res?.thread_id) return;
    const fallback = `Chat ${shortId(res.chat_id)}`;
    const next: ChatThreadSummary = {
      chatId: res.chat_id,
      threadId: res.thread_id,
      title: overrides[res.chat_id] ?? fallback,
      preview: "",
      status: "active",
      lastActivityAt: Date.now(),
      running: false,
    };
    setThreads((prev) => [next, ...prev]);
    setActiveChatId(res.chat_id);
    setActiveThread({
      chatId: res.chat_id,
      threadId: res.thread_id,
      title: next.title,
      items: [],
      running: false,
    });
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    setError(msg || "Failed to create chat");
  }
}

interface ArchiveChatThreadArgs {
  chatId: string;
  threads: ChatThreadSummary[];
  call: CallFn;
  activeChatIdRef: MutableRefObject<string | null>;
  setThreads: Dispatch<SetStateAction<ChatThreadSummary[]>>;
  setActiveChatId: Dispatch<SetStateAction<string | null>>;
  setActiveThread: Dispatch<SetStateAction<ActiveChatThread | null>>;
  setError: Dispatch<SetStateAction<string | null>>;
}

export async function archiveChatThread({
  chatId,
  threads,
  call,
  activeChatIdRef,
  setThreads,
  setActiveChatId,
  setActiveThread,
  setError,
}: ArchiveChatThreadArgs) {
  const thread = threads.find((entry) => entry.chatId === chatId);
  if (!thread) return;
  try {
    await call("chat.thread.archive", { chat_id: chatId, thread_id: thread.threadId });
    setThreads((prev) => prev.filter((entry) => entry.chatId !== chatId));
    if (activeChatIdRef.current === chatId) {
      setActiveChatId(null);
      setActiveThread(null);
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    setError(msg || "Failed to archive chat");
  }
}

interface RenameChatThreadArgs {
  chatId: string;
  title: string | null;
  call: CallFn;
  updateOverrides: (chatId: string, title: string | null) => void;
}

export async function renameChatThread({
  chatId,
  title,
  call,
  updateOverrides,
}: RenameChatThreadArgs) {
  updateOverrides(chatId, title);
  if (!title || !title.trim()) return;
  try {
    await call("chat.thread.rename", { chat_id: chatId, title: title.trim() });
  } catch {
    return;
  }
}
