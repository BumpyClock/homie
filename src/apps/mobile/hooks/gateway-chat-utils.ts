import {
  shortId,
  type ChatApprovalDecision,
  type ChatItem,
  type ChatMappedEvent,
  type ChatThreadSummary,
  type ConnectionStatus,
} from '@homie/shared';

export type StatusTone = 'accent' | 'success' | 'warning';

export interface StatusBadgeState {
  label: string;
  tone: StatusTone;
}

export interface ActiveMobileThread {
  chatId: string;
  threadId: string;
  title: string;
  items: ChatItem[];
  running: boolean;
  activeTurnId?: string;
}

export interface PendingApprovalMetadata {
  itemId: string;
  requestId: number | string;
  reason?: string;
  command?: string;
  cwd?: string;
  status: string;
}

function fallbackOutputId(turnId: string | undefined, count: number): string {
  return `output-${turnId ?? 'thread'}-${count + 1}`;
}

export function fallbackThreadTitle(chatId: string) {
  return `Chat ${shortId(chatId)}`;
}

function asTimestampSeconds(value: unknown): number | undefined {
  if (typeof value === 'number' && Number.isFinite(value)) return value;
  if (typeof value === 'string') {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return undefined;
}

export function threadLastActivityAt(
  thread: Record<string, unknown> | null,
  fallback: number,
): number {
  if (!thread) return fallback;
  const updatedAt = asTimestampSeconds(thread.updated_at);
  if (updatedAt !== undefined) return updatedAt * 1000;
  const createdAt = asTimestampSeconds(thread.created_at);
  if (createdAt !== undefined) return createdAt * 1000;
  return fallback;
}

export { previewFromItems, sortThreads } from '@homie/shared';

export function statusBadgeFor(status: ConnectionStatus): StatusBadgeState {
  if (status === 'connected') return { label: 'Connected', tone: 'success' };
  if (status === 'connecting' || status === 'handshaking') {
    return { label: 'Connecting', tone: 'accent' };
  }
  if (status === 'rejected') return { label: 'Rejected', tone: 'warning' };
  if (status === 'error') return { label: 'Error', tone: 'warning' };
  return { label: 'Disconnected', tone: 'warning' };
}

export function formatError(error: unknown): string {
  if (error instanceof Error && error.message.trim()) return error.message;
  if (typeof error === 'string' && error.trim()) return error;
  return 'Gateway request failed';
}

function upsertItem(items: ChatItem[], item: ChatItem): ChatItem[] {
  const index = items.findIndex((entry) => entry.id === item.id);
  if (index === -1) return [...items, item];
  const next = [...items];
  next[index] = { ...next[index], ...item };
  return next;
}

function findAssistantIndexForTurn(items: ChatItem[], turnId: string): number {
  return items.findIndex((entry) => entry.turnId === turnId && entry.kind === 'assistant');
}

function insertBeforeTurnAssistant(items: ChatItem[], item: ChatItem): ChatItem[] {
  if (!item.turnId || item.kind === 'assistant') {
    return [...items, item];
  }
  const assistantIndex = findAssistantIndexForTurn(items, item.turnId);
  if (assistantIndex < 0) {
    return [...items, item];
  }
  return [...items.slice(0, assistantIndex), item, ...items.slice(assistantIndex)];
}

function upsertItemWithTurnOrder(items: ChatItem[], item: ChatItem): ChatItem[] {
  const index = items.findIndex((entry) => entry.id === item.id);
  if (index === -1) {
    return insertBeforeTurnAssistant(items, item);
  }

  const next = [...items];
  const merged = { ...next[index], ...item };
  next[index] = merged;

  if (!merged.turnId || merged.kind === 'assistant') {
    return next;
  }

  const assistantIndex = findAssistantIndexForTurn(next, merged.turnId);
  if (assistantIndex < 0 || index < assistantIndex) {
    return next;
  }

  const [moved] = next.splice(index, 1);
  const targetIndex = findAssistantIndexForTurn(next, merged.turnId);
  if (targetIndex < 0) {
    next.push(moved);
    return next;
  }
  next.splice(targetIndex, 0, moved);
  return next;
}

function mapOutputToItems(
  items: ChatItem[],
  itemId: string | undefined,
  delta: string,
  turnId: string | undefined,
): ChatItem[] {
  if (!itemId) {
    if (!delta) return items;
    return insertBeforeTurnAssistant(items, {
      id: fallbackOutputId(turnId, items.length),
      kind: 'system',
      turnId,
      text: delta,
    });
  }

  let updated = false;
  const next = items.map((entry) => {
    if (entry.id !== itemId) return entry;
    updated = true;
    if (entry.kind === 'command') {
      return {
        ...entry,
        output: `${entry.output ?? ''}${delta}`,
      };
    }
    return {
      ...entry,
      text: `${entry.text ?? ''}${delta}`,
    };
  });

  if (updated) return next;
  if (!delta) return items;

  return insertBeforeTurnAssistant(items, {
    id: itemId,
    kind: 'system',
    turnId,
    text: delta,
  });
}

export function applyMappedEventToThread(
  thread: ActiveMobileThread,
  mapped: ChatMappedEvent,
): ActiveMobileThread {
  let nextItems = thread.items;
  let running = thread.running;
  let activeTurnId = thread.activeTurnId;

  if (mapped.type === 'turn.started') {
    running = true;
    activeTurnId = mapped.turnId;
  }

  if (mapped.type === 'turn.completed') {
    running = false;
    if (!mapped.turnId || activeTurnId === mapped.turnId) activeTurnId = undefined;
  }

  if (mapped.type === 'item.started' || mapped.type === 'item.completed') {
    nextItems = upsertItemWithTurnOrder(nextItems, mapped.item);
  }

  if (mapped.type === 'message.delta') {
    const id = mapped.itemId ?? `assistant-${mapped.turnId ?? mapped.threadId}`;
    nextItems = upsertItemWithTurnOrder(nextItems, {
      id,
      kind: 'assistant',
      role: 'assistant',
      turnId: mapped.turnId,
      text: mapped.text,
    });
  }

  if (mapped.type === 'command.output' || mapped.type === 'file.output') {
    nextItems = mapOutputToItems(nextItems, mapped.itemId, mapped.delta, mapped.turnId);
  }

  if (mapped.type === 'diff.updated') {
    const id = `diff-${mapped.turnId ?? mapped.threadId}`;
    nextItems = upsertItemWithTurnOrder(nextItems, {
      id,
      kind: 'diff',
      turnId: mapped.turnId,
      text: mapped.diff,
    });
  }

  if (mapped.type === 'plan.updated') {
    const id = `plan-${mapped.turnId ?? mapped.threadId}`;
    nextItems = upsertItemWithTurnOrder(nextItems, {
      id,
      kind: 'plan',
      turnId: mapped.turnId,
      text: mapped.text,
    });
  }

  if (mapped.type === 'approval.required') {
    nextItems = upsertItemWithTurnOrder(nextItems, {
      id: mapped.itemId,
      kind: 'approval',
      turnId: mapped.turnId,
      requestId: mapped.requestId,
      reason: mapped.reason,
      command: mapped.command,
      cwd: mapped.cwd,
      status: 'pending',
    });
  }

  return {
    ...thread,
    threadId: mapped.threadId,
    items: nextItems,
    running,
    activeTurnId,
  };
}

function requestIdMatches(left: number | string, right: number | string): boolean {
  return String(left) === String(right);
}

export function applyApprovalStatusToThread(
  thread: ActiveMobileThread,
  requestId: number | string,
  status: string,
): ActiveMobileThread {
  let changed = false;
  const nextItems = thread.items.map((item) => {
    if (item.kind !== 'approval' || item.requestId === undefined) return item;
    if (!requestIdMatches(item.requestId, requestId)) return item;
    if (item.status === status) return item;
    changed = true;
    return {
      ...item,
      status,
    };
  });
  if (!changed) return thread;
  return {
    ...thread,
    items: nextItems,
  };
}

export function applyApprovalDecisionToThread(
  thread: ActiveMobileThread,
  requestId: number | string,
  decision: ChatApprovalDecision,
): ActiveMobileThread {
  return applyApprovalStatusToThread(thread, requestId, decision);
}

export function countPendingApprovals(items: ChatItem[]): number {
  let count = 0;
  for (const item of items) {
    if (item.kind === 'approval' && (!item.status || item.status === 'pending')) {
      count += 1;
    }
  }
  return count;
}

export function pendingApprovalFromThread(
  thread: ActiveMobileThread | null,
): PendingApprovalMetadata | null {
  if (!thread) return null;
  for (let index = thread.items.length - 1; index >= 0; index -= 1) {
    const item = thread.items[index];
    if (item.kind !== 'approval' || item.requestId === undefined) continue;
    const status = item.status ?? 'pending';
    if (status !== 'pending') continue;
    return {
      itemId: item.id,
      requestId: item.requestId,
      reason: item.reason,
      command: item.command,
      cwd: item.cwd,
      status,
    };
  }
  return null;
}
