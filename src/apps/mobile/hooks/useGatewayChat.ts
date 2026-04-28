import {
  createChatClient,
  subscribeToChatEvents,
  type ChatAccountProviderStatus,
  type ChatApprovalDecision,
  type ChatDeviceCodePollResult,
  type ChatDeviceCodeSession,
  type ChatEffort,
  type ChatPermissionMode,
  type ChatThreadSummary,
  type CollaborationModeOption,
  type ConnectionStatus,
  type GatewayTransport,
  type ModelOption,
  type SessionInfo,
  type SkillOption,
  type TmuxSessionInfo,
} from '@homie/shared';
import { useEffect, useRef, useState } from 'react';

import { runtimeConfig } from '@/config/runtime';
import {
  formatError,
  statusBadgeFor,
  type ActiveMobileThread,
  type PendingApprovalMetadata,
  type StatusBadgeState,
} from '@/hooks/gateway-chat-utils';
import { useGatewayChatThreads } from '@/hooks/useGatewayChatThreads';
import { useGatewayProviderAuth } from '@/hooks/useGatewayProviderAuth';
import { useGatewayTerminalSessions } from '@/hooks/useGatewayTerminalSessions';
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
  activePendingApprovalCount: number;
  terminalSessions: SessionInfo[];
  tmuxSupported: boolean;
  tmuxError: string | null;
  tmuxSessions: TmuxSessionInfo[];
  models: ModelOption[];
  skills: SkillOption[];
  collaborationModes: CollaborationModeOption[];
  accountProviders: ChatAccountProviderStatus[];
  /** True if all enabled providers are logged in */
  providerAuthOk: boolean;
  selectedModel: string | null;
  selectedEffort: ChatEffort;
  selectedPermission: ChatPermissionMode;
  selectedCollaborationMode: string | null;
  setSelectedModel: (modelId: string | null) => void;
  setSelectedEffort: (effort: ChatEffort) => void;
  setSelectedPermission: (permission: ChatPermissionMode) => void;
  setSelectedCollaborationMode: (modeId: string | null) => void;
  selectThread: (chatId: string) => void;
  refreshThreads: () => Promise<void>;
  refreshAccountProviders: () => Promise<void>;
  refreshTerminals: () => Promise<void>;
  startTerminalSession: (shell?: string) => Promise<string | null>;
  attachTmuxSession: (sessionName: string) => Promise<string | null>;
  attachTerminalSession: (
    sessionId: string,
    options?: { replay?: boolean; maxBytes?: number },
  ) => Promise<void>;
  resizeTerminalSession: (sessionId: string, cols: number, rows: number) => Promise<void>;
  sendTerminalInput: (sessionId: string, data: string) => Promise<void>;
  onTerminalBinary: (callback: (data: ArrayBuffer) => void) => () => void;
  createChat: () => Promise<void>;
  sendMessage: (message: string) => Promise<void>;
  stopChat: () => Promise<void>;
  renameThread: (chatId: string, title: string) => Promise<void>;
  archiveThread: (chatId: string) => Promise<void>;
  respondApproval: (requestId: number | string, decision: ChatApprovalDecision) => Promise<void>;
  startProviderLogin: (provider: string, profile?: string) => Promise<ChatDeviceCodeSession>;
  pollProviderLogin: (
    provider: string,
    session: ChatDeviceCodeSession,
    profile?: string,
  ) => Promise<ChatDeviceCodePollResult>;
  queuedMessage: string | null;
  clearQueuedMessage: () => void;
}

export function useGatewayChat(
  gatewayUrl = runtimeConfig.gatewayUrl,
): UseGatewayChatResult {
  const [status, setStatus] = useState<ConnectionStatus>('disconnected');
  const [error, setError] = useState<string | null>(null);

  const transportRef = useRef<GatewayTransport | null>(null);
  const chatClientRef = useRef<ReturnType<typeof createChatClient> | null>(null);
  const connectionEpochRef = useRef(0);
  const bootstrappedRef = useRef(false);

  const providerAuth = useGatewayProviderAuth({
    chatClientRef,
    connectionEpochRef,
    status,
  });

  const chatThreads = useGatewayChatThreads({
    gatewayUrl,
    chatClientRef,
    setError,
    getMessagePreferences: providerAuth.getMessagePreferences,
  });

  const terminals = useGatewayTerminalSessions({
    transportRef,
    connectionEpochRef,
    status,
    setError,
  });
  const { handleGatewayEvent, refreshThreads } = chatThreads;
  const { refreshTerminals } = terminals;

  useEffect(() => {
    const transport = createMobileGatewayClient({
      url: gatewayUrl,
    });
    const chatClient = createChatClient(transport);
    transportRef.current = transport;
    chatClientRef.current = chatClient;

    const unsubscribeState = transport.onStateChange((nextState) => {
      if (nextState.status !== 'connected') {
        connectionEpochRef.current += 1;
      }
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
      connectionEpochRef.current += 1;
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
      return;
    }
    if (bootstrappedRef.current) return;
    bootstrappedRef.current = true;

    const transport = transportRef.current;
    if (!transport) return;
    const epoch = connectionEpochRef.current;
    void subscribeToChatEvents(transport.call.bind(transport), 'chat.*').catch((nextError) => {
      if (connectionEpochRef.current !== epoch || transportRef.current !== transport) return;
      setError(formatError(nextError));
    });
    void refreshThreads();
    void refreshTerminals();
  }, [refreshTerminals, refreshThreads, status]);

  return {
    status,
    statusBadge: statusBadgeFor(status),
    gatewayUrl,
    threads: chatThreads.threads,
    activeChatId: chatThreads.activeChatId,
    activeThread: chatThreads.activeThread,
    error,
    loadingThreads: chatThreads.loadingThreads,
    loadingMessages: chatThreads.loadingMessages,
    creatingChat: chatThreads.creatingChat,
    sendingMessage: chatThreads.sendingMessage,
    loadingTerminals: terminals.loadingTerminals,
    pendingApproval: chatThreads.pendingApproval,
    activePendingApprovalCount: chatThreads.activePendingApprovalCount,
    terminalSessions: terminals.terminalSessions,
    tmuxSupported: terminals.tmuxSupported,
    tmuxError: terminals.tmuxError,
    tmuxSessions: terminals.tmuxSessions,
    models: providerAuth.models,
    skills: providerAuth.skills,
    collaborationModes: providerAuth.collaborationModes,
    accountProviders: providerAuth.accountProviders,
    providerAuthOk: providerAuth.providerAuthOk,
    selectedModel: providerAuth.selectedModel,
    selectedEffort: providerAuth.selectedEffort,
    selectedPermission: providerAuth.selectedPermission,
    selectedCollaborationMode: providerAuth.selectedCollaborationMode,
    setSelectedModel: providerAuth.setSelectedModel,
    setSelectedEffort: providerAuth.setSelectedEffort,
    setSelectedPermission: providerAuth.setSelectedPermission,
    setSelectedCollaborationMode: providerAuth.setSelectedCollaborationMode,
    selectThread: chatThreads.selectThread,
    refreshThreads: chatThreads.refreshThreads,
    refreshAccountProviders: providerAuth.refreshAccountProviders,
    refreshTerminals: terminals.refreshTerminals,
    startTerminalSession: terminals.startTerminalSession,
    attachTmuxSession: terminals.attachTmuxSession,
    attachTerminalSession: terminals.attachTerminalSession,
    resizeTerminalSession: terminals.resizeTerminalSession,
    sendTerminalInput: terminals.sendTerminalInput,
    onTerminalBinary: terminals.onTerminalBinary,
    createChat: chatThreads.createChat,
    sendMessage: chatThreads.sendMessage,
    stopChat: chatThreads.stopChat,
    renameThread: chatThreads.renameThread,
    archiveThread: chatThreads.archiveThread,
    respondApproval: chatThreads.respondApproval,
    startProviderLogin: providerAuth.startProviderLogin,
    pollProviderLogin: providerAuth.pollProviderLogin,
    queuedMessage: chatThreads.queuedMessage,
    clearQueuedMessage: chatThreads.clearQueuedMessage,
  };
}
