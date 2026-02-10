import type { Dispatch, SetStateAction } from "react";
import type { ActiveChatThread, ChatThreadSummary } from "@homie/shared";

export function applyThreadOverrides(
  list: ChatThreadSummary[],
  overrides: Record<string, string>,
): ChatThreadSummary[] {
  return list.map((thread) => {
    const override = overrides[thread.chatId];
    if (override) return { ...thread, title: override };
    return thread;
  });
}

interface UpdateThreadOverridesArgs {
  chatId: string;
  title: string | null;
  overrides: Record<string, string>;
  setThreads: Dispatch<SetStateAction<ChatThreadSummary[]>>;
  setActiveThread: Dispatch<SetStateAction<ActiveChatThread | null>>;
}

export function updateThreadOverrides({
  chatId,
  title,
  overrides,
  setThreads,
  setActiveThread,
}: UpdateThreadOverridesArgs) {
  const next = { ...overrides };
  if (title && title.trim()) next[chatId] = title.trim();
  else delete next[chatId];

  setThreads((prev) =>
    prev.map((thread) =>
      thread.chatId === chatId ? { ...thread, title: next[chatId] ?? thread.title } : thread,
    ),
  );

  setActiveThread((prev) =>
    prev && prev.chatId === chatId ? { ...prev, title: next[chatId] ?? prev.title } : prev,
  );

  return next;
}
