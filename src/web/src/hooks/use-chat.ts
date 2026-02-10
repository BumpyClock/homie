import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ConnectionStatus } from "@/hooks/use-gateway";
import { uuid } from "@/lib/uuid";
import { handleChatEvent } from "@/hooks/chat-event-handler";
import { hydrateThread as hydrateThreadImpl, loadChats as loadChatsImpl } from "@/hooks/chat-loaders";
import { selectChat as selectChatImpl } from "@/hooks/chat-selection";
import {
  approvalPolicyForPermission,
  buildCollaborationPayload as buildCollaborationPayloadShared,
  type ActiveChatThread,
  type ChatSettings,
  type ChatThreadSummary,
  type ChatWebToolName,
  type CollaborationModeOption,
  type FileOption,
  type ModelOption,
  type SkillOption,
  type ThreadTokenUsage,
  formatRelativeTime,
  normalizeCollaborationModes,
  normalizeEnabledWebTools,
  normalizeChatSettings,
  normalizeFileOptions,
  normalizeModelOptions,
  normalizeSkillOptions,
  shortId,
  truncateText,
} from "@homie/shared";

interface UseChatOptions {
  status: ConnectionStatus;
  call: (method: string, params?: unknown) => Promise<unknown>;
  onEvent: (callback: (event: { topic: string; params?: unknown }) => void) => () => void;
  enabled: boolean;
  namespace: string;
}

const OVERRIDE_KEY_PREFIX = "homie-chat-overrides:";
const SETTINGS_KEY_PREFIX = "homie-chat-settings:";

function overridesKey(namespace: string) {
  return `${OVERRIDE_KEY_PREFIX}${namespace || "default"}`;
}

function settingsKey(namespace: string) {
  return `${SETTINGS_KEY_PREFIX}${namespace || "default"}`;
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

function loadSettings(namespace: string): Record<string, ChatSettings> {
  if (typeof window === "undefined") return {};
  try {
    const raw = window.localStorage.getItem(settingsKey(namespace));
    if (!raw) return {};
    const parsed = JSON.parse(raw);
    if (parsed && typeof parsed === "object") {
      return parsed as Record<string, ChatSettings>;
    }
  } catch {
    return {};
  }
  return {};
}

function saveSettings(namespace: string, settings: Record<string, ChatSettings>) {
  if (typeof window === "undefined") return;
  try {
    window.localStorage.setItem(settingsKey(namespace), JSON.stringify(settings));
  } catch {
    return;
  }
}

export function useChat({ status, call, onEvent, enabled, namespace }: UseChatOptions) {
  const [threads, setThreads] = useState<ChatThreadSummary[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [activeThread, setActiveThread] = useState<ActiveChatThread | null>(null);
  const [account, setAccount] = useState<Record<string, unknown> | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [queuedNoticeByChatId, setQueuedNoticeByChatId] = useState<Record<string, boolean>>({});
  const [models, setModels] = useState<ModelOption[]>([]);
  const [collaborationModes, setCollaborationModes] = useState<CollaborationModeOption[]>([]);
  const [skills, setSkills] = useState<SkillOption[]>([]);
  const [enabledWebTools, setEnabledWebTools] = useState<ChatWebToolName[]>([]);
  const [webToolsAvailable, setWebToolsAvailable] = useState(false);
  const [settingsByChatId, setSettingsByChatId] = useState<Record<string, ChatSettings>>({});
  const [tokenUsageByChatId, setTokenUsageByChatId] = useState<Record<string, ThreadTokenUsage>>({});
  const overridesRef = useRef<Record<string, string>>({});
  const settingsRef = useRef<Record<string, ChatSettings>>({});
  const threadIdLookupRef = useRef<Map<string, string>>(new Map());
  const activeChatIdRef = useRef<string | null>(null);
  const runningTurnsRef = useRef<Map<string, string>>(new Map());
  const queuedTimersRef = useRef<Record<string, ReturnType<typeof setTimeout>>>({});
  const hydratedRef = useRef<Set<string>>(new Set());
  const messageBufferRef = useRef<Map<string, string>>(new Map());
  const bootstrappedRef = useRef(false);

  useEffect(() => {
    activeChatIdRef.current = activeChatId;
  }, [activeChatId]);

  useEffect(() => {
    bootstrappedRef.current = false;
    setEnabledWebTools([]);
    setWebToolsAvailable(false);
  }, [namespace]);

  const refreshLocalSettings = useCallback(() => {
    overridesRef.current = loadOverrides(namespace);
    settingsRef.current = loadSettings(namespace);
    setSettingsByChatId(settingsRef.current);
  }, [namespace]);

  useEffect(() => {
    const map = new Map<string, string>();
    threads.forEach((thread) => map.set(thread.threadId, thread.chatId));
    threadIdLookupRef.current = map;
  }, [threads]);

  const clearQueuedNotice = useCallback((chatId: string) => {
    setQueuedNoticeByChatId((prev) => {
      if (!prev[chatId]) return prev;
      const next = { ...prev };
      delete next[chatId];
      return next;
    });
    const timer = queuedTimersRef.current[chatId];
    if (timer) {
      clearTimeout(timer);
      delete queuedTimersRef.current[chatId];
    }
  }, []);

  const defaultModel = useMemo(() => {
    const preferred = models.find((model) => model.isDefault) ?? models[0];
    return preferred ? preferred.model || preferred.id : undefined;
  }, [models]);

  const baseSettings = useMemo<ChatSettings>(
    () => ({
      model: defaultModel,
      effort: "auto",
      permission: "ask",
      agentMode: "code",
      attachedFolder: undefined,
    }),
    [defaultModel],
  );

  const resolveSettings = useCallback(
    (chatId: string | null) => {
      if (!chatId) return baseSettings;
      const current = settingsByChatId[chatId];
      return {
        model: current?.model ?? baseSettings.model,
        effort: current?.effort ?? baseSettings.effort,
        permission: current?.permission ?? baseSettings.permission,
        agentMode: current?.agentMode ?? baseSettings.agentMode,
        attachedFolder: current?.attachedFolder ?? baseSettings.attachedFolder,
      };
    },
    [baseSettings, settingsByChatId],
  );

  const updateSettings = useCallback(
    (chatId: string, updates: Partial<ChatSettings>) => {
      const current = settingsRef.current[chatId] ?? {};
      const nextEntry = { ...current, ...updates };
      const next = { ...settingsRef.current, [chatId]: nextEntry };
      settingsRef.current = next;
      saveSettings(namespace, next);
      setSettingsByChatId(next);
    },
    [namespace],
  );

  const applyServerSettings = useCallback(
    (chatId: string, raw: unknown) => {
      const normalized = normalizeChatSettings(raw);
      if (!normalized) return;
      updateSettings(chatId, normalized);
    },
    [updateSettings],
  );

  const activeSettings = useMemo(
    () => resolveSettings(activeChatId),
    [activeChatId, resolveSettings],
  );

  const activeTokenUsage = useMemo(() => {
    if (!activeChatId) return undefined;
    return tokenUsageByChatId[activeChatId];
  }, [activeChatId, tokenUsageByChatId]);
  const supportsCollaboration = collaborationModes.length > 0;
  const buildCollaborationPayload = useCallback(
    (settings: ChatSettings) => {
      return buildCollaborationPayloadShared({
        settings: {
          agentMode: settings.agentMode,
          model: settings.model,
          effort: settings.effort,
        },
        collaborationModes,
        defaultModel,
      });
    },
    [collaborationModes, defaultModel],
  );

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
    await hydrateThreadImpl({
      chatId,
      threadId,
      call,
      overrides: overridesRef.current,
      setThreads,
      applySettings: applyServerSettings,
    });
  }, [applyServerSettings, call]);

  const loadChats = useCallback(async () => {
    await loadChatsImpl({
      enabled,
      status,
      call,
      overrides: overridesRef.current,
      applyOverrides,
      applySettings: applyServerSettings,
      setThreads,
      setError,
      hydrate: (chatId, threadId) => void hydrateThread(chatId, threadId),
    });
  }, [applyOverrides, applyServerSettings, call, enabled, hydrateThread, status]);

  const refreshModels = useCallback(async () => {
    if (!enabled || status !== "connected") return;
    try {
      const res = await call("chat.model.list");
      const next = normalizeModelOptions(res);
      setModels(next);
    } catch {
      setModels([]);
    }
  }, [call, enabled, status]);

  const refreshCollaborationModes = useCallback(async () => {
    if (!enabled || status !== "connected") return;
    try {
      const res = await call("chat.collaboration.mode.list");
      const next = normalizeCollaborationModes(res);
      setCollaborationModes(next);
    } catch {
      setCollaborationModes([]);
    }
  }, [call, enabled, status]);

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

  const refreshSkills = useCallback(async () => {
    if (!enabled || status !== "connected") return;
    try {
      const res = await call("chat.skills.list");
      setSkills(normalizeSkillOptions(res));
    } catch {
      setSkills([]);
    }
  }, [call, enabled, status]);

  const refreshWebTools = useCallback(async () => {
    if (!enabled || status !== "connected") return;
    try {
      const res = await call("chat.tools.list", { channel: "web" });
      setEnabledWebTools(normalizeEnabledWebTools(res));
      setWebToolsAvailable(true);
    } catch {
      setEnabledWebTools([]);
      setWebToolsAvailable(false);
    }
  }, [call, enabled, status]);

  useEffect(() => {
    if (status === "connected" && enabled) return;
    bootstrappedRef.current = false;
    setEnabledWebTools([]);
    setWebToolsAvailable(false);
    setTimeout(() => {
      setThreads([]);
      setActiveChatId(null);
      setActiveThread(null);
    }, 0);
  }, [status, enabled]);

  useEffect(() => {
    if (!enabled || status !== "connected") return;
    if (bootstrappedRef.current) return;
    bootstrappedRef.current = true;
    const timer = setTimeout(() => {
      refreshLocalSettings();
      void refreshAccount();
      void loadChats();
      void refreshModels();
      void refreshCollaborationModes();
      void refreshSkills();
      void refreshWebTools();
    }, 0);
    return () => clearTimeout(timer);
  }, [
    enabled,
    status,
    refreshLocalSettings,
    refreshAccount,
    loadChats,
    refreshModels,
    refreshCollaborationModes,
    refreshSkills,
    refreshWebTools,
  ]);

  useEffect(() => {
    if (!activeThread?.chatId) return;
    if (activeThread.running) return;
    const chatId = activeThread.chatId;
    const timer = setTimeout(() => {
      clearQueuedNotice(chatId);
    }, 0);
    return () => clearTimeout(timer);
  }, [activeThread, clearQueuedNotice]);

  useEffect(() => {
    if (!enabled || status !== "connected") return;
    let subscribed = false;
    void call("events.subscribe", { topic: "chat.*" })
      .then(() => { subscribed = true; })
      .catch(() => { subscribed = false; });
    const unsubscribe = onEvent((evt) => {
      handleChatEvent(evt, {
        threadIdLookupRef,
        activeChatIdRef,
        runningTurnsRef,
        messageBufferRef,
        setThreads,
        setActiveThread,
        setTokenUsageByChatId,
      });
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

  const selectChat = useCallback(
    async (chatId: string) => {
      await selectChatImpl({
        chatId,
        threads,
        call,
        overrides: overridesRef.current,
        runningTurnsRef,
        applySettings: applyServerSettings,
        setActiveChatId,
        setActiveThread,
        setThreads,
        setError,
      });
    },
    [applyServerSettings, call, threads],
  );

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
    const settings = resolveSettings(activeThread.chatId);
    const approvalPolicy = approvalPolicyForPermission(settings.permission);
    const effort = settings.effort !== "auto" ? settings.effort : undefined;
    const collaborationMode = buildCollaborationPayload(settings);
    const inject = activeThread.running;
    if (inject) {
      const chatId = activeThread.chatId;
      setQueuedNoticeByChatId((prev) => ({ ...prev, [chatId]: true }));
      const existingTimer = queuedTimersRef.current[chatId];
      if (existingTimer) clearTimeout(existingTimer);
      queuedTimersRef.current[chatId] = setTimeout(() => {
        clearQueuedNotice(chatId);
      }, 4000);
    }
    const localId = uuid();
    setActiveThread((prev) => prev ? {
      ...prev,
      items: [...prev.items, { id: localId, kind: "user", role: "user", text: trimmed, optimistic: true }],
    } : prev);
    if (!overridesRef.current[activeThread.chatId]) {
      const autoTitle = truncateText(trimmed, 42);
      updateOverrides(activeThread.chatId, autoTitle);
    }
    try {
      const res = await call("chat.message.send", {
        chat_id: activeThread.chatId,
        message: trimmed,
        model: settings.model,
        effort,
        approval_policy: approvalPolicy,
        collaboration_mode: collaborationMode ?? undefined,
        inject,
      }) as { turn_id?: string };
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
  }, [activeThread, buildCollaborationPayload, call, clearQueuedNotice, resolveSettings, updateOverrides]);

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

  const respondApproval = useCallback(async (requestId: number | string, decision: "accept" | "decline") => {
    try {
      console.debug("[chat] approval respond", { requestId, decision });
      await call("chat.approval.respond", { codex_request_id: requestId, decision });
    } catch (err: unknown) {
      console.error("[chat] approval respond failed", err);
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg || "Approval failed");
    }
  }, [call]);

  const updateAttachments = useCallback(
    async (chatId: string, folder: string | null) => {
      updateSettings(chatId, { attachedFolder: folder ?? undefined });
      try {
        await call("chat.settings.update", {
          chat_id: chatId,
          settings: { attachments: folder ? { folder } : null },
        });
      } catch (err: unknown) {
        const msg = err instanceof Error ? err.message : String(err);
        setError(msg || "Failed to update attachments");
      }
    },
    [call, updateSettings],
  );

  const searchFiles = useCallback(
    async (chatId: string, query: string, basePath?: string | null, limit = 40): Promise<FileOption[]> => {
      if (!enabled || status !== "connected") return [];
      if (!query.trim()) return [];
      try {
        console.debug("[chat] files search", { chatId, query, limit, basePath });
        const res = await call("chat.files.search", {
          chat_id: chatId,
          query,
          limit,
          base_path: basePath ?? undefined,
        });
        const normalized = normalizeFileOptions(res);
        console.debug("[chat] files search result", { count: normalized.length });
        return normalized;
      } catch {
        console.debug("[chat] files search failed");
        return [];
      }
    },
    [call, enabled, status],
  );

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

  const queuedNotice = activeThread ? queuedNoticeByChatId[activeThread.chatId] ?? false : false;

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
    models,
    collaborationModes,
    supportsCollaboration,
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
  };
}
