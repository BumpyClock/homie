import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { ConnectionStatus } from "@homie/shared";
import { handleChatEvent } from "@/hooks/chat-event-handler";
import { hydrateThread as hydrateThreadImpl, loadChats as loadChatsImpl } from "@/hooks/chat-loaders";
import { selectChat as selectChatImpl } from "@/hooks/chat-selection";
import {
  archiveChatThread,
  createChatThread,
  renameChatThread,
} from "@/hooks/chat-thread-actions";
import {
  cancelActiveTurn,
  respondToApproval,
  searchChatFiles,
  sendChatMessage,
  updateChatAttachments,
} from "@/hooks/chat-composer-actions";
import { loadOverrides, loadSettings, saveOverrides, saveSettings } from "@/hooks/chat-storage";
import { resolveChatAccountStatus } from "@/hooks/chat-account-status";
import { applyThreadOverrides, updateThreadOverrides } from "@/hooks/chat-overrides";
import {
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
  normalizeModelOptions,
  normalizeSkillOptions,
} from "@homie/shared";

interface UseChatOptions {
  status: ConnectionStatus;
  call: (method: string, params?: unknown) => Promise<unknown>;
  onEvent: (callback: (event: { topic: string; params?: unknown }) => void) => () => void;
  enabled: boolean;
  namespace: string;
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

  const activeSettings = useMemo(() => resolveSettings(activeChatId), [activeChatId, resolveSettings]);

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
    return applyThreadOverrides(list, overridesRef.current);
  }, []);

  const updateOverrides = useCallback((chatId: string, title: string | null) => {
    const next = updateThreadOverrides({
      chatId,
      title,
      overrides: overridesRef.current,
      setThreads,
      setActiveThread,
    });
    overridesRef.current = next;
    saveOverrides(namespace, next);
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
    await createChatThread({
      call,
      overrides: overridesRef.current,
      setThreads,
      setActiveChatId,
      setActiveThread,
      setError,
    });
  }, [call]);

  const sendMessage = useCallback(async (message: string) => {
    await sendChatMessage({
      activeThread,
      message,
      call,
      resolveSettings,
      buildCollaborationPayload,
      overrides: overridesRef.current,
      updateOverrides,
      runningTurnsRef,
      queuedTimersRef,
      clearQueuedNotice,
      setQueuedNoticeByChatId,
      setActiveThread,
      setThreads,
      setError,
    });
  }, [activeThread, buildCollaborationPayload, call, clearQueuedNotice, resolveSettings, updateOverrides]);

  const cancelActive = useCallback(async () => {
    await cancelActiveTurn({ activeThread, call });
  }, [activeThread, call]);

  const archiveChat = useCallback(async (chatId: string) => {
    await archiveChatThread({
      chatId,
      threads,
      call,
      activeChatIdRef,
      setThreads,
      setActiveChatId,
      setActiveThread,
      setError,
    });
  }, [call, threads]);

  const renameChat = useCallback(async (chatId: string, title: string | null) => {
    await renameChatThread({ chatId, title, call, updateOverrides });
  }, [call, updateOverrides]);

  const respondApproval = useCallback(async (requestId: number | string, decision: "accept" | "decline") => {
    await respondToApproval({ requestId, decision, call, setError });
  }, [call]);

  const updateAttachments = useCallback(
    async (chatId: string, folder: string | null) => {
      await updateChatAttachments({
        chatId,
        folder,
        call,
        updateSettings,
        setError,
      });
    },
    [call, updateSettings],
  );

  const searchFiles = useCallback(
    async (chatId: string, query: string, basePath?: string | null, limit = 40): Promise<FileOption[]> => {
      return searchChatFiles({
        chatId,
        query,
        basePath,
        limit,
        enabled,
        status,
        call,
      });
    },
    [call, enabled, status],
  );

  const clearError = useCallback(() => setError(null), []);

  const accountStatus = useMemo(() => resolveChatAccountStatus(account), [account]);

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
