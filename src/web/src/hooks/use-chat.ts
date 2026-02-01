import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ConnectionStatus } from "@/hooks/use-gateway";
import { uuid } from "@/lib/uuid";

export type ChatStatus = "active" | "inactive" | "exited";

export type ChatItemKind =
  | "user"
  | "assistant"
  | "plan"
  | "reasoning"
  | "command"
  | "file"
  | "diff"
  | "approval"
  | "tool"
  | "system";

export interface ChatItem {
  id: string;
  kind: ChatItemKind;
  text?: string;
  summary?: string[];
  content?: string[];
  command?: string;
  cwd?: string;
  output?: string;
  changes?: Array<{ path: string; diff: string; kind?: string }>;
  status?: string;
  turnId?: string;
  role?: "user" | "assistant";
  requestId?: number;
  reason?: string;
  raw?: unknown;
}

export interface ChatThreadSummary {
  chatId: string;
  threadId: string;
  title: string;
  preview: string;
  status: ChatStatus;
  lastActivityAt?: number;
  running: boolean;
}

export interface ActiveChatThread {
  chatId: string;
  threadId: string;
  title: string;
  items: ChatItem[];
  running: boolean;
  activeTurnId?: string;
}

interface ChatListResponse {
  chats: Array<{
    chat_id: string;
    thread_id: string;
    created_at: string;
    status: ChatStatus;
  }>;
}

interface UseChatOptions {
  status: ConnectionStatus;
  call: (method: string, params?: unknown) => Promise<unknown>;
  onEvent: (callback: (event: { topic: string; params?: unknown }) => void) => () => void;
  enabled: boolean;
  namespace: string;
}

const OVERRIDE_KEY_PREFIX = "homie-chat-overrides:";

function overridesKey(namespace: string) {
  return `${OVERRIDE_KEY_PREFIX}${namespace || "default"}`;
}

function loadOverrides(namespace: string): Record<string, string> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(overridesKey(namespace));
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") {
      return parsed as Record<string, string>;
    }
  } catch {
    return {};
  }
  return {};
}

function saveOverrides(namespace: string, overrides: Record<string, string>) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(overridesKey(namespace), JSON.stringify(overrides));
  } catch {
    return;
  }
}

function shortId(id: string) {
  return id.slice(0, 8);
}

function truncateText(text: string, limit: number) {
  const trimmed = text.trim();
  if (trimmed.length <= limit) return trimmed;
  return `${trimmed.slice(0, limit).trim()}…`;
}

function parseCreatedAt(value?: string) {
  if (!value) return undefined;
  const num = Number.parseInt(value, 10);
  if (Number.isNaN(num)) return undefined;
  return num * 1000;
}

function getThreadId(params: Record<string, unknown> | undefined): string | undefined {
  if (!params) return undefined;
  const direct = params.threadId ?? params.thread_id;
  if (typeof direct === "string") return direct;
  const nested = (params.thread as { id?: string } | undefined)?.id;
  if (typeof nested === "string") return nested;
  return undefined;
}

function getTurnId(params: Record<string, unknown> | undefined): string | undefined {
  if (!params) return undefined;
  const direct = params.turnId ?? params.turn_id;
  return typeof direct === "string" ? direct : undefined;
}

function getItemId(params: Record<string, unknown> | undefined): string | undefined {
  if (!params) return undefined;
  const direct = params.itemId ?? params.item_id;
  return typeof direct === "string" ? direct : undefined;
}

function extractUserText(content: Array<Record<string, unknown>>): string {
  if (!Array.isArray(content)) return "";
  const parts = content.map((block) => {
    const type = block.type;
    if (type === "text" && typeof block.text === "string") return block.text;
    if (type === "image" && typeof block.url === "string") return `[image] ${block.url}`;
    if (type === "localImage" && typeof block.path === "string") return `[image] ${block.path}`;
    if (type === "skill" && typeof block.name === "string") return `[skill] ${block.name}`;
    if (type === "mention" && typeof block.name === "string") return `@${block.name}`;
    return `[${String(type ?? "input")}]`;
  });
  return parts.filter(Boolean).join("\n");
}

function itemsFromThread(thread: Record<string, unknown>): ChatItem[] {
  const turns = Array.isArray(thread.turns) ? thread.turns : [];
  const items: ChatItem[] = [];
  for (const turn of turns) {
    const turnId = typeof turn?.id === "string" ? turn.id : undefined;
    const turnItems = Array.isArray(turn?.items) ? turn.items : [];
    for (const item of turnItems) {
      if (!item || typeof item !== "object") continue;
      const type = item.type;
      const id = typeof item.id === "string" ? item.id : uuid();
      if (type === "userMessage") {
        items.push({
          id,
          kind: "user",
          role: "user",
          turnId,
          text: extractUserText(item.content as Array<Record<string, unknown>>),
        });
      } else if (type === "agentMessage") {
        items.push({
          id,
          kind: "assistant",
          role: "assistant",
          turnId,
          text: typeof item.text === "string" ? item.text : "",
        });
      } else if (type === "plan") {
        items.push({
          id,
          kind: "plan",
          turnId,
          text: typeof item.text === "string" ? item.text : "",
        });
      } else if (type === "reasoning") {
        items.push({
          id,
          kind: "reasoning",
          turnId,
          summary: Array.isArray(item.summary) ? item.summary : [],
          content: Array.isArray(item.content) ? item.content : [],
        });
      } else if (type === "commandExecution") {
        items.push({
          id,
          kind: "command",
          turnId,
          command: typeof item.command === "string" ? item.command : "",
          cwd: typeof item.cwd === "string" ? item.cwd : undefined,
          output: typeof item.aggregatedOutput === "string" ? item.aggregatedOutput : undefined,
          status: typeof item.status === "string" ? item.status : undefined,
        });
      } else if (type === "fileChange") {
        items.push({
          id,
          kind: "file",
          turnId,
          status: typeof item.status === "string" ? item.status : undefined,
          changes: Array.isArray(item.changes)
            ? item.changes.map((change: Record<string, unknown>) => ({
                path: typeof change.path === "string" ? change.path : "unknown",
                diff: typeof change.diff === "string" ? change.diff : "",
                kind: typeof change.kind === "string" ? change.kind : undefined,
              }))
            : [],
        });
      } else if (type === "mcpToolCall") {
        items.push({
          id,
          kind: "tool",
          turnId,
          text: typeof item.tool === "string" ? item.tool : "Tool call",
          status: typeof item.status === "string" ? item.status : undefined,
          raw: item,
        });
      } else if (type === "webSearch") {
        items.push({
          id,
          kind: "system",
          turnId,
          text: typeof item.query === "string" ? `Web search: ${item.query}` : "Web search",
          raw: item,
        });
      } else {
        items.push({
          id,
          kind: "system",
          turnId,
          text: `Item: ${String(type ?? "unknown")}`,
          raw: item,
        });
      }
    }
  }
  return items;
}

function extractLastMessage(items: ChatItem[]): string {
  let last = "";
  for (const item of items) {
    if ((item.kind === "user" || item.kind === "assistant") && item.text) {
      last = item.text;
    }
  }
  return last;
}

function deriveTitleFromThread(thread: Record<string, unknown>, fallback: string) {
  const preview = typeof thread.preview === "string" ? thread.preview : "";
  if (preview.trim().length > 0) return truncateText(preview, 42);
  const items = itemsFromThread(thread);
  const firstMessage = items.find((item) => item.kind === "user" && item.text);
  if (firstMessage?.text) return truncateText(firstMessage.text, 42);
  return fallback;
}

function formatRelativeTime(timestamp?: number) {
  if (!timestamp) return "";
  const diff = Date.now() - timestamp;
  if (diff < 60_000) return "just now";
  const mins = Math.floor(diff / 60_000);
  if (mins < 60) return `${mins}m`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h`;
  const days = Math.floor(hrs / 24);
  if (days < 7) return `${days}d`;
  return new Date(timestamp).toLocaleDateString();
}

export function useChat({ status, call, onEvent, enabled, namespace }: UseChatOptions) {
  const [threads, setThreads] = useState<ChatThreadSummary[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [activeThread, setActiveThread] = useState<ActiveChatThread | null>(null);
  const [account, setAccount] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const overridesRef = useRef<Record<string, string>>({});
  const threadIdLookupRef = useRef<Map<string, string>>(new Map());
  const activeChatIdRef = useRef<string | null>(null);
  const runningTurnsRef = useRef<Map<string, string>>(new Map());
  const hydratedRef = useRef<Set<string>>(new Set());
  const messageBufferRef = useRef<Map<string, string>>(new Map());

  useEffect(() => {
    overridesRef.current = loadOverrides(namespace);
  }, [namespace]);

  useEffect(() => {
    activeChatIdRef.current = activeChatId;
  }, [activeChatId]);

  useEffect(() => {
    const map = new Map<string, string>();
    threads.forEach((thread) => map.set(thread.threadId, thread.chatId));
    threadIdLookupRef.current = map;
  }, [threads]);

  const applyOverrides = useCallback((list: ChatThreadSummary[]) => {
    const overrides = overridesRef.current;
    return list.map((thread) => {
      const override = overrides[thread.chatId];
      if (override) return { ...thread, title: override };
      return thread;
    });
  }, []);

  const updateOverrides = useCallback((chatId: string, title: string | null) => {
    const next = { ...overridesRef.current };
    if (title && title.trim()) next[chatId] = title.trim();
    else delete next[chatId];
    overridesRef.current = next;
    saveOverrides(namespace, next);
    setThreads((prev) =>
      prev.map((thread) => (thread.chatId === chatId ? { ...thread, title: next[chatId] ?? thread.title } : thread))
    );
    setActiveThread((prev) =>
      prev && prev.chatId === chatId ? { ...prev, title: next[chatId] ?? prev.title } : prev
    );
  }, [namespace]);

  const hydrateThread = useCallback(async (chatId: string, threadId: string) => {
    if (hydratedRef.current.has(chatId)) return;
    hydratedRef.current.add(chatId);
    try {
      const res = await call("chat.thread.read", { chat_id: chatId, thread_id: threadId, include_turns: true });
      const thread = (res as Record<string, unknown>)?.thread ?? res;
      if (!thread || typeof thread !== "object") return;
      const items = itemsFromThread(thread as Record<string, unknown>);
      const preview = extractLastMessage(items);
      const updatedAt = typeof (thread as Record<string, unknown>).updated_at === "number"
        ? (thread as Record<string, unknown>).updated_at as number * 1000
        : undefined;
      setThreads((prev) =>
        prev.map((t) => {
          if (t.chatId !== chatId) return t;
          const baseTitle = overridesRef.current[chatId] ?? deriveTitleFromThread(thread as Record<string, unknown>, t.title);
          return {
            ...t,
            title: baseTitle,
            preview: preview ? truncateText(preview, 96) : t.preview,
            lastActivityAt: updatedAt ?? t.lastActivityAt,
          };
        })
      );
    } catch {
      return;
    }
  }, [call]);

  const loadChats = useCallback(async () => {
    if (!enabled || status !== "connected") return;
    try {
      const res = await call("chat.list") as ChatListResponse;
      const list = res.chats || [];
      const next = list.map((rec) => {
        const fallback = `Chat ${shortId(rec.chat_id)}`;
        const title = overridesRef.current[rec.chat_id] ?? fallback;
        return {
          chatId: rec.chat_id,
          threadId: rec.thread_id,
          title,
          preview: "",
          status: rec.status,
          lastActivityAt: parseCreatedAt(rec.created_at),
          running: false,
        };
      });
      setThreads(applyOverrides(next));
      setError(null);
      for (const entry of next) {
        void hydrateThread(entry.chatId, entry.threadId);
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || "Failed to load chats");
    }
  }, [applyOverrides, enabled, hydrateThread, status, call]);

  const refreshAccount = useCallback(async () => {
    if (!enabled || status !== "connected") return;
    try {
      const res = await call("chat.account.read") as Record<string, unknown>;
      console.debug("[chat] account.read", res);
      setAccount(res);
    } catch {
      setAccount(null);
    }
  }, [call, enabled, status]);

  useEffect(() => {
    if (!enabled || status !== "connected") return;
    void refreshAccount();
    void loadChats();
  }, [enabled, status, loadChats, refreshAccount]);

  useEffect(() => {
    if (status === "connected" && enabled) return;
    setThreads([]);
    setActiveChatId(null);
    setActiveThread(null);
  }, [status, enabled]);

  useEffect(() => {
    if (!enabled || status !== "connected") return;
    let subscribed = false;
    void call("events.subscribe", { topic: "chat.*" })
      .then(() => { subscribed = true; })
      .catch(() => { subscribed = false; });
    const unsubscribe = onEvent((evt) => {
      if (!evt.topic.startsWith("chat.")) return;
      const params = (evt.params ?? {}) as Record<string, unknown>;
      const threadId = getThreadId(params);
      if (!threadId) return;
      const chatId = threadIdLookupRef.current.get(threadId) ?? threadId;
      if (evt.topic === "chat.turn.started") {
        const turnId = getTurnId(params);
        if (turnId) runningTurnsRef.current.set(chatId, turnId);
        setThreads((prev) =>
          prev.map((thread) => thread.chatId === chatId ? { ...thread, running: true } : thread)
        );
        if (activeChatIdRef.current === chatId) {
          setActiveThread((prev) => prev ? { ...prev, running: true, activeTurnId: turnId } : prev);
        }
      }
      if (evt.topic === "chat.turn.completed") {
        runningTurnsRef.current.delete(chatId);
        setThreads((prev) =>
          prev.map((thread) => thread.chatId === chatId ? { ...thread, running: false } : thread)
        );
        if (activeChatIdRef.current === chatId) {
          setActiveThread((prev) => prev ? { ...prev, running: false, activeTurnId: undefined } : prev);
        }
      }
      if (evt.topic === "chat.message.delta") {
        const itemId = getItemId(params);
        const delta = typeof params.delta === "string" ? params.delta : "";
        const nextText = itemId
          ? `${messageBufferRef.current.get(itemId) ?? ""}${delta}`
          : delta;
        if (itemId) messageBufferRef.current.set(itemId, nextText);
        if (activeChatIdRef.current === chatId && itemId) {
          setActiveThread((prev) => {
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
        setThreads((prev) =>
          prev.map((thread) =>
            thread.chatId === chatId
              ? {
                  ...thread,
                  preview: truncateText(nextText, 96),
                  lastActivityAt: Date.now(),
                }
              : thread
          )
        );
      }
      if (evt.topic === "chat.item.started" || evt.topic === "chat.item.completed") {
        const item = params.item as Record<string, unknown> | undefined;
        if (!item || typeof item !== "object") return;
        const itemId = typeof item.id === "string" ? item.id : uuid();
        if (activeChatIdRef.current === chatId) {
          setActiveThread((prev) => {
            if (!prev) return prev;
            const nextItem = itemsFromThread({ turns: [{ id: getTurnId(params), items: [item] }] })[0];
            if (nextItem?.kind === "assistant" && nextItem.text) {
              messageBufferRef.current.set(nextItem.id, nextItem.text);
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
      if (evt.topic === "chat.command.output" || evt.topic === "chat.file.output") {
        const itemId = getItemId(params);
        const delta = typeof params.delta === "string" ? params.delta : "";
        if (activeChatIdRef.current === chatId && itemId) {
          setActiveThread((prev) => {
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
      if (evt.topic === "chat.diff.updated") {
        const turnId = getTurnId(params) ?? "unknown";
        const diff = typeof params.diff === "string" ? params.diff : "";
        if (activeChatIdRef.current === chatId) {
          setActiveThread((prev) => {
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
      if (evt.topic === "chat.plan.updated") {
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
        ].filter(Boolean).join("\n");
        if (activeChatIdRef.current === chatId) {
          setActiveThread((prev) => {
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
      if (evt.topic === "chat.approval.required") {
        const requestId = typeof params.codex_request_id === "number" ? params.codex_request_id : undefined;
        const itemId = getItemId(params) ?? uuid();
        const reason = typeof params.reason === "string" ? params.reason : undefined;
        const command = typeof params.command === "string" ? params.command : undefined;
        const cwd = typeof params.cwd === "string" ? params.cwd : undefined;
        if (activeChatIdRef.current === chatId) {
          setActiveThread((prev) => {
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
      setThreads((prev) =>
        prev.map((thread) =>
          thread.chatId === chatId
            ? { ...thread, lastActivityAt: Date.now() }
            : thread
        )
      );
    });
    return () => {
      if (subscribed) {
        // no unsubscribe method available yet
      }
      unsubscribe();
    };
  }, [call, enabled, onEvent, status]);

  const sortedThreads = useMemo(() => {
    return [...threads].sort((a, b) => (b.lastActivityAt ?? 0) - (a.lastActivityAt ?? 0));
  }, [threads]);

  const selectChat = useCallback(async (chatId: string) => {
    const thread = threads.find((t) => t.chatId === chatId);
    if (!thread) return;
    setActiveChatId(chatId);
    try {
      await call("chat.resume", { chat_id: chatId, thread_id: thread.threadId });
    } catch {
      // ignore resume errors; allow read attempt
    }
    try {
      const res = await call("chat.thread.read", { chat_id: chatId, thread_id: thread.threadId, include_turns: true });
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
      const updatedAt = typeof (threadRes as Record<string, unknown>).updated_at === "number"
        ? (threadRes as Record<string, unknown>).updated_at as number * 1000
        : undefined;
      const title = overridesRef.current[chatId] ?? deriveTitleFromThread(threadRes as Record<string, unknown>, thread.title);
      setThreads((prev) =>
        prev.map((t) =>
          t.chatId === chatId
            ? { ...t, title, preview: preview ? truncateText(preview, 96) : t.preview, lastActivityAt: updatedAt ?? t.lastActivityAt }
            : t
        )
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
  }, [call, threads]);

  const createChat = useCallback(async () => {
    try {
      const res = await call("chat.create") as { chat_id?: string; thread_id?: string };
      if (!res?.chat_id || !res?.thread_id) return;
      const fallback = `Chat ${shortId(res.chat_id)}`;
      const next: ChatThreadSummary = {
        chatId: res.chat_id,
        threadId: res.thread_id,
        title: overridesRef.current[res.chat_id] ?? fallback,
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
  }, [call]);

  const sendMessage = useCallback(async (message: string) => {
    if (!activeThread) return;
    const trimmed = message.trim();
    if (!trimmed) return;
    const localId = uuid();
    setActiveThread((prev) => prev ? {
      ...prev,
      items: [...prev.items, { id: localId, kind: "user", role: "user", text: trimmed }],
    } : prev);
    if (!overridesRef.current[activeThread.chatId]) {
      const autoTitle = truncateText(trimmed, 42);
      updateOverrides(activeThread.chatId, autoTitle);
    }
    try {
      const res = await call("chat.message.send", { chat_id: activeThread.chatId, message: trimmed }) as { turn_id?: string };
      if (res?.turn_id) {
        runningTurnsRef.current.set(activeThread.chatId, res.turn_id);
        setActiveThread((prev) => prev ? { ...prev, running: true, activeTurnId: res.turn_id } : prev);
        setThreads((prev) =>
          prev.map((thread) =>
            thread.chatId === activeThread.chatId
              ? { ...thread, running: true, lastActivityAt: Date.now(), preview: truncateText(trimmed, 96) }
              : thread
          )
        );
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || "Failed to send message");
    }
  }, [activeThread, call, updateOverrides]);

  const cancelActive = useCallback(async () => {
    if (!activeThread?.activeTurnId) return;
    try {
      await call("chat.cancel", { chat_id: activeThread.chatId, turn_id: activeThread.activeTurnId });
    } catch {
      return;
    }
  }, [activeThread, call]);

  const archiveChat = useCallback(async (chatId: string) => {
    const thread = threads.find((t) => t.chatId === chatId);
    if (!thread) return;
    try {
      await call("chat.thread.archive", { chat_id: chatId, thread_id: thread.threadId });
      setThreads((prev) => prev.filter((t) => t.chatId !== chatId));
      if (activeChatIdRef.current === chatId) {
        setActiveChatId(null);
        setActiveThread(null);
      }
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || "Failed to archive chat");
    }
  }, [call, threads]);

  const renameChat = useCallback(async (chatId: string, title: string | null) => {
    updateOverrides(chatId, title);
    if (!title || !title.trim()) return;
    try {
      await call("chat.thread.rename", { chat_id: chatId, title: title.trim() });
    } catch {
      return;
    }
  }, [call, updateOverrides]);

  const respondApproval = useCallback(async (requestId: number, decision: "accept" | "decline") => {
    try {
      await call("chat.approval.respond", { codex_request_id: requestId, decision });
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || "Approval failed");
    }
  }, [call]);

  const clearError = useCallback(() => setError(null), []);

  const accountStatus = useMemo(() => {
    if (!account) return { ok: false, message: "Not connected to Codex CLI." };
    const raw = account as { requiresOpenaiAuth?: boolean; requires_openai_auth?: boolean; account?: unknown };
    const requires = raw.requiresOpenaiAuth ?? raw.requires_openai_auth ?? false;
    const hasAccount = !!raw.account;
    if (!hasAccount && requires) {
      return { ok: false, message: "Codex CLI not logged in. Run `codex login` on the gateway host." };
    }
    if (!hasAccount) {
      return { ok: false, message: "Codex CLI not logged in. Run `codex login` on the gateway host." };
    }
    return { ok: true, message: "" };
  }, [account]);

  return {
    threads: sortedThreads,
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
  };
}
