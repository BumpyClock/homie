import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import { uuid } from "@/lib/uuid";
import {
  type ActiveChatThread,
  type ChatThreadSummary,
  type ThreadTokenUsage,
  getItemId,
  getThreadId,
  getTurnId,
  itemsFromThread,
  normalizeTokenUsage,
  truncateText,
} from "@/lib/chat-utils";

type ChatEvent = { topic: string; params?: unknown };

interface ChatEventContext {
  threadIdLookupRef: MutableRefObject<Map<string, string>>;
  activeChatIdRef: MutableRefObject<string | null>;
  runningTurnsRef: MutableRefObject<Map<string, string>>;
  messageBufferRef: MutableRefObject<Map<string, string>>;
  setThreads: Dispatch<SetStateAction<ChatThreadSummary[]>>;
  setActiveThread: Dispatch<SetStateAction<ActiveChatThread | null>>;
  setTokenUsageByChatId: Dispatch<SetStateAction<Record<string, ThreadTokenUsage>>>;
}

function resolveApprovalRequestId(params: Record<string, unknown>): number | string | undefined {
  const candidates = [
    params.codex_request_id,
    params.codexRequestId,
    params.request_id,
    params.requestId,
    params.id,
  ];
  for (const candidate of candidates) {
    if (typeof candidate === "number" || typeof candidate === "string") {
      return candidate;
    }
  }
  return undefined;
}

export function handleChatEvent(event: ChatEvent, ctx: ChatEventContext) {
  if (!event.topic.startsWith("chat.")) return;
  const params = (event.params ?? {}) as Record<string, unknown>;
  const threadId = getThreadId(params);
  if (!threadId) return;
  const chatId = ctx.threadIdLookupRef.current.get(threadId) ?? threadId;

  if (event.topic === "chat.turn.started") {
    const turnId = getTurnId(params);
    if (turnId) ctx.runningTurnsRef.current.set(chatId, turnId);
    ctx.setThreads((prev) =>
      prev.map((thread) => (thread.chatId === chatId ? { ...thread, running: true } : thread)),
    );
    if (ctx.activeChatIdRef.current === chatId) {
      ctx.setActiveThread((prev) => (prev ? { ...prev, running: true, activeTurnId: turnId } : prev));
    }
  }

  if (event.topic === "chat.turn.completed") {
    ctx.runningTurnsRef.current.delete(chatId);
    ctx.setThreads((prev) =>
      prev.map((thread) => (thread.chatId === chatId ? { ...thread, running: false } : thread)),
    );
    if (ctx.activeChatIdRef.current === chatId) {
      ctx.setActiveThread((prev) => (prev ? { ...prev, running: false, activeTurnId: undefined } : prev));
    }
  }

  if (event.topic === "chat.message.delta") {
    const itemId = getItemId(params);
    const delta = typeof params.delta === "string" ? params.delta : "";
    const turnId = getTurnId(params);
    if (turnId) {
      ctx.runningTurnsRef.current.set(chatId, turnId);
      ctx.setThreads((prev) =>
        prev.map((thread) => (thread.chatId === chatId ? { ...thread, running: true } : thread)),
      );
      if (ctx.activeChatIdRef.current === chatId) {
        ctx.setActiveThread((prev) => (prev ? { ...prev, running: true, activeTurnId: turnId } : prev));
      }
    }
    const nextText = itemId ? `${ctx.messageBufferRef.current.get(itemId) ?? ""}${delta}` : delta;
    if (itemId) ctx.messageBufferRef.current.set(itemId, nextText);
    if (ctx.activeChatIdRef.current === chatId && itemId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;
        const idx = prev.items.findIndex((item) => item.id === itemId);
        if (idx >= 0) {
          const next = [...prev.items];
          const current = next[idx];
          next[idx] = { ...current, text: nextText };
          return { ...prev, items: next };
        }
        return {
          ...prev,
          items: [...prev.items, { id: itemId, kind: "assistant", role: "assistant", text: nextText }],
        };
      });
    }
    ctx.setThreads((prev) =>
      prev.map((thread) =>
        thread.chatId === chatId
          ? {
              ...thread,
              preview: truncateText(nextText, 96),
              lastActivityAt: Date.now(),
            }
          : thread,
      ),
    );
  }

  if (event.topic === "chat.item.started" || event.topic === "chat.item.completed") {
    const item = params.item as Record<string, unknown> | undefined;
    if (!item || typeof item !== "object") return;
    const itemId = typeof item.id === "string" ? item.id : uuid();
    const turnId = getTurnId(params);
    if (event.topic === "chat.item.started" && turnId) {
      ctx.runningTurnsRef.current.set(chatId, turnId);
      ctx.setThreads((prev) =>
        prev.map((thread) => (thread.chatId === chatId ? { ...thread, running: true } : thread)),
      );
      if (ctx.activeChatIdRef.current === chatId) {
        ctx.setActiveThread((prev) => (prev ? { ...prev, running: true, activeTurnId: turnId } : prev));
      }
    }
    if (ctx.activeChatIdRef.current === chatId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;
        const nextItem = itemsFromThread({ turns: [{ id: getTurnId(params), items: [item] }] })[0];
        if (!nextItem) return prev;
        if (nextItem.kind === "assistant" && nextItem.text) {
          ctx.messageBufferRef.current.set(nextItem.id, nextItem.text);
        }
        if (nextItem.kind === "user") {
          const text = nextItem.text?.trim() ?? "";
          const optimisticIndex = prev.items.findIndex(
            (existing) =>
              existing.kind === "user" &&
              existing.optimistic &&
              (existing.text?.trim() ?? "") === text,
          );
          if (optimisticIndex >= 0) {
            const next = [...prev.items];
            next[optimisticIndex] = { ...nextItem, optimistic: false };
            return { ...prev, items: next };
          }
        }
        const idx = prev.items.findIndex((it) => it.id === itemId);
        if (idx >= 0) {
          const next = [...prev.items];
          next[idx] = { ...next[idx], ...nextItem };
          return { ...prev, items: next };
        }
        return { ...prev, items: [...prev.items, nextItem] };
      });
    }
  }

  if (event.topic === "chat.command.output" || event.topic === "chat.file.output") {
    const itemId = getItemId(params);
    const delta = typeof params.delta === "string" ? params.delta : "";
    if (ctx.activeChatIdRef.current === chatId && itemId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;
        const idx = prev.items.findIndex((item) => item.id === itemId);
        if (idx < 0) return prev;
        const next = [...prev.items];
        const current = next[idx];
        next[idx] = { ...current, output: `${current.output ?? ""}${delta}` };
        return { ...prev, items: next };
      });
    }
  }

  if (event.topic === "chat.diff.updated") {
    const turnId = getTurnId(params) ?? "unknown";
    const diff = typeof params.diff === "string" ? params.diff : "";
    if (ctx.activeChatIdRef.current === chatId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;
        const id = `diff-${turnId}`;
        const idx = prev.items.findIndex((item) => item.id === id);
        if (idx >= 0) {
          const next = [...prev.items];
          next[idx] = { ...next[idx], text: diff };
          return { ...prev, items: next };
        }
        return { ...prev, items: [...prev.items, { id, kind: "diff", turnId, text: diff }] };
      });
    }
  }

  if (event.topic === "chat.plan.updated") {
    const turnId = getTurnId(params) ?? "unknown";
    const plan = Array.isArray(params.plan) ? params.plan : [];
    const explanation = typeof params.explanation === "string" ? params.explanation : "";
    const text = [
      explanation ? `Note: ${explanation}` : "",
      ...plan.map((step: Record<string, unknown>) => {
        const stepText = typeof step.step === "string" ? step.step : "step";
        const status = typeof step.status === "string" ? step.status : "pending";
        return `- [${status}] ${stepText}`;
      }),
    ]
      .filter(Boolean)
      .join("\n");
    if (ctx.activeChatIdRef.current === chatId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;
        const id = `plan-${turnId}`;
        const idx = prev.items.findIndex((item) => item.id === id);
        if (idx >= 0) {
          const next = [...prev.items];
          next[idx] = { ...next[idx], text };
          return { ...prev, items: next };
        }
        return { ...prev, items: [...prev.items, { id, kind: "plan", turnId, text }] };
      });
    }
  }

  if (event.topic === "chat.token.usage.updated") {
    const tokenUsage =
      (params.tokenUsage as Record<string, unknown> | undefined) ??
      (params.token_usage as Record<string, unknown> | undefined);
    if (tokenUsage) {
      const normalized = normalizeTokenUsage(tokenUsage);
      ctx.setTokenUsageByChatId((prev) => ({ ...prev, [chatId]: normalized }));
    }
  }

  if (event.topic === "chat.approval.required") {
    const requestId = resolveApprovalRequestId(params);
    const itemId = getItemId(params) ?? uuid();
    const reason = typeof params.reason === "string" ? params.reason : undefined;
    const command = typeof params.command === "string" ? params.command : undefined;
    const cwd = typeof params.cwd === "string" ? params.cwd : undefined;
    const turnId = getTurnId(params);
    if (turnId) {
      ctx.runningTurnsRef.current.set(chatId, turnId);
      ctx.setThreads((prev) =>
        prev.map((thread) => (thread.chatId === chatId ? { ...thread, running: true } : thread)),
      );
      if (ctx.activeChatIdRef.current === chatId) {
        ctx.setActiveThread((prev) => (prev ? { ...prev, running: true, activeTurnId: turnId } : prev));
      }
    }
    if (typeof window !== "undefined") {
      console.debug("[chat] approval required", {
        requestId,
        threadId,
        turnId: getTurnId(params),
        itemId,
        reason,
        command,
        cwd,
        raw: params,
      });
      if (requestId === undefined) {
        console.warn("[chat] approval request missing id", params);
      }
    }
    if (ctx.activeChatIdRef.current === chatId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          items: [
            ...prev.items,
            {
              id: itemId,
              kind: "approval",
              requestId,
              reason,
              command,
              cwd,
            },
          ],
        };
      });
    }
  }

  ctx.setThreads((prev) =>
    prev.map((thread) =>
      thread.chatId === chatId ? { ...thread, lastActivityAt: Date.now() } : thread,
    ),
  );
}
