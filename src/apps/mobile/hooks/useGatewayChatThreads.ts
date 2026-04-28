import {
  buildChatThreadSummaries,
  deriveTitleFromThread,
  itemsFromThread,
  mapChatEvent,
  type ChatApprovalDecision,
  type ChatThreadSummary,
  type RpcEvent,
  type createChatClient,
} from '@homie/shared';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { useCallback, useEffect, useRef, useState, type MutableRefObject } from 'react';
import {
  applyApprovalDecisionToThread,
  applyApprovalStatusToThread,
  applyMappedEventToThread,
  countPendingApprovals,
  fallbackThreadTitle,
  formatError,
  pendingApprovalFromThread,
  previewFromItems,
  sortThreads,
  threadLastActivityAt,
  type ActiveMobileThread,
} from '@/hooks/gateway-chat-utils';
import type { ChatMessagePreferences } from '@/hooks/useGatewayProviderAuth';
type ChatClient = ReturnType<typeof createChatClient>;

interface GatewayTargetSnapshot {
  gatewayUrl: string;
  generation: number;
}

interface QueuedChatMessage extends GatewayTargetSnapshot {
  chatId: string;
  threadId: string;
  message: string;
}

interface UseGatewayChatThreadsOptions {
  gatewayUrl: string;
  chatClientRef: MutableRefObject<ChatClient | null>;
  setError: (error: string | null) => void;
  getMessagePreferences: () => ChatMessagePreferences;
}
const LAST_ACTIVE_CHAT_KEY_PREFIX = 'homie.mobile.last_active_chat';
const THREAD_CACHE_KEY_PREFIX = 'homie.mobile.thread_cache';
const THREAD_CACHE_MAX = 50;
function storageKeyForGatewayTarget(gatewayUrl: string): string | null {
  const normalized = gatewayUrl.trim();
  if (!normalized) return null;
  return `${LAST_ACTIVE_CHAT_KEY_PREFIX}:${encodeURIComponent(normalized)}`;
}
function normalizeStoredChatId(raw: string | null): string | null {
  if (!raw) return null;
  const normalized = raw.trim();
  return normalized.length > 0 ? normalized : null;
}
function threadCacheKey(gatewayUrl: string): string {
  return `${THREAD_CACHE_KEY_PREFIX}:${encodeURIComponent(gatewayUrl.trim())}`;
}
async function saveThreadCache(
  gatewayUrl: string,
  threads: ChatThreadSummary[],
): Promise<void> {
  try {
    const bounded = threads.slice(0, THREAD_CACHE_MAX);
    await AsyncStorage.setItem(threadCacheKey(gatewayUrl), JSON.stringify(bounded));
  } catch {
    return;
  }
}
async function loadThreadCache(gatewayUrl: string): Promise<ChatThreadSummary[] | null> {
  try {
    const raw = await AsyncStorage.getItem(threadCacheKey(gatewayUrl));
    if (!raw) return null;
    const parsed = JSON.parse(raw);
    if (!Array.isArray(parsed)) return null;
    return parsed as ChatThreadSummary[];
  } catch {
    return null;
  }
}
export function useGatewayChatThreads({
  gatewayUrl,
  chatClientRef,
  setError,
  getMessagePreferences,
}: UseGatewayChatThreadsOptions) {
  const [threads, setThreads] = useState<ChatThreadSummary[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [activeThreadState, setActiveThreadState] =
    useState<ActiveMobileThread | null>(null);
  const [loadingThreads, setLoadingThreads] = useState(false);
  const [loadingMessages, setLoadingMessages] = useState(false);
  const [creatingChat, setCreatingChat] = useState(false);
  const [sendingMessage, setSendingMessage] = useState(false);
  const [restoringSelection, setRestoringSelection] = useState(false);
  const [queuedMessage, setQueuedMessage] = useState<string | null>(null);
  const activeThreadRef = useRef<ActiveMobileThread | null>(null);
  const threadsRef = useRef<ChatThreadSummary[]>([]);
  const threadIdLookupRef = useRef<Map<string, string>>(new Map());
  const messageBufferRef = useRef<Map<string, string>>(new Map());
  const loadingThreadKeyRef = useRef<string | null>(null);
  const restoredChatIdRef = useRef<string | null>(null);
  const queuedMessageRef = useRef<QueuedChatMessage | null>(null);
  const gatewayUrlRef = useRef(gatewayUrl);
  const gatewayGenerationRef = useRef(0);

  const currentGatewayTarget = useCallback((): GatewayTargetSnapshot => ({
    gatewayUrl: gatewayUrlRef.current,
    generation: gatewayGenerationRef.current,
  }), []);

  const isCurrentGatewayTarget = useCallback((target: GatewayTargetSnapshot): boolean =>
    target.gatewayUrl === gatewayUrlRef.current &&
    target.generation === gatewayGenerationRef.current,
  []);

  useEffect(() => {
    threadsRef.current = threads;
    const nextLookup = new Map<string, string>();
    for (const thread of threads) {
      nextLookup.set(thread.threadId, thread.chatId);
    }
    threadIdLookupRef.current = nextLookup;
  }, [threads]);
  const setActiveThread = useCallback((next: ActiveMobileThread | null) => {
    activeThreadRef.current = next;
    setActiveThreadState(next);
  }, []);
  useEffect(() => {
    let cancelled = false;
    gatewayUrlRef.current = gatewayUrl;
    gatewayGenerationRef.current += 1;
    setThreads([]);
    setActiveChatId(null);
    setActiveThread(null);
    setError(null);
    setLoadingThreads(false);
    setLoadingMessages(false);
    setCreatingChat(false);
    setSendingMessage(false);
    messageBufferRef.current.clear();
    loadingThreadKeyRef.current = null;
    restoredChatIdRef.current = null;
    queuedMessageRef.current = null;
    setQueuedMessage(null);
    const storageKey = storageKeyForGatewayTarget(gatewayUrl);
    if (!storageKey) {
      setRestoringSelection(false);
      return;
    }
    void loadThreadCache(gatewayUrl).then((cached) => {
      if (cancelled || !cached || cached.length === 0) return;
      setThreads((current) => (current.length === 0 ? cached : current));
    });
    setRestoringSelection(true);
    void AsyncStorage.getItem(storageKey)
      .then((stored) => {
        if (cancelled) return;
        restoredChatIdRef.current = normalizeStoredChatId(stored);
      })
      .catch(() => {
        if (cancelled) return;
        restoredChatIdRef.current = null;
      })
      .finally(() => {
        if (cancelled) return;
        setRestoringSelection(false);
      });
    return () => {
      cancelled = true;
    };
  }, [gatewayUrl, setActiveThread, setError]);
  useEffect(() => {
    const storageKey = storageKeyForGatewayTarget(gatewayUrl);
    if (!storageKey || !activeChatId) return;
    restoredChatIdRef.current = activeChatId;
    void AsyncStorage.setItem(storageKey, activeChatId).catch(() => {
      return;
    });
  }, [activeChatId, gatewayUrl]);
  const updateThreadSummaryFromActive = useCallback(
    (thread: ActiveMobileThread, activityAt?: number) => {
      const preview = previewFromItems(thread.items);
      setThreads((current) =>
        sortThreads(
          current.map((entry) => {
            if (entry.chatId !== thread.chatId) return entry;
            return {
              ...entry,
              threadId: thread.threadId,
              title: thread.title,
              preview,
              running: thread.running,
              lastActivityAt: activityAt ?? entry.lastActivityAt,
            };
          }),
        ),
      );
    },
    [],
  );
  const loadThread = useCallback(
    async (chatId: string, threadId: string) => {
      const chatClient = chatClientRef.current;
      if (!chatClient) return;
      const target = currentGatewayTarget();
      const loadKey = `${chatId}:${threadId}`;
      if (loadingThreadKeyRef.current === loadKey) return;
      loadingThreadKeyRef.current = loadKey;
      setLoadingMessages(true);
      try {
        const response = await chatClient.readThread(chatId, threadId, true);
        if (!isCurrentGatewayTarget(target)) return;
        const threadRecord = response.thread;
        const fallback = fallbackThreadTitle(chatId);
        const title = threadRecord ? deriveTitleFromThread(threadRecord, fallback) : fallback;
        const items = threadRecord ? itemsFromThread(threadRecord) : [];
        const previous = threadsRef.current.find((entry) => entry.chatId === chatId);
        const running = previous?.running ?? false;
        const nextThread: ActiveMobileThread = { chatId, threadId, title, items, running };
        setActiveThread(nextThread);
        updateThreadSummaryFromActive(
          nextThread,
          threadLastActivityAt(threadRecord, previous?.lastActivityAt ?? Date.now()),
        );
        setError(null);
      } catch (nextError) {
        if (isCurrentGatewayTarget(target)) {
          setError(formatError(nextError));
        }
      } finally {
        if (isCurrentGatewayTarget(target) && loadingThreadKeyRef.current === loadKey) {
          loadingThreadKeyRef.current = null;
          setLoadingMessages(false);
        }
      }
    },
    [
      chatClientRef, currentGatewayTarget, isCurrentGatewayTarget, setActiveThread, setError,
      updateThreadSummaryFromActive,
    ],
  );
  const hydrateThread = useCallback(async (chatId: string, threadId: string) => {
    const chatClient = chatClientRef.current;
    if (!chatClient) return;
    const target = currentGatewayTarget();
    try {
      const response = await chatClient.readThread(chatId, threadId, true);
      if (!isCurrentGatewayTarget(target)) return;
      const threadRecord = response.thread;
      if (!threadRecord) return;
      const fallback = fallbackThreadTitle(chatId);
      const items = itemsFromThread(threadRecord);
      const title = deriveTitleFromThread(threadRecord, fallback);
      const preview = previewFromItems(items);
      const existing = threadsRef.current.find((entry) => entry.chatId === chatId);
      const activityAt = threadLastActivityAt(
        threadRecord,
        existing?.lastActivityAt ?? Date.now(),
      );
      setThreads((current) =>
        sortThreads(
          current.map((entry) => {
            if (entry.chatId !== chatId) return entry;
            return { ...entry, title, preview, lastActivityAt: activityAt };
          }),
        ),
      );
      const currentActive = activeThreadRef.current;
      if (currentActive && currentActive.chatId === chatId) {
        setActiveThread({ ...currentActive, threadId, title, items });
      }
    } catch {
      return;
    }
  }, [chatClientRef, currentGatewayTarget, isCurrentGatewayTarget, setActiveThread]);
  const refreshThreads = useCallback(async () => {
    const chatClient = chatClientRef.current;
    if (!chatClient) return;
    const target = currentGatewayTarget();
    setLoadingThreads(true);
    try {
      const records = await chatClient.list();
      if (!isCurrentGatewayTarget(target)) return;
      const nextThreads = sortThreads(buildChatThreadSummaries(records));
      setThreads(nextThreads);
      setError(null);
      void saveThreadCache(gatewayUrl, nextThreads);
      for (const thread of nextThreads) {
        void hydrateThread(thread.chatId, thread.threadId);
      }
    } catch (nextError) {
      if (isCurrentGatewayTarget(target)) {
        setError(formatError(nextError));
      }
    } finally {
      if (isCurrentGatewayTarget(target)) {
        setLoadingThreads(false);
      }
    }
  }, [chatClientRef, currentGatewayTarget, gatewayUrl, hydrateThread, isCurrentGatewayTarget, setError]);
  const sendMessageInternal = useCallback(async (trimmed: string, target?: QueuedChatMessage) => {
    const chatClient = chatClientRef.current;
    const active = activeThreadRef.current;
    const messageTarget = target ?? (active ? {
      ...currentGatewayTarget(),
      chatId: active.chatId,
      threadId: active.threadId,
      message: trimmed,
    } : null);
    if (!chatClient || !messageTarget || !trimmed || !isCurrentGatewayTarget(messageTarget)) return;
    const activeMatchesTarget =
      active?.chatId === messageTarget.chatId &&
      active.threadId === messageTarget.threadId;
    setSendingMessage(true);
    if (active && activeMatchesTarget) {
      const optimistic = { ...active, running: true };
      setActiveThread(optimistic);
      updateThreadSummaryFromActive(optimistic, Date.now());
    }
    try {
      const prefs = getMessagePreferences();
      await chatClient.sendMessage({
        chatId: messageTarget.chatId,
        message: trimmed,
        model: prefs.model,
        effort: prefs.effort,
      });
      if (!isCurrentGatewayTarget(messageTarget)) return;
      setError(null);
    } catch (nextError) {
      if (!isCurrentGatewayTarget(messageTarget)) return;
      const current = activeThreadRef.current;
      if (
        current &&
        current.chatId === messageTarget.chatId &&
        current.threadId === messageTarget.threadId
      ) {
        const stopped = { ...current, running: false };
        setActiveThread(stopped);
        updateThreadSummaryFromActive(stopped, Date.now());
      }
      setError(formatError(nextError));
      throw nextError;
    } finally {
      if (isCurrentGatewayTarget(messageTarget)) {
        setSendingMessage(false);
      }
    }
  }, [
    chatClientRef, currentGatewayTarget, getMessagePreferences, isCurrentGatewayTarget,
    setActiveThread, setError, updateThreadSummaryFromActive,
  ]);
  const handleGatewayEvent = useCallback((event: RpcEvent) => {
    const mapped = mapChatEvent(
      { topic: event.topic, params: event.params },
      { threadIdLookup: threadIdLookupRef.current, messageBuffer: messageBufferRef.current },
    );
    if (!mapped) return;
    setThreads((current) => {
      let matched = false;
      const next = current.map((entry) => {
        if (entry.chatId !== mapped.chatId) return entry;
        matched = true;
        let running = entry.running;
        if (mapped.type === 'turn.started') running = true;
        if (mapped.type === 'turn.completed') running = false;
        return { ...entry, threadId: mapped.threadId, running, lastActivityAt: mapped.activityAt };
      });
      if (matched) return sortThreads(next);
      return sortThreads([
        {
          chatId: mapped.chatId, threadId: mapped.threadId, title: fallbackThreadTitle(mapped.chatId),
          preview: '', status: 'active', lastActivityAt: mapped.activityAt, running: mapped.type === 'turn.started',
        },
        ...next,
      ]);
    });
    if (mapped.type === 'turn.completed') {
      const queued = queuedMessageRef.current;
      if (
        queued &&
        queued.chatId === mapped.chatId &&
        queued.threadId === mapped.threadId &&
        isCurrentGatewayTarget(queued)
      ) {
        queuedMessageRef.current = null;
        setQueuedMessage(null);
        setTimeout(() => {
          void sendMessageInternal(queued.message, queued);
        }, 100);
      }
    }
    const active = activeThreadRef.current;
    if (!active || active.chatId !== mapped.chatId) return;
    const nextActive = applyMappedEventToThread(active, mapped);
    setActiveThread(nextActive);
    updateThreadSummaryFromActive(nextActive, mapped.activityAt);
  }, [isCurrentGatewayTarget, sendMessageInternal, setActiveThread, updateThreadSummaryFromActive]);
  useEffect(() => {
    if (restoringSelection) return;
    if (threads.length === 0) {
      if (activeChatId !== null) setActiveChatId(null);
      if (activeThreadRef.current) setActiveThread(null);
      return;
    }
    const activeThreadSummary = activeChatId
      ? threads.find((entry) => entry.chatId === activeChatId)
      : undefined;
    if (activeThreadSummary) {
      const activeLoaded =
        activeThreadRef.current?.chatId === activeThreadSummary.chatId &&
        activeThreadRef.current?.threadId === activeThreadSummary.threadId;
      if (!activeLoaded) {
        void loadThread(activeThreadSummary.chatId, activeThreadSummary.threadId);
      }
      return;
    }
    const restoredChatId = restoredChatIdRef.current;
    const restoredThread = restoredChatId
      ? threads.find((entry) => entry.chatId === restoredChatId)
      : undefined;
    const nextThread = restoredThread ?? threads[0];
    if (!nextThread) return;
    setActiveChatId(nextThread.chatId);
    void loadThread(nextThread.chatId, nextThread.threadId);
  }, [activeChatId, loadThread, restoringSelection, setActiveThread, threads]);
  const selectThread = useCallback((chatId: string) => {
    const thread = threadsRef.current.find((entry) => entry.chatId === chatId);
    if (!thread) return;
    setActiveChatId(chatId);
    void loadThread(thread.chatId, thread.threadId);
  }, [loadThread]);
  const createChat = useCallback(async () => {
    const chatClient = chatClientRef.current;
    if (!chatClient) return;
    const target = currentGatewayTarget();
    setCreatingChat(true);
    try {
      const created = await chatClient.create();
      if (!isCurrentGatewayTarget(target)) return;
      if (!created.chatId || !created.threadId) {
        throw new Error('Gateway returned an invalid chat reference');
      }
      const createdThread: ChatThreadSummary = {
        chatId: created.chatId, threadId: created.threadId, title: fallbackThreadTitle(created.chatId),
        preview: '', status: 'active', lastActivityAt: Date.now(), running: false,
      };
      setThreads((current) =>
        sortThreads([createdThread, ...current.filter((entry) => entry.chatId !== created.chatId)]),
      );
      setActiveChatId(created.chatId);
      await loadThread(created.chatId, created.threadId);
      if (!isCurrentGatewayTarget(target)) return;
      setError(null);
    } catch (nextError) {
      if (isCurrentGatewayTarget(target)) {
        setError(formatError(nextError));
      }
    } finally {
      if (isCurrentGatewayTarget(target)) {
        setCreatingChat(false);
      }
    }
  }, [chatClientRef, currentGatewayTarget, isCurrentGatewayTarget, loadThread, setError]);
  const sendMessage = useCallback(async (message: string) => {
    const active = activeThreadRef.current;
    const trimmed = message.trim();
    if (!chatClientRef.current || !active || !trimmed) return;
    if (active.running) {
      queuedMessageRef.current = {
        ...currentGatewayTarget(),
        chatId: active.chatId,
        threadId: active.threadId,
        message: trimmed,
      };
      setQueuedMessage(trimmed);
      return;
    }
    await sendMessageInternal(trimmed);
  }, [chatClientRef, currentGatewayTarget, sendMessageInternal]);
  const clearQueuedMessage = useCallback(() => {
    queuedMessageRef.current = null;
    setQueuedMessage(null);
  }, []);
  const stopChat = useCallback(async () => {
    const chatClient = chatClientRef.current;
    const active = activeThreadRef.current;
    if (!chatClient || !active || !active.activeTurnId) return;
    const target = currentGatewayTarget();
    try {
      await chatClient.cancel({ chatId: active.chatId, turnId: active.activeTurnId });
    } catch (nextError) {
      if (isCurrentGatewayTarget(target)) {
        setError(formatError(nextError));
      }
    }
  }, [chatClientRef, currentGatewayTarget, isCurrentGatewayTarget, setError]);
  const renameThread = useCallback(async (chatId: string, title: string) => {
    const chatClient = chatClientRef.current;
    if (!chatClient) return;
    const nextTitle = title.trim();
    if (!nextTitle) return;
    const target = currentGatewayTarget();
    try {
      await chatClient.renameThread({ chatId, title: nextTitle });
      if (!isCurrentGatewayTarget(target)) return;
      setThreads((current) =>
        current.map((entry) => (entry.chatId === chatId ? { ...entry, title: nextTitle } : entry)),
      );
      const active = activeThreadRef.current;
      if (active?.chatId === chatId) {
        setActiveThread({ ...active, title: nextTitle });
      }
      setError(null);
    } catch (nextError) {
      if (!isCurrentGatewayTarget(target)) return;
      setError(formatError(nextError));
      throw nextError;
    }
  }, [chatClientRef, currentGatewayTarget, isCurrentGatewayTarget, setActiveThread, setError]);
  const archiveThread = useCallback(async (chatId: string) => {
    const chatClient = chatClientRef.current;
    const summary = threadsRef.current.find((entry) => entry.chatId === chatId);
    if (!chatClient || !summary) return;
    const target = currentGatewayTarget();
    try {
      await chatClient.archiveThread({ chatId, threadId: summary.threadId });
      if (!isCurrentGatewayTarget(target)) return;
      setThreads((current) => sortThreads(current.filter((entry) => entry.chatId !== chatId)));
      const active = activeThreadRef.current;
      if (active?.chatId === chatId) {
        const nextThread = threadsRef.current.find((entry) => entry.chatId !== chatId) ?? null;
        if (nextThread) {
          setActiveChatId(nextThread.chatId);
          await loadThread(nextThread.chatId, nextThread.threadId);
        } else {
          setActiveChatId(null);
          setActiveThread(null);
        }
      }
      if (!isCurrentGatewayTarget(target)) return;
      setError(null);
    } catch (nextError) {
      if (!isCurrentGatewayTarget(target)) return;
      setError(formatError(nextError));
      throw nextError;
    }
  }, [
    chatClientRef, currentGatewayTarget, isCurrentGatewayTarget, loadThread, setActiveThread,
    setError,
  ]);
  const respondApproval = useCallback(
    async (requestId: number | string, decision: ChatApprovalDecision) => {
      const chatClient = chatClientRef.current;
      const active = activeThreadRef.current;
      if (!chatClient || !active) return;
      const target = currentGatewayTarget();
      const optimistic = applyApprovalDecisionToThread(active, requestId, decision);
      if (optimistic !== active) {
        setActiveThread(optimistic);
        updateThreadSummaryFromActive(optimistic, Date.now());
      }
      try {
        await chatClient.respondApproval({ requestId, decision });
        if (!isCurrentGatewayTarget(target)) return;
        setError(null);
      } catch (nextError) {
        if (!isCurrentGatewayTarget(target)) return;
        const current = activeThreadRef.current;
        if (current && current.chatId === active.chatId) {
          const rollback = applyApprovalStatusToThread(current, requestId, 'pending');
          setActiveThread(rollback);
          updateThreadSummaryFromActive(rollback, Date.now());
        }
        setError(formatError(nextError));
        throw nextError;
      }
    },
    [
      chatClientRef, currentGatewayTarget, isCurrentGatewayTarget, setActiveThread, setError,
      updateThreadSummaryFromActive,
    ],
  );
  const pendingApproval = pendingApprovalFromThread(activeThreadState);
  const activePendingApprovalCount = activeThreadState
    ? countPendingApprovals(activeThreadState.items)
    : 0;
  return {
    threads, activeChatId, activeThread: activeThreadState, loadingThreads, loadingMessages,
    creatingChat, sendingMessage, pendingApproval, activePendingApprovalCount, queuedMessage,
    selectThread, refreshThreads, createChat, sendMessage, stopChat, renameThread, archiveThread,
    respondApproval, clearQueuedMessage, handleGatewayEvent,
  };
}
