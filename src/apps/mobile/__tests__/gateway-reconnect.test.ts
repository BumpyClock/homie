/**
 * Integration tests for approval responses and reconnect flows
 * (bead remotely-8di.8.3).
 *
 * Mock-based tests always run. Live tests require GATEWAY_URL env var.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import { createChatClient, mapChatEvent } from '@homie/shared';
import {
  MockTransport,
  chatListResponse,
  chatCreateResponse,
  chatThreadReadResponse,
  turnStartedEvent,
} from './helpers/mock-transport';
import { LIVE_TEST_ENABLED, GATEWAY_URL } from './setup';
import {
  applyMappedEventToThread,
  applyApprovalDecisionToThread,
  applyApprovalStatusToThread,
  countPendingApprovals,
  pendingApprovalFromThread,
  type ActiveMobileThread,
} from '@/hooks/gateway-chat-utils';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function threadWithApproval(
  chatId: string,
  threadId: string,
  requestId: number | string,
): ActiveMobileThread {
  return {
    chatId,
    threadId,
    title: 'Test',
    items: [
      {
        id: `approval-${requestId}`,
        kind: 'approval',
        turnId: 'turn-1',
        requestId,
        reason: 'Run command',
        command: 'rm -rf /tmp/test',
        cwd: '/home/user',
        status: 'pending',
      },
    ],
    running: true,
    activeTurnId: 'turn-1',
  };
}

// ---------------------------------------------------------------------------
// Approval tests
// ---------------------------------------------------------------------------

describe('approval responses', () => {
  let transport: MockTransport;
  let client: ReturnType<typeof createChatClient>;

  beforeEach(() => {
    transport = new MockTransport({
      handlers: {
        'chat.approval.respond': () => ({ ok: true }),
        'chat.list': () => chatListResponse([{ chatId: 'c1', threadId: 't1' }]),
        'chat.create': () => chatCreateResponse('c1', 't1'),
        'chat.thread.read': () => chatThreadReadResponse({ id: 't1', turns: [] }),
        'events.subscribe': () => ({ ok: true }),
      },
    });
    client = createChatClient(transport);
  });

  it('approval.required event produces an approval item', () => {
    const event = mapChatEvent(
      {
        topic: 'chat.approval.required',
        params: {
          thread_id: 't1',
          turn_id: 'turn-1',
          codex_request_id: 42,
          item_id: 'approval-42',
          reason: 'Execute command',
          command: 'echo test',
          cwd: '/tmp',
        },
      },
      { threadIdLookup: new Map([['t1', 'c1']]) },
    );

    expect(event).not.toBeNull();
    expect(event!.type).toBe('approval.required');
    if (event!.type === 'approval.required') {
      expect(event!.requestId).toBe(42);
      expect(event!.command).toBe('echo test');
      expect(event!.reason).toBe('Execute command');
    }
  });

  it('applyMappedEventToThread adds approval item', () => {
    const thread: ActiveMobileThread = {
      chatId: 'c1',
      threadId: 't1',
      title: 'Test',
      items: [],
      running: true,
      activeTurnId: 'turn-1',
    };

    const event = mapChatEvent(
      {
        topic: 'chat.approval.required',
        params: {
          thread_id: 't1',
          turn_id: 'turn-1',
          codex_request_id: 42,
          item_id: 'approval-42',
          reason: 'Run rm',
          command: 'rm /tmp/x',
          cwd: '/tmp',
        },
      },
      { threadIdLookup: new Map([['t1', 'c1']]) },
    )!;

    const updated = applyMappedEventToThread(thread, event);
    expect(updated.items).toHaveLength(1);
    expect(updated.items[0].kind).toBe('approval');
    expect(updated.items[0].status).toBe('pending');
    expect(updated.items[0].requestId).toBe(42);
  });

  it('applyApprovalDecisionToThread sets decision status', () => {
    const thread = threadWithApproval('c1', 't1', 42);
    expect(countPendingApprovals(thread.items)).toBe(1);

    const accepted = applyApprovalDecisionToThread(thread, 42, 'accept');
    expect(accepted.items[0].status).toBe('accept');
    expect(countPendingApprovals(accepted.items)).toBe(0);
  });

  it('applyApprovalStatusToThread updates status', () => {
    const thread = threadWithApproval('c1', 't1', 99);
    const updated = applyApprovalStatusToThread(thread, 99, 'decline');
    expect(updated.items[0].status).toBe('decline');
  });

  it('pendingApprovalFromThread returns most recent pending', () => {
    const thread: ActiveMobileThread = {
      chatId: 'c1',
      threadId: 't1',
      title: 'Test',
      items: [
        { id: 'a1', kind: 'approval', requestId: 1, status: 'accept', turnId: 'turn-1' },
        { id: 'a2', kind: 'approval', requestId: 2, status: 'pending', turnId: 'turn-1', reason: 'Delete', command: 'rm file' },
        { id: 'a3', kind: 'approval', requestId: 3, status: 'pending', turnId: 'turn-1', reason: 'Run', command: 'echo hi' },
      ],
      running: true,
    };

    const pending = pendingApprovalFromThread(thread);
    expect(pending).not.toBeNull();
    // Returns the LAST pending (most recent)
    expect(pending!.requestId).toBe(3);
    expect(pending!.command).toBe('echo hi');
  });

  it('countPendingApprovals counts only pending items', () => {
    const items = [
      { id: 'a1', kind: 'approval' as const, requestId: 1, status: 'accept' },
      { id: 'a2', kind: 'approval' as const, requestId: 2, status: 'pending' },
      { id: 'a3', kind: 'approval' as const, requestId: 3 }, // no status -> treated as pending
      { id: 'u1', kind: 'user' as const, text: 'hello' },
    ];
    expect(countPendingApprovals(items)).toBe(2);
  });

  it('respondApproval sends RPC with correct params', async () => {
    let capturedParams: unknown = null;
    transport.setHandler('chat.approval.respond', (_m, params) => {
      capturedParams = params;
      return { ok: true };
    });

    await client.respondApproval({ requestId: 42, decision: 'accept' });
    expect(capturedParams).toEqual({
      codex_request_id: 42,
      decision: 'accept',
    });
  });

  it('respondApproval with decline', async () => {
    let capturedParams: unknown = null;
    transport.setHandler('chat.approval.respond', (_m, params) => {
      capturedParams = params;
      return { ok: true };
    });

    await client.respondApproval({ requestId: 99, decision: 'decline' });
    expect(capturedParams).toEqual({
      codex_request_id: 99,
      decision: 'decline',
    });
  });
});

// ---------------------------------------------------------------------------
// Reconnect flow tests
// ---------------------------------------------------------------------------

describe('reconnect flows', () => {
  let transport: MockTransport;

  beforeEach(() => {
    transport = new MockTransport({
      handlers: {
        'events.subscribe': () => ({ ok: true }),
        'chat.list': () =>
          chatListResponse([{ chatId: 'c1', threadId: 't1' }]),
        'chat.thread.read': () =>
          chatThreadReadResponse({
            id: 't1',
            turns: [
              {
                id: 'turn-1',
                items: [
                  { id: 'u1', type: 'userMessage', content: [{ type: 'text', text: 'Hi' }] },
                  { id: 'a1', type: 'agentMessage', text: 'Hello!' },
                ],
              },
            ],
          }),
      },
    });
  });

  it('disconnect then reconnect triggers state transitions', () => {
    const statuses: string[] = [];
    transport.onStateChange((state) => statuses.push(state.status));

    // Initial state emitted by onStateChange
    expect(statuses).toContain('disconnected');

    transport.simulateConnect();
    expect(statuses).toContain('connected');

    transport.simulateDisconnect();
    expect(statuses.filter((s) => s === 'disconnected')).toHaveLength(2);

    transport.simulateConnect();
    expect(statuses.filter((s) => s === 'connected')).toHaveLength(2);
  });

  it('re-subscribes to events on reconnect', async () => {
    let subscribeCount = 0;
    transport.setHandler('events.subscribe', () => {
      subscribeCount += 1;
      return { ok: true };
    });

    // First connection
    transport.simulateConnect();
    await transport.call('events.subscribe', { topic: 'chat.*' });
    expect(subscribeCount).toBe(1);

    // Disconnect
    transport.simulateDisconnect();

    // Reconnect
    transport.simulateConnect();
    await transport.call('events.subscribe', { topic: 'chat.*' });
    expect(subscribeCount).toBe(2);
  });

  it('events emitted after reconnect are received', () => {
    const events: { topic: string }[] = [];
    transport.onEvent((e) => events.push({ topic: e.topic }));

    // First connection
    transport.simulateConnect();
    transport.emitEvent(turnStartedEvent('t1', 'turn-1'));
    expect(events).toHaveLength(1);

    // Disconnect
    transport.simulateDisconnect();

    // Reconnect
    transport.simulateConnect();
    transport.emitEvent(turnStartedEvent('t1', 'turn-2'));
    expect(events).toHaveLength(2);
    expect(events[1].topic).toBe('chat.turn.started');
  });

  it('approval state survives reconnect cycle', () => {
    const threadIdLookup = new Map([['t1', 'c1']]);
    let thread: ActiveMobileThread = {
      chatId: 'c1',
      threadId: 't1',
      title: 'Test',
      items: [],
      running: false,
    };

    // Connect and receive approval
    transport.simulateConnect();

    const startEvent = mapChatEvent(
      { topic: 'chat.turn.started', params: { thread_id: 't1', turn_id: 'turn-1' } },
      { threadIdLookup },
    )!;
    thread = applyMappedEventToThread(thread, startEvent);

    const approvalEvent = mapChatEvent(
      {
        topic: 'chat.approval.required',
        params: {
          thread_id: 't1',
          turn_id: 'turn-1',
          codex_request_id: 42,
          item_id: 'approval-42',
          reason: 'Run cmd',
          command: 'echo ok',
        },
      },
      { threadIdLookup },
    )!;
    thread = applyMappedEventToThread(thread, approvalEvent);
    expect(thread.items).toHaveLength(1);
    expect(thread.items[0].status).toBe('pending');

    // Disconnect — thread state is held in memory
    transport.simulateDisconnect();

    // Thread still has the pending approval
    const pending = pendingApprovalFromThread(thread);
    expect(pending).not.toBeNull();
    expect(pending!.requestId).toBe(42);

    // Reconnect
    transport.simulateConnect();

    // Respond to approval after reconnect
    const decided = applyApprovalDecisionToThread(thread, 42, 'accept');
    expect(decided.items[0].status).toBe('accept');
    expect(pendingApprovalFromThread(decided)).toBeNull();
  });

  it('thread read after reconnect loads cached data', async () => {
    transport.simulateConnect();
    const client = createChatClient(transport);

    const result = await client.readThread('c1', 't1', true);
    expect(result.thread).not.toBeNull();

    // Verify turns are present
    const thread = result.thread as Record<string, unknown>;
    const turns = thread.turns as unknown[];
    expect(turns).toHaveLength(1);
  });
});

// ---------------------------------------------------------------------------
// Live tests (only when GATEWAY_URL is set)
// ---------------------------------------------------------------------------

describe.skipIf(!LIVE_TEST_ENABLED)('reconnect — live', () => {
  it('connects, lists, disconnects, reconnects, lists again', async () => {
    const { GatewayTransport, createChatClient: createClient, subscribeToChatEvents } =
      await import('@homie/shared');

    const transport = new GatewayTransport({
      url: GATEWAY_URL,
      clientId: 'homie-mobile-test/0.1.0',
      capabilities: ['chat'],
      reconnect: false,
    });

    const waitForConnected = () =>
      new Promise<void>((resolve, reject) => {
        const timeout = setTimeout(() => reject(new Error('Timeout')), 10_000);
        const unsub = transport.onStateChange((s) => {
          if (s.status === 'connected') {
            clearTimeout(timeout);
            unsub();
            resolve();
          }
          if (s.status === 'error' || s.status === 'rejected') {
            clearTimeout(timeout);
            unsub();
            reject(new Error(s.status));
          }
        });
      });

    // First connection
    transport.start();
    await waitForConnected();

    const client = createClient(transport);
    await subscribeToChatEvents(transport.call.bind(transport));
    const firstList = await client.list();
    expect(Array.isArray(firstList)).toBe(true);

    transport.stop();

    // Second connection
    transport.start();
    await waitForConnected();
    await subscribeToChatEvents(transport.call.bind(transport));
    const secondList = await client.list();
    expect(Array.isArray(secondList)).toBe(true);

    transport.stop();
  }, 30_000);
});
