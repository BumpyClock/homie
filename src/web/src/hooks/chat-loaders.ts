import type { Dispatch, SetStateAction } from "react";
import type { ConnectionStatus } from "@/hooks/use-gateway";
import {
  type ChatThreadSummary,
  deriveTitleFromThread,
  extractLastMessage,
  itemsFromThread,
  parseCreatedAt,
  shortId,
  truncateText,
} from "@/lib/chat-utils";

type CallFn = (method: string, params?: unknown) => Promise<unknown>;

interface HydrateThreadArgs {
  chatId: string;
  threadId: string;
  call: CallFn;
  overrides: Record<string, string>;
  setThreads: Dispatch<SetStateAction<ChatThreadSummary[]>>;
  applySettings?: (chatId: string, settings: unknown) => void;
}

export async function hydrateThread({
  chatId,
  threadId,
  call,
  overrides,
  setThreads,
  applySettings,
}: HydrateThreadArgs) {
  try {
    const res = await call("chat.thread.read", {
      chat_id: chatId,
      thread_id: threadId,
      include_turns: true,
    });
    const settings = (res as Record<string, unknown>)?.settings;
    if (settings) {
      applySettings?.(chatId, settings);
    }
    const thread = (res as Record<string, unknown>)?.thread ?? res;
    if (!thread || typeof thread !== "object") return;
    const items = itemsFromThread(thread as Record<string, unknown>);
    const preview = extractLastMessage(items);
    const updatedAt =
      typeof (thread as Record<string, unknown>).updated_at === "number"
        ? ((thread as Record<string, unknown>).updated_at as number) * 1000
        : undefined;
    setThreads((prev) =>
      prev.map((t) => {
        if (t.chatId !== chatId) return t;
        const baseTitle = overrides[chatId] ?? deriveTitleFromThread(thread as Record<string, unknown>, t.title);
        return {
          ...t,
          title: baseTitle,
          preview: preview ? truncateText(preview, 96) : t.preview,
          lastActivityAt: updatedAt ?? t.lastActivityAt,
        };
      }),
    );
  } catch {
    return;
  }
}

interface LoadChatsArgs {
  enabled: boolean;
  status: ConnectionStatus;
  call: CallFn;
  overrides: Record<string, string>;
  applyOverrides: (threads: ChatThreadSummary[]) => ChatThreadSummary[];
  applySettings?: (chatId: string, settings: unknown) => void;
  setThreads: Dispatch<SetStateAction<ChatThreadSummary[]>>;
  setError: Dispatch<SetStateAction<string | null>>;
  hydrate: (chatId: string, threadId: string) => void;
}

export async function loadChats({
  enabled,
  status,
  call,
  overrides,
  applyOverrides,
  applySettings,
  setThreads,
  setError,
  hydrate,
}: LoadChatsArgs) {
  if (!enabled || status !== "connected") return;
  try {
    const res = (await call("chat.list")) as {
      chats?: Array<{
        chat_id: string;
        thread_id: string;
        created_at: string;
        status: ChatThreadSummary["status"];
        settings?: unknown;
      }>;
    };
    const list = res.chats || [];
    for (const entry of list) {
      if (entry.settings) applySettings?.(entry.chat_id, entry.settings);
    }
    const next = list.map((rec) => {
      const fallback = `Chat ${shortId(rec.chat_id)}`;
      const title = overrides[rec.chat_id] ?? fallback;
      return {
        chatId: rec.chat_id,
        threadId: rec.thread_id,
        title,
        preview: "",
        status: rec.status,
        lastActivityAt: parseCreatedAt(rec.created_at),
        running: false,
      } satisfies ChatThreadSummary;
    });
    setThreads(applyOverrides(next));
    setError(null);
    for (const entry of next) {
      hydrate(entry.chatId, entry.threadId);
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    setError(msg || "Failed to load chats");
  }
}
