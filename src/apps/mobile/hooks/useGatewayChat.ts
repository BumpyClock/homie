import {
  buildChatThreadSummaries,
  type ChatApprovalDecision,
  createChatClient,
  deriveTitleFromThread,
  itemsFromThread,
  mapChatEvent,
  type SessionInfo,
  subscribeToChatEvents,
  type ChatThreadSummary,
  type ConnectionStatus,
  type GatewayTransport,
  type RpcEvent,
} from '@homie/shared';
import { useCallback, useEffect, useRef, useState } from 'react';

import { runtimeConfig } from '@/config/runtime';
import {
  applyApprovalDecisionToThread,
  applyApprovalStatusToThread,
  applyMappedEventToThread,
  fallbackThreadTitle,
  formatError,
  pendingApprovalFromThread,
  previewFromItems,
  sortThreads,
  statusBadgeFor,
  threadLastActivityAt,
  type ActiveMobileThread,
  type PendingApprovalMetadata,
  type StatusBadgeState,
} from '@/hooks/gateway-chat-utils';
import { createMobileGatewayClient } from '@/lib/gateway-client';

export interface UseGatewayChatResult {
  status: ConnectionStatus;
  statusBadge: StatusBadgeState;
  gatewayUrl: string;
  threads: ChatThreadSummary[];
  activeChatId: string | null;
  activeThread: ActiveMobileThread | null;
  error: string | null;
  loadingThreads: boolean;
  loadingMessages: boolean;
  creatingChat: boolean;
  sendingMessage: boolean;
  loadingTerminals: boolean;
  pendingApproval: PendingApprovalMetadata | null;
  terminalSessions: SessionInfo[];
  selectThread: (chatId: string) => void;
  refreshThreads: () => Promise<void>;
  refreshTerminals: () => Promise<void>;
  createChat: () => Promise<void>;
  sendMessage: (message: string) => Promise<void>;
  renameThread: (chatId: string, title: string) => Promise<void>;
  archiveThread: (chatId: string) => Promise<void>;
  respondApproval: (requestId: number | string, decision: ChatApprovalDecision) => Promise<void>;
}

function normalizeTerminalSessions(raw: unknown): SessionInfo[] {
  if (!raw || typeof raw !== 'object') return [];
  const sessions = (raw as { sessions?: unknown }).sessions;
  if (!Array.isArray(sessions)) return [];
  return sessions
    .map((entry): SessionInfo | null => {
      if (!entry || typeof entry !== 'object') return null;
      const record = entry as Record<string, unknown>;
      const session_id = typeof record.session_id === 'string' ? record.session_id : '';
      const shell = typeof record.shell === 'string' ? record.shell : '';
      const cols = typeof record.cols === 'number' ? record.cols : 0;
      const rows = typeof record.rows === 'number' ? record.rows : 0;
      const started_at = typeof record.started_at === 'string' ? record.started_at : '';
      const status = typeof record.status === 'string' ? record.status : 'inactive';
      if (!session_id || !shell || !started_at) return null;
      return {
        session_id,
        name: typeof record.name === 'string' ? record.name : null,
        shell,
        cols,
        rows,
        started_at,
        status: status === 'active' || status === 'exited' || status === 'inactive' ? status : 'inactive',
        exit_code: typeof record.exit_code === 'number' ? record.exit_code : undefined,
      };
    })
    .filter((entry): entry is SessionInfo => entry !== null);
}

export function useGatewayChat(
  gatewayUrl = runtimeConfig.gatewayUrl,
): UseGatewayChatResult {
  const [status, setStatus] = useState<ConnectionStatus>('disconnected');
  const [threads, setThreads] = useState<ChatThreadSummary[]>([]);
  const [activeChatId, setActiveChatId] = useState<string | null>(null);
  const [activeThreadState, setActiveThreadState] =
    useState<ActiveMobileThread | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [loadingThreads, setLoadingThreads] = useState(false);
  const [loadingMessages, setLoadingMessages] = useState(false);
  const [creatingChat, setCreatingChat] = useState(false);
  const [sendingMessage, setSendingMessage] = useState(false);
  const [terminalSessions, setTerminalSessions] = useState<SessionInfo[]>([]);
  const [loadingTerminals, setLoadingTerminals] = useState(false);

  const transportRef = useRef<GatewayTransport | null>(null);
  const chatClientRef = useRef<ReturnType<typeof createChatClient> | null>(null);
  const activeThreadRef = useRef<ActiveMobileThread | null>(null);
  const threadsRef = useRef<ChatThreadSummary[]>([]);
  const threadIdLookupRef = useRef<Map<string, string>>(new Map());
  const messageBufferRef = useRef<Map<string, string>>(new Map());
  const bootstrappedRef = useRef(false);

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
    setThreads([]);
    setActiveChatId(null);
    setActiveThread(null);
    setError(null);
    messageBufferRef.current.clear();
  }, [gatewayUrl, setActiveThread]);

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

      setLoadingMessages(true);
      try {
        const response = await chatClient.readThread(chatId, threadId, true);
        const threadRecord = response.thread;
        const fallback = fallbackThreadTitle(chatId);
        const title = threadRecord ? deriveTitleFromThread(threadRecord, fallback) : fallback;
        const items = threadRecord ? itemsFromThread(threadRecord) : [];
        const previous = threadsRef.current.find((entry) => entry.chatId === chatId);
        const running = previous?.running ?? false;
        const nextThread: ActiveMobileThread = {
          chatId,
          threadId,
          title,
          items,
          running,
        };
        setActiveThread(nextThread);
        updateThreadSummaryFromActive(
          nextThread,
          threadLastActivityAt(threadRecord, previous?.lastActivityAt ?? Date.now()),
        );
        setError(null);
      } catch (nextError) {
        setError(formatError(nextError));
      } finally {
        setLoadingMessages(false);
      }
    },
    [setActiveThread, updateThreadSummaryFromActive],
  );

  const hydrateThread = useCallback(async (chatId: string, threadId: string) => {
    const chatClient = chatClientRef.current;
    if (!chatClient) return;

    try {
      const response = await chatClient.readThread(chatId, threadId, true);
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
            return {
              ...entry,
              title,
              preview,
              lastActivityAt: activityAt,
            };
          }),
        ),
      );

      const currentActive = activeThreadRef.current;
      if (currentActive && currentActive.chatId === chatId && currentActive.items.length === 0) {
        setActiveThread({
          ...currentActive,
          threadId,
          title,
          items,
        });
      }
    } catch {
      return;
    }
  }, [setActiveThread]);

  const refreshThreads = useCallback(async () => {
    const chatClient = chatClientRef.current;
    if (!chatClient) return;

    setLoadingThreads(true);
    try {
      const records = await chatClient.list();
      const nextThreads = sortThreads(buildChatThreadSummaries(records));
      setThreads(nextThreads);
      setError(null);
      for (const thread of nextThreads) {
        void hydrateThread(thread.chatId, thread.threadId);
      }
    } catch (nextError) {
      setError(formatError(nextError));
    } finally {
      setLoadingThreads(false);
    }
  }, [hydrateThread]);

  const refreshTerminals = useCallback(async () => {
    const transport = transportRef.current;
    if (!transport || status !== 'connected') {
      setTerminalSessions([]);
      return;
    }

    setLoadingTerminals(true);
    try {
      const result = await transport.call<{ sessions?: unknown }>('terminal.session.list');
      setTerminalSessions(normalizeTerminalSessions(result));
      setError(null);
    } catch (nextError) {
      setError(formatError(nextError));
    } finally {
      setLoadingTerminals(false);
    }
  }, [status]);

  const handleGatewayEvent = useCallback((event: RpcEvent) => {
    const mapped = mapChatEvent(
      {
        topic: event.topic,
        params: event.params,
      },
      {
        threadIdLookup: threadIdLookupRef.current,
        messageBuffer: messageBufferRef.current,
      },
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
        return {
          ...entry,
          threadId: mapped.threadId,
          running,
          lastActivityAt: mapped.activityAt,
        };
      });
      if (matched) return sortThreads(next);
      return sortThreads([
        {
          chatId: mapped.chatId,
          threadId: mapped.threadId,
          title: fallbackThreadTitle(mapped.chatId),
          preview: '',
          status: 'active',
          lastActivityAt: mapped.activityAt,
          running: mapped.type === 'turn.started',
        },
        ...next,
      ]);
    });

    const active = activeThreadRef.current;
    if (!active || active.chatId !== mapped.chatId) return;
    const nextActive = applyMappedEventToThread(active, mapped);
    setActiveThread(nextActive);
    updateThreadSummaryFromActive(nextActive, mapped.activityAt);
  }, [setActiveThread, updateThreadSummaryFromActive]);

  useEffect(() => {
    const transport = createMobileGatewayClient({
      url: gatewayUrl,
    });
    const chatClient = createChatClient(transport);
    transportRef.current = transport;
    chatClientRef.current = chatClient;

    const unsubscribeState = transport.onStateChange((nextState) => {
      setStatus(nextState.status);
      if (nextState.error) setError(formatError(nextState.error));
    });
    const unsubscribeEvent = transport.onEvent((event) => {
      handleGatewayEvent(event);
    });

    transport.start();

    return () => {
      unsubscribeEvent();
      unsubscribeState();
      transport.stop();
      bootstrappedRef.current = false;
      transportRef.current = null;
      chatClientRef.current = null;
      setStatus('disconnected');
    };
  }, [gatewayUrl, handleGatewayEvent]);

  useEffect(() => {
    if (status !== 'connected') {
      bootstrappedRef.current = false;
      setTerminalSessions([]);
      return;
    }
    if (bootstrappedRef.current) return;
    bootstrappedRef.current = true;

    const transport = transportRef.current;
    if (!transport) return;
    void subscribeToChatEvents(transport.call.bind(transport), 'chat.*').catch((nextError) => {
      setError(formatError(nextError));
    });
    void refreshThreads();
    void refreshTerminals();
  }, [refreshTerminals, refreshThreads, status]);

  useEffect(() => {
    if (threads.length === 0) {
      if (activeChatId !== null) setActiveChatId(null);
      if (activeThreadRef.current) setActiveThread(null);
      return;
    }

    const activeExists = activeChatId
      ? threads.some((entry) => entry.chatId === activeChatId)
      : false;
    if (activeExists) return;

    const firstThread = threads[0];
    setActiveChatId(firstThread.chatId);
    void loadThread(firstThread.chatId, firstThread.threadId);
  }, [activeChatId, loadThread, setActiveThread, threads]);

  const selectThread = useCallback((chatId: string) => {
    const thread = threadsRef.current.find((entry) => entry.chatId === chatId);
    if (!thread) return;
    setActiveChatId(chatId);
    void loadThread(thread.chatId, thread.threadId);
  }, [loadThread]);

  const createChat = useCallback(async () => {
    const chatClient = chatClientRef.current;
    if (!chatClient) return;

    setCreatingChat(true);
    try {
      const created = await chatClient.create();
      if (!created.chatId || !created.threadId) {
        throw new Error('Gateway returned an invalid chat reference');
      }
      const createdThread: ChatThreadSummary = {
        chatId: created.chatId,
        threadId: created.threadId,
        title: fallbackThreadTitle(created.chatId),
        preview: '',
        status: 'active',
        lastActivityAt: Date.now(),
        running: false,
      };
      setThreads((current) =>
        sortThreads([
          createdThread,
          ...current.filter((entry) => entry.chatId !== created.chatId),
        ]),
      );
      setActiveChatId(created.chatId);
      await loadThread(created.chatId, created.threadId);
      setError(null);
    } catch (nextError) {
      setError(formatError(nextError));
    } finally {
      setCreatingChat(false);
    }
  }, [loadThread]);

  const sendMessage = useCallback(async (message: string) => {
    const chatClient = chatClientRef.current;
    const active = activeThreadRef.current;
    const trimmed = message.trim();
    if (!chatClient || !active || !trimmed) return;

    setSendingMessage(true);
    const optimistic = {
      ...active,
      running: true,
    };
    setActiveThread(optimistic);
    updateThreadSummaryFromActive(optimistic, Date.now());

    try {
      await chatClient.sendMessage({
        chatId: active.chatId,
        message: trimmed,
      });
      setError(null);
    } catch (nextError) {
      const current = activeThreadRef.current;
      if (current && current.chatId === active.chatId) {
        const stopped = {
          ...current,
          running: false,
        };
        setActiveThread(stopped);
        updateThreadSummaryFromActive(stopped, Date.now());
      }
      setError(formatError(nextError));
      throw nextError;
    } finally {
      setSendingMessage(false);
    }
  }, [setActiveThread, updateThreadSummaryFromActive]);

  const renameThread = useCallback(async (chatId: string, title: string) => {
    const chatClient = chatClientRef.current;
    if (!chatClient) return;

    const nextTitle = title.trim();
    if (!nextTitle) return;

    try {
      await chatClient.renameThread({ chatId, title: nextTitle });
      setThreads((current) =>
        current.map((entry) => (entry.chatId === chatId ? { ...entry, title: nextTitle } : entry)),
      );
      const active = activeThreadRef.current;
      if (active?.chatId === chatId) {
        setActiveThread({
          ...active,
          title: nextTitle,
        });
      }
      setError(null);
    } catch (nextError) {
      setError(formatError(nextError));
      throw nextError;
    }
  }, [setActiveThread]);

  const archiveThread = useCallback(async (chatId: string) => {
    const chatClient = chatClientRef.current;
    const summary = threadsRef.current.find((entry) => entry.chatId === chatId);
    if (!chatClient || !summary) return;

    try {
      await chatClient.archiveThread({
        chatId,
        threadId: summary.threadId,
      });

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
      setError(null);
    } catch (nextError) {
      setError(formatError(nextError));
      throw nextError;
    }
  }, [loadThread, setActiveThread]);

  const respondApproval = useCallback(
    async (requestId: number | string, decision: ChatApprovalDecision) => {
      const chatClient = chatClientRef.current;
      const active = activeThreadRef.current;
      if (!chatClient || !active) return;

      const optimistic = applyApprovalDecisionToThread(active, requestId, decision);
      if (optimistic !== active) {
        setActiveThread(optimistic);
        updateThreadSummaryFromActive(optimistic, Date.now());
      }

      try {
        await chatClient.respondApproval({ requestId, decision });
        setError(null);
      } catch (nextError) {
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
    [setActiveThread, updateThreadSummaryFromActive],
  );

  const pendingApproval = pendingApprovalFromThread(activeThreadState);

  return {
    status,
    statusBadge: statusBadgeFor(status),
    gatewayUrl,
    threads,
    activeChatId,
    activeThread: activeThreadState,
    error,
    loadingThreads,
    loadingMessages,
    creatingChat,
    sendingMessage,
    loadingTerminals,
    pendingApproval,
    terminalSessions,
    selectThread,
    refreshThreads,
    refreshTerminals,
    createChat,
    sendMessage,
    renameThread,
    archiveThread,
    respondApproval,
  };
}
