/**
 * Mock GatewayTransport that simulates RPC calls and event emission
 * without opening a real WebSocket connection.
 */

import type {
  ConnectionStatus,
  GatewayTransportState,
  RpcEvent,
  Unsubscribe,
} from '@homie/shared';

export type RpcHandler = (method: string, params?: unknown) => unknown | Promise<unknown>;

export interface MockTransportOptions {
  /** Default RPC handler — called for any method without a specific override. */
  defaultHandler?: RpcHandler;
  /** Per-method handlers (takes precedence over defaultHandler). */
  handlers?: Record<string, RpcHandler>;
}

export class MockTransport {
  private status: ConnectionStatus = 'disconnected';
  private stateListeners = new Set<(state: GatewayTransportState) => void>();
  private eventListeners = new Set<(event: RpcEvent) => void>();
  private binaryListeners = new Set<(data: ArrayBuffer) => void>();
  private handlers: Record<string, RpcHandler>;
  private defaultHandler: RpcHandler;

  constructor(options: MockTransportOptions = {}) {
    this.handlers = { ...options.handlers };
    this.defaultHandler =
      options.defaultHandler ?? (() => {
        throw Object.assign(new Error('Method not found'), { code: -32601 });
      });
  }

  // --- Transport API surface used by createChatClient / useGatewayChat ---

  start(): void {
    this.setStatus('connecting');
    // Simulate async handshake
    queueMicrotask(() => {
      this.setStatus('handshaking');
      queueMicrotask(() => this.setStatus('connected'));
    });
  }

  stop(): void {
    this.setStatus('disconnected');
  }

  async call<TResult = unknown>(method: string, params?: unknown): Promise<TResult> {
    const handler = this.handlers[method] ?? this.defaultHandler;
    const result = await handler(method, params);
    return result as TResult;
  }

  onStateChange(listener: (state: GatewayTransportState) => void): Unsubscribe {
    this.stateListeners.add(listener);
    listener(this.getState());
    return () => this.stateListeners.delete(listener);
  }

  onEvent(listener: (event: RpcEvent) => void): Unsubscribe {
    this.eventListeners.add(listener);
    return () => this.eventListeners.delete(listener);
  }

  onBinaryMessage(listener: (data: ArrayBuffer) => void): Unsubscribe {
    this.binaryListeners.add(listener);
    return () => this.binaryListeners.delete(listener);
  }

  getState(): GatewayTransportState {
    return {
      status: this.status,
      serverHello: null,
      rejection: null,
      error: null,
    };
  }

  // --- Test helpers ---

  /** Set a per-method handler. */
  setHandler(method: string, handler: RpcHandler): void {
    this.handlers[method] = handler;
  }

  /** Remove a per-method handler. */
  removeHandler(method: string): void {
    delete this.handlers[method];
  }

  /** Emit an RPC event to all listeners (simulates server push). */
  emitEvent(event: RpcEvent): void {
    for (const listener of this.eventListeners) {
      listener(event);
    }
  }

  /** Emit a binary message to all listeners. */
  emitBinary(data: ArrayBuffer): void {
    for (const listener of this.binaryListeners) {
      listener(data);
    }
  }

  /** Programmatically set the connection status and notify listeners. */
  setStatus(status: ConnectionStatus): void {
    this.status = status;
    const state = this.getState();
    for (const listener of this.stateListeners) {
      listener(state);
    }
  }

  /** Convenience: transition to 'connected' in one call. */
  simulateConnect(): void {
    this.setStatus('connected');
  }

  /** Convenience: transition to 'disconnected'. */
  simulateDisconnect(): void {
    this.setStatus('disconnected');
  }
}

// ---- Factories for common RPC responses ----

export function chatListResponse(
  chats: Array<{
    chatId: string;
    threadId: string;
    createdAt?: string;
    status?: string;
  }>,
) {
  return {
    chats: chats.map((c) => ({
      chat_id: c.chatId,
      thread_id: c.threadId,
      created_at: c.createdAt ?? String(Math.floor(Date.now() / 1000)),
      status: c.status ?? 'active',
    })),
  };
}

export function chatCreateResponse(chatId: string, threadId: string) {
  return { chat_id: chatId, thread_id: threadId };
}

export function chatThreadReadResponse(
  thread: Record<string, unknown> | null,
  settings?: Record<string, unknown>,
) {
  return { thread, settings: settings ?? null };
}

export function chatSendMessageResponse(chatId: string, turnId: string) {
  return { chat_id: chatId, turn_id: turnId };
}

// ---- Event factories ----

export function turnStartedEvent(threadId: string, turnId: string): RpcEvent {
  return {
    type: 'event',
    topic: 'chat.turn.started',
    params: { thread_id: threadId, turn_id: turnId },
  };
}

export function turnCompletedEvent(threadId: string, turnId: string): RpcEvent {
  return {
    type: 'event',
    topic: 'chat.turn.completed',
    params: { thread_id: threadId, turn_id: turnId },
  };
}

export function messageDeltaEvent(
  threadId: string,
  turnId: string,
  delta: string,
  itemId?: string,
): RpcEvent {
  return {
    type: 'event',
    topic: 'chat.message.delta',
    params: { thread_id: threadId, turn_id: turnId, delta, item_id: itemId },
  };
}

export function itemStartedEvent(
  threadId: string,
  turnId: string,
  item: Record<string, unknown>,
): RpcEvent {
  return {
    type: 'event',
    topic: 'chat.item.started',
    params: { thread_id: threadId, turn_id: turnId, item },
  };
}

export function itemCompletedEvent(
  threadId: string,
  turnId: string,
  item: Record<string, unknown>,
): RpcEvent {
  return {
    type: 'event',
    topic: 'chat.item.completed',
    params: { thread_id: threadId, turn_id: turnId, item },
  };
}

export function approvalRequiredEvent(
  threadId: string,
  turnId: string,
  requestId: number | string,
  opts: { reason?: string; command?: string; cwd?: string; itemId?: string } = {},
): RpcEvent {
  return {
    type: 'event',
    topic: 'chat.approval.required',
    params: {
      thread_id: threadId,
      turn_id: turnId,
      codex_request_id: requestId,
      item_id: opts.itemId ?? `approval-${requestId}`,
      reason: opts.reason,
      command: opts.command,
      cwd: opts.cwd,
    },
  };
}

export function commandOutputEvent(
  threadId: string,
  turnId: string,
  delta: string,
  itemId?: string,
): RpcEvent {
  return {
    type: 'event',
    topic: 'chat.command.output',
    params: { thread_id: threadId, turn_id: turnId, delta, item_id: itemId },
  };
}

export function planUpdatedEvent(
  threadId: string,
  turnId: string,
  explanation: string,
  plan: Array<{ step: string; status: string }>,
): RpcEvent {
  return {
    type: 'event',
    topic: 'chat.plan.updated',
    params: { thread_id: threadId, turn_id: turnId, explanation, plan },
  };
}
