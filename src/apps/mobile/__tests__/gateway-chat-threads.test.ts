import React, { type MutableRefObject } from 'react';
import { afterEach, beforeAll, describe, expect, it, vi } from 'vitest';
// @ts-expect-error react-test-renderer is a test-only dependency without bundled types here.
import TestRenderer, { act } from 'react-test-renderer';
import { createChatClient } from '@homie/shared';

import { useGatewayChatThreads } from '@/hooks/useGatewayChatThreads';
import { resetAsyncStorage } from './helpers/mock-async-storage';
import {
  MockTransport,
  chatCreateResponse,
  chatListResponse,
  chatSendMessageResponse,
  chatThreadReadResponse,
  turnCompletedEvent,
  turnStartedEvent,
} from './helpers/mock-transport';

type ChatClient = ReturnType<typeof createChatClient>;
type HookResult = ReturnType<typeof useGatewayChatThreads>;

function createDeferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((nextResolve, nextReject) => {
    resolve = nextResolve;
    reject = nextReject;
  });
  return { promise, resolve, reject };
}

function renderGatewayChatThreads(options: {
  gatewayUrl: string;
  chatClientRef: MutableRefObject<ChatClient | null>;
}) {
  let latest: HookResult | null = null;
  const setError = vi.fn();
  const getMessagePreferences = () => ({});

  function Harness({ gatewayUrl }: { gatewayUrl: string }) {
    latest = useGatewayChatThreads({
      gatewayUrl,
      chatClientRef: options.chatClientRef,
      setError,
      getMessagePreferences,
    });
    return null;
  }

  let renderer: TestRenderer.ReactTestRenderer;
  act(() => {
    renderer = TestRenderer.create(React.createElement(Harness, { gatewayUrl: options.gatewayUrl }));
  });

  return {
    get result() {
      if (!latest) throw new Error('Hook result is not available');
      return latest;
    },
    setError,
    updateGatewayUrl(gatewayUrl: string) {
      act(() => {
        renderer.update(React.createElement(Harness, { gatewayUrl }));
      });
    },
    unmount() {
      act(() => {
        renderer.unmount();
      });
    },
  };
}

async function settleHook() {
  await act(async () => {
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
  });
}

async function loadTwoThreads(hook: ReturnType<typeof renderGatewayChatThreads>) {
  await act(async () => {
    await hook.result.refreshThreads();
  });
  await settleHook();
}

describe('useGatewayChatThreads target-bound async behavior', () => {
  beforeAll(() => {
    (globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  });

  afterEach(() => {
    resetAsyncStorage();
    vi.useRealTimers();
    vi.clearAllMocks();
  });

  it('ignores refreshThreads results from a stale gateway target', async () => {
    const listFromFirstTarget = createDeferred<unknown>();
    const firstTransport = new MockTransport({
      handlers: {
        'chat.list': () => listFromFirstTarget.promise,
        'chat.thread.read': (_method, params) => {
          const threadId = (params as { thread_id?: string }).thread_id ?? 'thread-a';
          return chatThreadReadResponse({ id: threadId, turns: [] });
        },
      },
    });
    const secondTransport = new MockTransport({
      handlers: {
        'chat.list': () => chatListResponse([]),
        'chat.thread.read': () => chatThreadReadResponse(null),
      },
    });
    const chatClientRef: MutableRefObject<ChatClient | null> = {
      current: createChatClient(firstTransport),
    };
    const hook = renderGatewayChatThreads({ gatewayUrl: 'ws://target-a', chatClientRef });

    let refreshPromise!: Promise<void>;
    act(() => {
      refreshPromise = hook.result.refreshThreads();
    });

    chatClientRef.current = createChatClient(secondTransport);
    hook.updateGatewayUrl('ws://target-b');

    await act(async () => {
      listFromFirstTarget.resolve(chatListResponse([{ chatId: 'chat-a', threadId: 'thread-a' }]));
      await refreshPromise;
    });

    expect(hook.result.threads).toEqual([]);
    expect(hook.result.activeThread).toBeNull();

    hook.unmount();
  });

  it('ignores loadThread results from a stale gateway target', async () => {
    const readFromFirstTarget = createDeferred<unknown>();
    const firstTransport = new MockTransport({
      handlers: {
        'chat.list': () =>
          chatListResponse([
            { chatId: 'chat-1', threadId: 'thread-1', createdAt: '200' },
            { chatId: 'chat-2', threadId: 'thread-2', createdAt: '100' },
          ]),
        'chat.thread.read': (_method, params) => {
          const threadId = (params as { thread_id?: string }).thread_id ?? 'thread-1';
          if (threadId === 'thread-2') return readFromFirstTarget.promise;
          return chatThreadReadResponse({ id: threadId, turns: [] });
        },
      },
    });
    const secondTransport = new MockTransport({
      handlers: {
        'chat.list': () => chatListResponse([]),
        'chat.thread.read': () => chatThreadReadResponse(null),
      },
    });
    const chatClientRef: MutableRefObject<ChatClient | null> = {
      current: createChatClient(firstTransport),
    };
    const hook = renderGatewayChatThreads({ gatewayUrl: 'ws://target-a', chatClientRef });
    await loadTwoThreads(hook);

    act(() => {
      hook.result.selectThread('chat-2');
    });

    chatClientRef.current = createChatClient(secondTransport);
    hook.updateGatewayUrl('ws://target-b');

    await act(async () => {
      readFromFirstTarget.resolve(
        chatThreadReadResponse({
          id: 'thread-2',
          turns: [
            {
              id: 'turn-1',
              items: [
                { id: 'msg-1', type: 'agentMessage', text: 'stale target message' },
              ],
            },
          ],
        }),
      );
      await readFromFirstTarget.promise;
    });

    expect(hook.result.threads).toEqual([]);
    expect(hook.result.activeThread).toBeNull();

    hook.unmount();
  });

  it('ignores createChat results from a stale gateway target', async () => {
    const createFromFirstTarget = createDeferred<unknown>();
    const firstTransport = new MockTransport({
      handlers: {
        'chat.create': () => createFromFirstTarget.promise,
        'chat.thread.read': () => chatThreadReadResponse({ id: 'stale-thread', turns: [] }),
      },
    });
    const secondTransport = new MockTransport({
      handlers: {
        'chat.list': () => chatListResponse([]),
        'chat.thread.read': () => chatThreadReadResponse(null),
      },
    });
    const chatClientRef: MutableRefObject<ChatClient | null> = {
      current: createChatClient(firstTransport),
    };
    const hook = renderGatewayChatThreads({ gatewayUrl: 'ws://target-a', chatClientRef });

    let createPromise!: Promise<void>;
    act(() => {
      createPromise = hook.result.createChat();
    });

    chatClientRef.current = createChatClient(secondTransport);
    hook.updateGatewayUrl('ws://target-b');
    hook.setError.mockClear();

    await act(async () => {
      createFromFirstTarget.resolve(chatCreateResponse('stale-chat', 'stale-thread'));
      await createPromise;
    });

    expect(hook.result.threads).toEqual([]);
    expect(hook.result.activeThread).toBeNull();
    expect(hook.result.activeChatId).toBeNull();
    expect(hook.result.creatingChat).toBe(false);
    expect(hook.setError).not.toHaveBeenCalled();

    hook.unmount();
  });

  it('ignores sendMessage results from a stale gateway target', async () => {
    const sendFromFirstTarget = createDeferred<unknown>();
    const firstTransport = new MockTransport({
      handlers: {
        'chat.list': () =>
          chatListResponse([{ chatId: 'chat-1', threadId: 'thread-1', createdAt: '200' }]),
        'chat.thread.read': () => chatThreadReadResponse({ id: 'thread-1', turns: [] }),
        'chat.message.send': () => sendFromFirstTarget.promise,
      },
    });
    const secondTransport = new MockTransport({
      handlers: {
        'chat.list': () => chatListResponse([]),
        'chat.thread.read': () => chatThreadReadResponse(null),
      },
    });
    const chatClientRef: MutableRefObject<ChatClient | null> = {
      current: createChatClient(firstTransport),
    };
    const hook = renderGatewayChatThreads({ gatewayUrl: 'ws://target-a', chatClientRef });
    await loadTwoThreads(hook);

    let sendPromise!: Promise<void>;
    act(() => {
      sendPromise = hook.result.sendMessage('message for stale target');
    });

    chatClientRef.current = createChatClient(secondTransport);
    hook.updateGatewayUrl('ws://target-b');
    hook.setError.mockClear();

    await act(async () => {
      sendFromFirstTarget.resolve(chatSendMessageResponse('chat-1', 'turn-stale'));
      await sendPromise;
    });

    expect(hook.result.threads).toEqual([]);
    expect(hook.result.activeThread).toBeNull();
    expect(hook.result.activeChatId).toBeNull();
    expect(hook.result.sendingMessage).toBe(false);
    expect(hook.setError).not.toHaveBeenCalledWith(null);

    hook.unmount();
  });

  it('ignores sendMessage errors from a stale gateway target', async () => {
    const sendFromFirstTarget = createDeferred<unknown>();
    const firstTransport = new MockTransport({
      handlers: {
        'chat.list': () =>
          chatListResponse([{ chatId: 'chat-1', threadId: 'thread-1', createdAt: '200' }]),
        'chat.thread.read': () => chatThreadReadResponse({ id: 'thread-1', turns: [] }),
        'chat.message.send': () => sendFromFirstTarget.promise,
      },
    });
    const secondTransport = new MockTransport({
      handlers: {
        'chat.list': () => chatListResponse([]),
        'chat.thread.read': () => chatThreadReadResponse(null),
      },
    });
    const chatClientRef: MutableRefObject<ChatClient | null> = {
      current: createChatClient(firstTransport),
    };
    const hook = renderGatewayChatThreads({ gatewayUrl: 'ws://target-a', chatClientRef });
    await loadTwoThreads(hook);

    let sendPromise!: Promise<void>;
    act(() => {
      sendPromise = hook.result.sendMessage('message for stale target');
    });

    chatClientRef.current = createChatClient(secondTransport);
    hook.updateGatewayUrl('ws://target-b');
    hook.setError.mockClear();

    await act(async () => {
      sendFromFirstTarget.reject(new Error('stale send failed'));
      await sendPromise;
    });

    expect(hook.result.threads).toEqual([]);
    expect(hook.result.activeThread).toBeNull();
    expect(hook.result.activeChatId).toBeNull();
    expect(hook.result.sendingMessage).toBe(false);
    expect(hook.setError).not.toHaveBeenCalled();

    hook.unmount();
  });

  it('does not send a queued message when another thread completes', async () => {
    vi.useFakeTimers();
    const sends: unknown[] = [];
    const transport = new MockTransport({
      handlers: {
        'chat.list': () =>
          chatListResponse([
            { chatId: 'chat-1', threadId: 'thread-1', createdAt: '200' },
            { chatId: 'chat-2', threadId: 'thread-2', createdAt: '100' },
          ]),
        'chat.thread.read': (_method, params) => {
          const threadId = (params as { thread_id?: string }).thread_id ?? 'thread-1';
          return chatThreadReadResponse({ id: threadId, turns: [] });
        },
        'chat.message.send': (_method, params) => {
          sends.push(params);
          return { chat_id: (params as { chat_id?: string }).chat_id, turn_id: 'turn-new' };
        },
      },
    });
    const chatClientRef: MutableRefObject<ChatClient | null> = {
      current: createChatClient(transport),
    };
    const hook = renderGatewayChatThreads({ gatewayUrl: 'ws://target-a', chatClientRef });
    await loadTwoThreads(hook);

    act(() => {
      hook.result.handleGatewayEvent(turnStartedEvent('thread-1', 'turn-1'));
    });
    await act(async () => {
      await hook.result.sendMessage('queued for thread one');
    });
    await act(async () => {
      hook.result.selectThread('chat-2');
    });
    await settleHook();

    act(() => {
      hook.result.handleGatewayEvent(turnCompletedEvent('thread-2', 'turn-2'));
    });
    await act(async () => {
      vi.advanceTimersByTime(150);
      await Promise.resolve();
    });

    expect(sends).toEqual([]);
    expect(hook.result.queuedMessage).toBe('queued for thread one');

    hook.unmount();
  });

  it('sends a queued message when the matching target and thread complete', async () => {
    vi.useFakeTimers();
    const sends: unknown[] = [];
    const transport = new MockTransport({
      handlers: {
        'chat.list': () =>
          chatListResponse([{ chatId: 'chat-1', threadId: 'thread-1', createdAt: '200' }]),
        'chat.thread.read': () => chatThreadReadResponse({ id: 'thread-1', turns: [] }),
        'chat.message.send': (_method, params) => {
          sends.push(params);
          return { chat_id: 'chat-1', turn_id: 'turn-new' };
        },
      },
    });
    const chatClientRef: MutableRefObject<ChatClient | null> = {
      current: createChatClient(transport),
    };
    const hook = renderGatewayChatThreads({ gatewayUrl: 'ws://target-a', chatClientRef });
    await loadTwoThreads(hook);

    act(() => {
      hook.result.handleGatewayEvent(turnStartedEvent('thread-1', 'turn-1'));
    });
    await act(async () => {
      await hook.result.sendMessage('queued for thread one');
    });
    act(() => {
      hook.result.handleGatewayEvent(turnCompletedEvent('thread-1', 'turn-1'));
    });
    await act(async () => {
      vi.advanceTimersByTime(150);
      await Promise.resolve();
    });

    expect(sends).toEqual([
      expect.objectContaining({
        chat_id: 'chat-1',
        message: 'queued for thread one',
      }),
    ]);
    expect(hook.result.queuedMessage).toBeNull();

    hook.unmount();
  });
});
