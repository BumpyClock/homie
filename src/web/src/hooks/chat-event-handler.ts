import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import {
  mapChatEvent,
  truncateText,
  type ActiveChatThread,
  type ChatGatewayEvent,
  type ChatThreadSummary,
  type ThreadTokenUsage,
} from "@homie/shared";
import { uuid } from "@/lib/uuid";

interface ChatEventContext {
  threadIdLookupRef: MutableRefObject<Map<string, string>>;
  activeChatIdRef: MutableRefObject<string | null>;
  runningTurnsRef: MutableRefObject<Map<string, string>>;
  messageBufferRef: MutableRefObject<Map<string, string>>;
  setThreads: Dispatch<SetStateAction<ChatThreadSummary[]>>;
  setActiveThread: Dispatch<SetStateAction<ActiveChatThread | null>>;
  setTokenUsageByChatId: Dispatch<SetStateAction<Record<string, ThreadTokenUsage>>>;
}

function setRunningState(
  ctx: ChatEventContext,
  chatId: string,
  running: boolean,
  turnId?: string,
) {
  if (running && turnId) {
    ctx.runningTurnsRef.current.set(chatId, turnId);
  }

  if (!running) {
    ctx.runningTurnsRef.current.delete(chatId);
  }

  ctx.setThreads((prev) =>
    prev.map((thread) =>
      thread.chatId === chatId ? { ...thread, running } : thread,
    ),
  );

  if (ctx.activeChatIdRef.current === chatId) {
    ctx.setActiveThread((prev) =>
      prev
        ? {
            ...prev,
            running,
            activeTurnId: running ? turnId : undefined,
          }
        : prev,
    );
  }
}

function upsertThreadActivity(
  ctx: ChatEventContext,
  chatId: string,
  lastActivityAt: number,
  preview?: string,
) {
  ctx.setThreads((prev) =>
    prev.map((thread) =>
      thread.chatId === chatId
        ? {
            ...thread,
            preview: preview ?? thread.preview,
            lastActivityAt,
          }
        : thread,
    ),
  );
}

export function handleChatEvent(event: ChatGatewayEvent, ctx: ChatEventContext) {
  const mapped = mapChatEvent(event, {
    threadIdLookup: ctx.threadIdLookupRef.current,
    messageBuffer: ctx.messageBufferRef.current,
    idFactory: uuid,
  });

  if (!mapped) return;

  if (mapped.type === "turn.started") {
    setRunningState(ctx, mapped.chatId, true, mapped.turnId);
  }

  if (mapped.type === "turn.completed") {
    setRunningState(ctx, mapped.chatId, false);
  }

  if (mapped.type === "message.delta") {
    if (mapped.turnId) {
      setRunningState(ctx, mapped.chatId, true, mapped.turnId);
    }

    if (ctx.activeChatIdRef.current === mapped.chatId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;
        const fallbackId = mapped.turnId
          ? `assistant-${mapped.turnId}`
          : `assistant-${mapped.activityAt}`;
        const itemId = mapped.itemId ?? fallbackId;
        const index = prev.items.findIndex((item) => item.id === itemId);
        if (index >= 0) {
          const next = [...prev.items];
          next[index] = { ...next[index], text: mapped.text };
          return { ...prev, items: next };
        }
        return {
          ...prev,
          items: [
            ...prev.items,
            {
              id: itemId,
              kind: "assistant",
              role: "assistant",
              text: mapped.text,
            },
          ],
        };
      });
    }

    upsertThreadActivity(
      ctx,
      mapped.chatId,
      mapped.activityAt,
      truncateText(mapped.text, 96),
    );
  }

  if (mapped.type === "item.started" || mapped.type === "item.completed") {
    if (mapped.type === "item.started" && mapped.turnId) {
      setRunningState(ctx, mapped.chatId, true, mapped.turnId);
    }

    if (ctx.activeChatIdRef.current === mapped.chatId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;

        if (mapped.item.kind === "assistant" && mapped.item.text) {
          ctx.messageBufferRef.current.set(mapped.item.id, mapped.item.text);
        }

        if (mapped.item.kind === "user") {
          const text = mapped.item.text?.trim() ?? "";
          const optimisticIndex = prev.items.findIndex(
            (existing) =>
              existing.kind === "user" &&
              existing.optimistic &&
              (existing.text?.trim() ?? "") === text,
          );

          if (optimisticIndex >= 0) {
            const next = [...prev.items];
            next[optimisticIndex] = { ...mapped.item, optimistic: false };
            return { ...prev, items: next };
          }
        }

        const index = prev.items.findIndex((item) => item.id === mapped.item.id);
        if (index >= 0) {
          const next = [...prev.items];
          next[index] = { ...next[index], ...mapped.item };
          return { ...prev, items: next };
        }

        return { ...prev, items: [...prev.items, mapped.item] };
      });
    }
  }

  if (mapped.type === "command.output" || mapped.type === "file.output") {
    if (ctx.activeChatIdRef.current === mapped.chatId && mapped.itemId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;
        const index = prev.items.findIndex((item) => item.id === mapped.itemId);
        if (index < 0) return prev;
        const next = [...prev.items];
        const current = next[index];
        next[index] = { ...current, output: `${current.output ?? ""}${mapped.delta}` };
        return { ...prev, items: next };
      });
    }
  }

  if (mapped.type === "diff.updated") {
    if (ctx.activeChatIdRef.current === mapped.chatId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;
        const id = `diff-${mapped.turnId ?? "unknown"}`;
        const index = prev.items.findIndex((item) => item.id === id);
        if (index >= 0) {
          const next = [...prev.items];
          next[index] = { ...next[index], text: mapped.diff };
          return { ...prev, items: next };
        }
        return {
          ...prev,
          items: [
            ...prev.items,
            {
              id,
              kind: "diff",
              turnId: mapped.turnId,
              text: mapped.diff,
            },
          ],
        };
      });
    }
  }

  if (mapped.type === "plan.updated") {
    if (ctx.activeChatIdRef.current === mapped.chatId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;
        const id = `plan-${mapped.turnId ?? "unknown"}`;
        const index = prev.items.findIndex((item) => item.id === id);
        if (index >= 0) {
          const next = [...prev.items];
          next[index] = { ...next[index], text: mapped.text };
          return { ...prev, items: next };
        }
        return {
          ...prev,
          items: [
            ...prev.items,
            {
              id,
              kind: "plan",
              turnId: mapped.turnId,
              text: mapped.text,
            },
          ],
        };
      });
    }
  }

  if (mapped.type === "token.usage.updated") {
    ctx.setTokenUsageByChatId((prev) => ({
      ...prev,
      [mapped.chatId]: mapped.tokenUsage,
    }));
  }

  if (mapped.type === "approval.required") {
    if (mapped.turnId) {
      setRunningState(ctx, mapped.chatId, true, mapped.turnId);
    }

    if (typeof window !== "undefined") {
      console.debug("[chat] approval required", {
        requestId: mapped.requestId,
        threadId: mapped.threadId,
        turnId: mapped.turnId,
        itemId: mapped.itemId,
        reason: mapped.reason,
        command: mapped.command,
        cwd: mapped.cwd,
        raw: mapped.rawParams,
      });
      if (mapped.requestId === undefined) {
        console.warn("[chat] approval request missing id", mapped.rawParams);
      }
    }

    if (ctx.activeChatIdRef.current === mapped.chatId) {
      ctx.setActiveThread((prev) => {
        if (!prev) return prev;
        return {
          ...prev,
          items: [
            ...prev.items,
            {
              id: mapped.itemId,
              kind: "approval",
              requestId: mapped.requestId,
              reason: mapped.reason,
              command: mapped.command,
              cwd: mapped.cwd,
            },
          ],
        };
      });
    }
  }

  upsertThreadActivity(ctx, mapped.chatId, mapped.activityAt);
}
