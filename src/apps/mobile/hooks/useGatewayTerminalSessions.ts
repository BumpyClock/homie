import type {
  ConnectionStatus,
  GatewayTransport,
  SessionInfo,
  TmuxListResponse,
  TmuxSessionInfo,
} from '@homie/shared';
import { useCallback, useEffect, useRef, useState, type MutableRefObject } from 'react';

import { formatError } from '@/hooks/gateway-chat-utils';

interface UseGatewayTerminalSessionsOptions {
  transportRef: MutableRefObject<GatewayTransport | null>;
  connectionEpochRef: MutableRefObject<number>;
  status: ConnectionStatus;
  setError: (error: string | null) => void;
}

export interface UseGatewayTerminalSessionsResult {
  terminalSessions: SessionInfo[];
  tmuxSupported: boolean;
  tmuxError: string | null;
  tmuxSessions: TmuxSessionInfo[];
  loadingTerminals: boolean;
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

function normalizeTmuxList(raw: unknown): TmuxListResponse {
  if (!raw || typeof raw !== 'object') {
    return { supported: false, sessions: [] };
  }
  const record = raw as Record<string, unknown>;
  const input = Array.isArray(record.sessions) ? record.sessions : [];
  const supported = record.supported === true || input.length > 0;
  const sessions: TmuxSessionInfo[] = input
    .map((entry): TmuxSessionInfo | null => {
      if (!entry || typeof entry !== 'object') return null;
      const session = entry as Record<string, unknown>;
      const name = typeof session.name === 'string' ? session.name : '';
      const windows = typeof session.windows === 'number' ? session.windows : 0;
      const attached = session.attached === true;
      if (!name) return null;
      return { name, windows, attached };
    })
    .filter((entry): entry is TmuxSessionInfo => entry !== null);
  return { supported, sessions };
}

function isRpcMethodNotFound(error: unknown): boolean {
  if (!error || typeof error !== 'object') return false;
  const record = error as Record<string, unknown>;
  if (typeof record.code === 'number') return record.code === -32601;
  return false;
}

function staleGatewayTargetError(): Error {
  const error = new Error('Gateway target changed.');
  error.name = 'AbortError';
  return error;
}

export function useGatewayTerminalSessions({
  transportRef,
  connectionEpochRef,
  status,
  setError,
}: UseGatewayTerminalSessionsOptions): UseGatewayTerminalSessionsResult {
  const [terminalSessions, setTerminalSessions] = useState<SessionInfo[]>([]);
  const [tmuxSessions, setTmuxSessions] = useState<TmuxSessionInfo[]>([]);
  const [tmuxSupported, setTmuxSupported] = useState(false);
  const [tmuxError, setTmuxError] = useState<string | null>(null);
  const [loadingTerminals, setLoadingTerminals] = useState(false);
  const statusRef = useRef(status);
  statusRef.current = status;

  const isCurrentTransport = useCallback(
    (transport: GatewayTransport, epoch: number) =>
      statusRef.current === 'connected' &&
      connectionEpochRef.current === epoch &&
      transportRef.current === transport,
    [connectionEpochRef, transportRef],
  );

  const refreshTerminals = useCallback(async () => {
    const transport = transportRef.current;
    if (!transport || status !== 'connected') {
      setTerminalSessions([]);
      setTmuxSessions([]);
      setTmuxSupported(false);
      setTmuxError(null);
      return;
    }

    const epoch = connectionEpochRef.current;
    setLoadingTerminals(true);
    try {
      const [sessionResult, tmuxResult] = await Promise.allSettled([
        transport.call<{ sessions?: unknown }>('terminal.session.list'),
        transport.call<TmuxListResponse>('terminal.tmux.list'),
      ]);

      if (!isCurrentTransport(transport, epoch)) return;

      if (sessionResult.status === 'rejected') {
        throw sessionResult.reason;
      }

      setTerminalSessions(normalizeTerminalSessions(sessionResult.value));

      if (tmuxResult.status === 'fulfilled') {
        const normalized = normalizeTmuxList(tmuxResult.value);
        setTmuxSupported(normalized.supported);
        setTmuxSessions(normalized.sessions);
        setTmuxError(null);
      } else if (isRpcMethodNotFound(tmuxResult.reason)) {
        setTmuxSupported(false);
        setTmuxSessions([]);
        setTmuxError(null);
      } else {
        setTmuxSupported(false);
        setTmuxSessions([]);
        setTmuxError(formatError(tmuxResult.reason));
      }

      setError(null);
    } catch (nextError) {
      if (!isCurrentTransport(transport, epoch)) return;
      setError(formatError(nextError));
    } finally {
      if (!isCurrentTransport(transport, epoch)) return;
      setLoadingTerminals(false);
    }
  }, [connectionEpochRef, isCurrentTransport, setError, status, transportRef]);

  useEffect(() => {
    if (status !== 'connected') {
      setTerminalSessions([]);
      setTmuxSessions([]);
      setTmuxSupported(false);
      setTmuxError(null);
      setLoadingTerminals(false);
    }
  }, [status]);

  const startTerminalSession = useCallback(async (shell?: string) => {
    const transport = transportRef.current;
    if (!transport || status !== 'connected') return null;
    const epoch = connectionEpochRef.current;
    try {
      const params = shell ? { shell } : undefined;
      const response = await transport.call<{ session_id?: string }>('terminal.session.start', params);
      if (!isCurrentTransport(transport, epoch)) return null;
      await refreshTerminals();
      if (!isCurrentTransport(transport, epoch)) return null;
      return typeof response.session_id === 'string' ? response.session_id : null;
    } catch (nextError) {
      if (!isCurrentTransport(transport, epoch)) return null;
      setError(formatError(nextError));
      return null;
    }
  }, [connectionEpochRef, isCurrentTransport, refreshTerminals, setError, status, transportRef]);

  const attachTmuxSession = useCallback(async (sessionName: string) => {
    const transport = transportRef.current;
    if (!transport || status !== 'connected') return null;
    const epoch = connectionEpochRef.current;
    try {
      const response = await transport.call<SessionInfo>('terminal.tmux.attach', {
        session_name: sessionName,
        cols: 80,
        rows: 24,
      });
      if (!isCurrentTransport(transport, epoch)) return null;
      await refreshTerminals();
      if (!isCurrentTransport(transport, epoch)) return null;
      return response?.session_id ?? null;
    } catch (nextError) {
      if (!isCurrentTransport(transport, epoch)) return null;
      setError(formatError(nextError));
      return null;
    }
  }, [connectionEpochRef, isCurrentTransport, refreshTerminals, setError, status, transportRef]);

  const attachTerminalSession = useCallback(
    async (sessionId: string, options?: { replay?: boolean; maxBytes?: number }) => {
      const transport = transportRef.current;
      if (!transport || status !== 'connected') return;
      const epoch = connectionEpochRef.current;
      try {
        await transport.call('terminal.session.attach', {
          session_id: sessionId,
          replay: options?.replay ?? true,
          max_bytes: options?.maxBytes ?? 65536,
        });
      } catch (nextError) {
        if (!isCurrentTransport(transport, epoch)) throw staleGatewayTargetError();
        throw nextError;
      }
      if (!isCurrentTransport(transport, epoch)) throw staleGatewayTargetError();
    },
    [connectionEpochRef, isCurrentTransport, status, transportRef],
  );

  const resizeTerminalSession = useCallback(
    async (sessionId: string, cols: number, rows: number) => {
      const transport = transportRef.current;
      if (!transport || status !== 'connected') return;
      const epoch = connectionEpochRef.current;
      try {
        await transport.call('terminal.session.resize', {
          session_id: sessionId,
          cols,
          rows,
        });
      } catch (nextError) {
        if (!isCurrentTransport(transport, epoch)) return;
        throw nextError;
      }
    },
    [connectionEpochRef, isCurrentTransport, status, transportRef],
  );

  const sendTerminalInput = useCallback(
    async (sessionId: string, data: string) => {
      const transport = transportRef.current;
      if (!transport || status !== 'connected') return;
      const epoch = connectionEpochRef.current;
      try {
        await transport.call('terminal.session.input', {
          session_id: sessionId,
          data,
        });
      } catch (nextError) {
        if (!isCurrentTransport(transport, epoch)) return;
        throw nextError;
      }
    },
    [connectionEpochRef, isCurrentTransport, status, transportRef],
  );

  const onTerminalBinary = useCallback((callback: (data: ArrayBuffer) => void) => {
    const transport = transportRef.current;
    if (!transport) return () => { return; };
    return transport.onBinaryMessage(callback);
  }, [transportRef]);

  return {
    terminalSessions,
    tmuxSupported,
    tmuxError,
    tmuxSessions,
    loadingTerminals,
    refreshTerminals,
    startTerminalSession,
    attachTmuxSession,
    attachTerminalSession,
    resizeTerminalSession,
    sendTerminalInput,
    onTerminalBinary,
  };
}
