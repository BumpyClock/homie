import type {
  ChatAccountProviderStatus,
  ChatDeviceCodePollResult,
  ChatDeviceCodeSession,
  ChatEffort,
  ChatPermissionMode,
  CollaborationModeOption,
  ConnectionStatus,
  ModelOption,
  SkillOption,
  createChatClient,
} from '@homie/shared';
import AsyncStorage from '@react-native-async-storage/async-storage';
import { useCallback, useEffect, useRef, useState, type MutableRefObject } from 'react';

type ChatClient = ReturnType<typeof createChatClient>;

interface UseGatewayProviderAuthOptions {
  chatClientRef: MutableRefObject<ChatClient | null>;
  connectionEpochRef: MutableRefObject<number>;
  status: ConnectionStatus;
}

export interface ChatMessagePreferences {
  model?: string;
  effort?: ChatEffort;
}

export interface UseGatewayProviderAuthResult {
  models: ModelOption[];
  skills: SkillOption[];
  collaborationModes: CollaborationModeOption[];
  accountProviders: ChatAccountProviderStatus[];
  providerAuthOk: boolean;
  selectedModel: string | null;
  selectedEffort: ChatEffort;
  selectedPermission: ChatPermissionMode;
  selectedCollaborationMode: string | null;
  setSelectedModel: (modelId: string | null) => void;
  setSelectedEffort: (effort: ChatEffort) => void;
  setSelectedPermission: (permission: ChatPermissionMode) => void;
  setSelectedCollaborationMode: (modeId: string | null) => void;
  refreshAccountProviders: () => Promise<void>;
  startProviderLogin: (provider: string, profile?: string) => Promise<ChatDeviceCodeSession>;
  pollProviderLogin: (
    provider: string,
    session: ChatDeviceCodeSession,
    profile?: string,
  ) => Promise<ChatDeviceCodePollResult>;
  getMessagePreferences: () => ChatMessagePreferences;
}

const SELECTED_MODEL_KEY = 'homie.mobile.selected_model';
const SELECTED_EFFORT_KEY = 'homie.mobile.selected_effort';
const SELECTED_PERMISSION_KEY = 'homie.mobile.selected_permission';
const SELECTED_COLLABORATION_MODE_KEY = 'homie.mobile.selected_collaboration_mode';

function staleGatewayTargetError(): Error {
  const error = new Error('Gateway target changed.');
  error.name = 'AbortError';
  return error;
}

export function useGatewayProviderAuth({
  chatClientRef,
  connectionEpochRef,
  status,
}: UseGatewayProviderAuthOptions): UseGatewayProviderAuthResult {
  const [models, setModels] = useState<ModelOption[]>([]);
  const [skills, setSkills] = useState<SkillOption[]>([]);
  const [collaborationModes, setCollaborationModes] = useState<CollaborationModeOption[]>([]);
  const [accountProviders, setAccountProviders] = useState<ChatAccountProviderStatus[]>([]);
  const [selectedModel, setSelectedModelState] = useState<string | null>(null);
  const selectedModelRef = useRef<string | null>(null);
  const [selectedEffort, setSelectedEffortState] = useState<ChatEffort>('auto');
  const selectedEffortRef = useRef<ChatEffort>('auto');
  const [selectedPermission, setSelectedPermissionState] = useState<ChatPermissionMode>('ask');
  const selectedPermissionRef = useRef<ChatPermissionMode>('ask');
  const [selectedCollaborationMode, setSelectedCollaborationModeState] = useState<string | null>(null);
  const selectedCollaborationModeRef = useRef<string | null>(null);
  const statusRef = useRef(status);
  statusRef.current = status;

  const isCurrentClient = useCallback(
    (chatClient: ChatClient, epoch: number) =>
      statusRef.current === 'connected' &&
      connectionEpochRef.current === epoch &&
      chatClientRef.current === chatClient,
    [chatClientRef, connectionEpochRef],
  );

  const setSelectedModel = useCallback((modelId: string | null) => {
    selectedModelRef.current = modelId;
    setSelectedModelState(modelId);
    if (modelId) {
      void AsyncStorage.setItem(SELECTED_MODEL_KEY, modelId).catch(() => { return; });
    } else {
      void AsyncStorage.removeItem(SELECTED_MODEL_KEY).catch(() => { return; });
    }
  }, []);

  const setSelectedEffort = useCallback((effort: ChatEffort) => {
    selectedEffortRef.current = effort;
    setSelectedEffortState(effort);
    void AsyncStorage.setItem(SELECTED_EFFORT_KEY, effort).catch(() => { return; });
  }, []);

  const setSelectedPermission = useCallback((permission: ChatPermissionMode) => {
    selectedPermissionRef.current = permission;
    setSelectedPermissionState(permission);
    void AsyncStorage.setItem(SELECTED_PERMISSION_KEY, permission).catch(() => { return; });
  }, []);

  const setSelectedCollaborationMode = useCallback((modeId: string | null) => {
    selectedCollaborationModeRef.current = modeId;
    setSelectedCollaborationModeState(modeId);
    if (modeId) {
      void AsyncStorage.setItem(SELECTED_COLLABORATION_MODE_KEY, modeId).catch(() => { return; });
    } else {
      void AsyncStorage.removeItem(SELECTED_COLLABORATION_MODE_KEY).catch(() => { return; });
    }
  }, []);

  const refreshAccountProviders = useCallback(async () => {
    const chatClient = chatClientRef.current;
    if (!chatClient || status !== 'connected') {
      setAccountProviders([]);
      return;
    }
    const epoch = connectionEpochRef.current;
    try {
      const providers = await chatClient.listAccounts();
      if (!isCurrentClient(chatClient, epoch)) return;
      setAccountProviders(providers);
    } catch {
      if (!isCurrentClient(chatClient, epoch)) return;
      setAccountProviders([]);
    }
  }, [chatClientRef, connectionEpochRef, isCurrentClient, status]);

  useEffect(() => {
    if (status !== 'connected') {
      setAccountProviders([]);
      setModels([]);
      setSkills([]);
      setCollaborationModes([]);
      return;
    }

    const chatClient = chatClientRef.current;
    if (!chatClient) return;
    const epoch = connectionEpochRef.current;

    void refreshAccountProviders();

    void chatClient.listModels().then((nextModels) => {
      if (!isCurrentClient(chatClient, epoch)) return;
      setModels(nextModels);
      void AsyncStorage.getItem(SELECTED_MODEL_KEY).then((stored) => {
        if (!isCurrentClient(chatClient, epoch)) return;
        if (stored) {
          const match = nextModels.find((model) => model.model === stored || model.id === stored);
          if (match) {
            selectedModelRef.current = stored;
            setSelectedModelState(stored);
          }
        }
      }).catch(() => { return; });
    }).catch(() => { return; });

    void chatClient.listSkills().then((nextSkills) => {
      if (!isCurrentClient(chatClient, epoch)) return;
      setSkills(nextSkills);
    }).catch(() => { return; });

    void chatClient.listCollaborationModes().then((nextModes) => {
      if (!isCurrentClient(chatClient, epoch)) return;
      setCollaborationModes(nextModes);
      void AsyncStorage.getItem(SELECTED_COLLABORATION_MODE_KEY).then((stored) => {
        if (!isCurrentClient(chatClient, epoch)) return;
        if (stored) {
          const match = nextModes.find((mode) => mode.id === stored || mode.mode === stored);
          if (match) {
            selectedCollaborationModeRef.current = stored;
            setSelectedCollaborationModeState(stored);
          }
        }
      }).catch(() => { return; });
    }).catch(() => { return; });

    void AsyncStorage.getItem(SELECTED_EFFORT_KEY).then((stored) => {
      if (!isCurrentClient(chatClient, epoch)) return;
      if (stored) {
        selectedEffortRef.current = stored as ChatEffort;
        setSelectedEffortState(stored as ChatEffort);
      }
    }).catch(() => { return; });

    void AsyncStorage.getItem(SELECTED_PERMISSION_KEY).then((stored) => {
      if (!isCurrentClient(chatClient, epoch)) return;
      if (stored) {
        selectedPermissionRef.current = stored as ChatPermissionMode;
        setSelectedPermissionState(stored as ChatPermissionMode);
      }
    }).catch(() => { return; });
  }, [chatClientRef, connectionEpochRef, isCurrentClient, refreshAccountProviders, status]);

  const startProviderLogin = useCallback(
    async (provider: string, profile?: string) => {
      const chatClient = chatClientRef.current;
      if (!chatClient || status !== 'connected') {
        throw new Error('Gateway is not connected.');
      }
      const epoch = connectionEpochRef.current;
      const session = await chatClient.startAccountLogin({ provider, profile });
      if (!isCurrentClient(chatClient, epoch)) {
        throw staleGatewayTargetError();
      }
      return session;
    },
    [chatClientRef, connectionEpochRef, isCurrentClient, status],
  );

  const pollProviderLogin = useCallback(
    async (
      provider: string,
      session: ChatDeviceCodeSession,
      profile?: string,
    ): Promise<ChatDeviceCodePollResult> => {
      const chatClient = chatClientRef.current;
      if (!chatClient || status !== 'connected') {
        throw new Error('Gateway is not connected.');
      }
      const epoch = connectionEpochRef.current;
      const result = await chatClient.pollAccountLogin({ provider, session, profile });
      if (!isCurrentClient(chatClient, epoch)) {
        throw staleGatewayTargetError();
      }
      if (result.status === 'authorized') {
        void refreshAccountProviders();
      }
      return result;
    },
    [chatClientRef, connectionEpochRef, isCurrentClient, refreshAccountProviders, status],
  );

  const getMessagePreferences = useCallback((): ChatMessagePreferences => {
    const effort = selectedEffortRef.current;
    return {
      model: selectedModelRef.current ?? undefined,
      effort: effort !== 'auto' ? effort : undefined,
    };
  }, []);

  const providerAuthOk =
    accountProviders.length === 0 ||
    accountProviders.every((provider) => !provider.enabled || provider.loggedIn);

  return {
    models,
    skills,
    collaborationModes,
    accountProviders,
    providerAuthOk,
    selectedModel,
    selectedEffort,
    selectedPermission,
    selectedCollaborationMode,
    setSelectedModel,
    setSelectedEffort,
    setSelectedPermission,
    setSelectedCollaborationMode,
    refreshAccountProviders,
    startProviderLogin,
    pollProviderLogin,
    getMessagePreferences,
  };
}
