/**
 * Integration tests for the gateway chat pipeline:
 *  - send / stream / tool events (bead remotely-8di.8.2)
 *
 * These tests exercise the chat-client + event-mapping layers against
 * a MockTransport.  When GATEWAY_URL is set they run against a real
 * gateway (live mode); otherwise they use the mock.
 */

import { describe, it, expect, beforeEach } from 'vitest';
import {
  createChatClient,
  mapChatEvent,
  buildChatThreadSummaries,
  type ChatMappedEvent,
  type RpcEvent,
} from '@homie/shared';
import {
  MockTransport,
  chatListResponse,
  chatCreateResponse,
  chatThreadReadResponse,
  chatSendMessageResponse,
  turnStartedEvent,
  turnCompletedEvent,
  messageDeltaEvent,
  itemStartedEvent,
  itemCompletedEvent,
  commandOutputEvent,
  planUpdatedEvent,
} from './helpers/mock-transport';
import { LIVE_TEST_ENABLED, GATEWAY_URL } from './setup';
import {
  applyMappedEventToThread,
  type ActiveMobileThread,
} from '@/hooks/gateway-chat-utils';

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

function emptyThread(chatId: string, threadId: string): ActiveMobileThread {
  return { chatId, threadId, title: 'Test', items: [], running: false };
}

// ---------------------------------------------------------------------------
// Mock-based tests (always run)
// ---------------------------------------------------------------------------

describe('gateway chat — mock transport', () => {
  let transport: MockTransport;
  let client: ReturnType<typeof createChatClient>;

  beforeEach(() => {
    transport = new MockTransport({
      handlers: {
        'chat.list': () =>
          chatListResponse([
            { chatId: 'c1', threadId: 't1' },
            { chatId: 'c2', threadId: 't2' },
          ]),
        'chat.create': () => chatCreateResponse('c3', 't3'),
        'chat.thread.read': () =>
          chatThreadReadResponse({
            id: 't1',
            preview: 'Hello world',
            turns: [
              {
                id: 'turn-1',
                items: [
                  { id: 'item-1', type: 'userMessage', content: [{ type: 'text', text: 'Hi' }] },
                  { id: 'item-2', type: 'agentMessage', text: 'Hello!' },
                ],
              },
            ],
          }),
        'chat.message.send': (_m, params) => {
          const p = params as Record<string, unknown>;
          return chatSendMessageResponse(p.chat_id as string, 'turn-new');
        },
        'events.subscribe': () => ({ ok: true }),
        'chat.model.list': () => ({ data: [] }),
        'chat.skills.list': () => ({ skills: [] }),
        'chat.collaboration.mode.list': () => ({ data: [] }),
        'chat.account.list': () => ({ providers: [] }),
        'terminal.session.list': () => ({ sessions: [] }),
        'terminal.tmux.list': () => ({ supported: false, sessions: [] }),
      },
    });
    client = createChatClient(transport);
  });

  // ---- List threads ----

  it('list threads returns array of summaries', async () => {
    const records = await client.list();
    expect(records).toHaveLength(2);
    expect(records[0].chat_id).toBe('c1');

    const summaries = buildChatThreadSummaries(records);
    expect(summaries).toHaveLength(2);
    expect(summaries[0].chatId).toBe('c1');
    expect(summaries[0].threadId).toBe('t1');
  });

  // ---- Create chat ----

  it('create chat returns chatId and threadId', async () => {
    const result = await client.create();
    expect(result.chatId).toBe('c3');
    expect(result.threadId).toBe('t3');
  });

  // ---- Read thread ----

  it('readThread returns items from turns', async () => {
    const result = await client.readThread('c1', 't1', true);
    expect(result.thread).not.toBeNull();
  });

  // ---- Send message ----

  it('sendMessage returns turn reference', async () => {
    const result = await client.sendMessage({
      chatId: 'c1',
      message: 'Hello from test',
    });
    expect(result.chatId).toBe('c1');
    expect(result.turnId).toBe('turn-new');
  });

  // ---- Streaming events ----

  describe('streaming events via mapChatEvent', () => {
    const threadId = 't1';
    const turnId = 'turn-1';
    const messageBuffer = new Map<string, string>();
    const threadIdLookup = new Map([['t1', 'c1']]);

    it('turn.started / turn.completed cycle', () => {
      const started = mapChatEvent(
        { topic: 'chat.turn.started', params: { thread_id: threadId, turn_id: turnId } },
        { threadIdLookup },
      );
      expect(started).not.toBeNull();
      expect(started!.type).toBe('turn.started');
      expect(started!.chatId).toBe('c1');

      const completed = mapChatEvent(
        { topic: 'chat.turn.completed', params: { thread_id: threadId, turn_id: turnId } },
        { threadIdLookup },
      );
      expect(completed).not.toBeNull();
      expect(completed!.type).toBe('turn.completed');
    });

    it('message.delta accumulates text', () => {
      const delta1 = mapChatEvent(
        { topic: 'chat.message.delta', params: { thread_id: threadId, turn_id: turnId, delta: 'Hel', item_id: 'msg-1' } },
        { threadIdLookup, messageBuffer },
      ) as ChatMappedEvent & { type: 'message.delta' };
      expect(delta1.text).toBe('Hel');

      const delta2 = mapChatEvent(
        { topic: 'chat.message.delta', params: { thread_id: threadId, turn_id: turnId, delta: 'lo!', item_id: 'msg-1' } },
        { threadIdLookup, messageBuffer },
      ) as ChatMappedEvent & { type: 'message.delta' };
      expect(delta2.text).toBe('Hello!');
    });

    it('item.started with commandExecution produces a command item', () => {
      const event = mapChatEvent(
        {
          topic: 'chat.item.started',
          params: {
            thread_id: threadId,
            turn_id: turnId,
            item: { id: 'cmd-1', type: 'commandExecution', command: 'ls -la', cwd: '/tmp' },
          },
        },
        { threadIdLookup },
      );
      expect(event).not.toBeNull();
      expect(event!.type).toBe('item.started');
      if (event!.type === 'item.started') {
        expect(event!.item.kind).toBe('command');
        expect(event!.item.command).toBe('ls -la');
      }
    });

    it('item.started with mcpToolCall produces a tool item', () => {
      const event = mapChatEvent(
        {
          topic: 'chat.item.started',
          params: {
            thread_id: threadId,
            turn_id: turnId,
            item: { id: 'tool-1', type: 'mcpToolCall', tool: 'web_fetch', status: 'running' },
          },
        },
        { threadIdLookup },
      );
      expect(event).not.toBeNull();
      if (event!.type === 'item.started') {
        expect(event!.item.kind).toBe('tool');
        expect(event!.item.text).toBe('web_fetch');
      }
    });

    it('command.output appends to thread items', () => {
      const event = mapChatEvent(
        {
          topic: 'chat.command.output',
          params: { thread_id: threadId, turn_id: turnId, delta: 'file.txt\n', item_id: 'cmd-1' },
        },
        { threadIdLookup },
      );
      expect(event).not.toBeNull();
      expect(event!.type).toBe('command.output');
    });

    it('plan.updated produces plan text', () => {
      const event = mapChatEvent(
        {
          topic: 'chat.plan.updated',
          params: {
            thread_id: threadId,
            turn_id: turnId,
            explanation: 'Building feature',
            plan: [{ step: 'Step 1', status: 'done' }, { step: 'Step 2', status: 'pending' }],
          },
        },
        { threadIdLookup },
      );
      expect(event).not.toBeNull();
      if (event!.type === 'plan.updated') {
        expect(event!.plan).toHaveLength(2);
        expect(event!.text).toContain('Step 1');
      }
    });
  });

  // ---- applyMappedEventToThread ----

  describe('applyMappedEventToThread', () => {
    it('turn.started sets running = true', () => {
      const thread = emptyThread('c1', 't1');
      const event = mapChatEvent(
        { topic: 'chat.turn.started', params: { thread_id: 't1', turn_id: 'turn-1' } },
        { threadIdLookup: new Map([['t1', 'c1']]) },
      )!;

      const updated = applyMappedEventToThread(thread, event);
      expect(updated.running).toBe(true);
      expect(updated.activeTurnId).toBe('turn-1');
    });

    it('turn.completed sets running = false', () => {
      const thread: ActiveMobileThread = {
        ...emptyThread('c1', 't1'),
        running: true,
        activeTurnId: 'turn-1',
      };
      const event = mapChatEvent(
        { topic: 'chat.turn.completed', params: { thread_id: 't1', turn_id: 'turn-1' } },
        { threadIdLookup: new Map([['t1', 'c1']]) },
      )!;

      const updated = applyMappedEventToThread(thread, event);
      expect(updated.running).toBe(false);
      expect(updated.activeTurnId).toBeUndefined();
    });

    it('message.delta adds/updates assistant item', () => {
      const thread = emptyThread('c1', 't1');
      const event = mapChatEvent(
        { topic: 'chat.message.delta', params: { thread_id: 't1', turn_id: 'turn-1', delta: 'Hi' } },
        { threadIdLookup: new Map([['t1', 'c1']]) },
      )!;

      const updated = applyMappedEventToThread(thread, event);
      expect(updated.items.length).toBeGreaterThan(0);
      const assistantItem = updated.items.find((i) => i.kind === 'assistant');
      expect(assistantItem).toBeDefined();
      expect(assistantItem!.text).toBe('Hi');
    });

    it('item.started adds command item', () => {
      const thread = emptyThread('c1', 't1');
      const event = mapChatEvent(
        {
          topic: 'chat.item.started',
          params: {
            thread_id: 't1',
            turn_id: 'turn-1',
            item: { id: 'cmd-1', type: 'commandExecution', command: 'echo hello' },
          },
        },
        { threadIdLookup: new Map([['t1', 'c1']]) },
      )!;

      const updated = applyMappedEventToThread(thread, event);
      const cmdItem = updated.items.find((i) => i.kind === 'command');
      expect(cmdItem).toBeDefined();
      expect(cmdItem!.command).toBe('echo hello');
    });
  });

  // ---- MockTransport event emission ----

  describe('MockTransport event emission', () => {
    it('emitEvent dispatches to listeners', () => {
      const events: RpcEvent[] = [];
      transport.onEvent((e) => events.push(e));

      transport.emitEvent(turnStartedEvent('t1', 'turn-1'));
      transport.emitEvent(messageDeltaEvent('t1', 'turn-1', 'Hello'));
      transport.emitEvent(turnCompletedEvent('t1', 'turn-1'));

      expect(events).toHaveLength(3);
      expect(events[0].topic).toBe('chat.turn.started');
      expect(events[1].topic).toBe('chat.message.delta');
      expect(events[2].topic).toBe('chat.turn.completed');
    });

    it('full streaming cycle: start → deltas → item → complete', () => {
      const events: RpcEvent[] = [];
      transport.onEvent((e) => events.push(e));

      transport.emitEvent(turnStartedEvent('t1', 'turn-1'));
      transport.emitEvent(
        itemStartedEvent('t1', 'turn-1', { id: 'msg-1', type: 'agentMessage', text: '' }),
      );
      transport.emitEvent(messageDeltaEvent('t1', 'turn-1', 'He', 'msg-1'));
      transport.emitEvent(messageDeltaEvent('t1', 'turn-1', 'llo', 'msg-1'));
      transport.emitEvent(
        itemCompletedEvent('t1', 'turn-1', { id: 'msg-1', type: 'agentMessage', text: 'Hello' }),
      );
      transport.emitEvent(turnCompletedEvent('t1', 'turn-1'));

      expect(events).toHaveLength(6);
    });

    it('tool events are properly typed', () => {
      const events: RpcEvent[] = [];
      transport.onEvent((e) => events.push(e));

      transport.emitEvent(
        itemStartedEvent('t1', 'turn-1', {
          id: 'tool-1',
          type: 'mcpToolCall',
          tool: 'web_search',
          status: 'running',
        }),
      );
      transport.emitEvent(commandOutputEvent('t1', 'turn-1', 'result data', 'cmd-1'));
      transport.emitEvent(planUpdatedEvent('t1', 'turn-1', 'Planning', [{ step: 'analyze', status: 'done' }]));

      // Verify they parse correctly
      const threadIdLookup = new Map([['t1', 'c1']]);
      const mapped = events
        .map((e) => mapChatEvent({ topic: e.topic, params: e.params }, { threadIdLookup }))
        .filter(Boolean);

      expect(mapped).toHaveLength(3);
      expect(mapped[0]!.type).toBe('item.started');
      expect(mapped[1]!.type).toBe('command.output');
      expect(mapped[2]!.type).toBe('plan.updated');
    });
  });
});

// ---------------------------------------------------------------------------
// Live tests (only when GATEWAY_URL is set)
// ---------------------------------------------------------------------------

describe.skipIf(!LIVE_TEST_ENABLED)('gateway chat — live', () => {
  it('connects to gateway and lists threads', async () => {
    const { GatewayTransport, createChatClient: createClient } = await import('@homie/shared');
    const transport = new GatewayTransport({
      url: GATEWAY_URL,
      clientId: 'homie-mobile-test/0.1.0',
      capabilities: ['chat'],
      reconnect: false,
    });

    const connected = new Promise<void>((resolve, reject) => {
      const timeout = setTimeout(() => reject(new Error('Connection timeout')), 10_000);
      transport.onStateChange((state) => {
        if (state.status === 'connected') {
          clearTimeout(timeout);
          resolve();
        }
        if (state.status === 'error' || state.status === 'rejected') {
          clearTimeout(timeout);
          reject(new Error(`Connection failed: ${state.status}`));
        }
      });
    });

    transport.start();
    await connected;

    const client = createClient(transport);
    const records = await client.list();
    expect(Array.isArray(records)).toBe(true);

    transport.stop();
  }, 15_000);
});
