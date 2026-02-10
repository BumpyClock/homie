import type { ChatRpcCall } from "./chat-client";
import {
  createLocalId,
  getItemId,
  getThreadId,
  getTurnId,
  itemsFromThread,
  normalizeTokenUsage,
  resolveApprovalRequestId,
  type ChatGatewayEvent,
  type ChatItem,
  type ThreadTokenUsage,
} from "./chat-types";

export interface ChatPlanStep {
  step: string;
  status: string;
}

interface ChatMappedEventBase {
  topic: string;
  chatId: string;
  threadId: string;
  turnId?: string;
  activityAt: number;
  rawParams: Record<string, unknown>;
}

export interface ChatTurnStartedEvent extends ChatMappedEventBase {
  type: "turn.started";
}

export interface ChatTurnCompletedEvent extends ChatMappedEventBase {
  type: "turn.completed";
}

export interface ChatMessageDeltaEvent extends ChatMappedEventBase {
  type: "message.delta";
  itemId?: string;
  delta: string;
  text: string;
}

export interface ChatItemLifecycleEvent extends ChatMappedEventBase {
  type: "item.started" | "item.completed";
  item: ChatItem;
}

export interface ChatOutputDeltaEvent extends ChatMappedEventBase {
  type: "command.output" | "file.output";
  itemId?: string;
  delta: string;
}

export interface ChatDiffUpdatedEvent extends ChatMappedEventBase {
  type: "diff.updated";
  diff: string;
}

export interface ChatPlanUpdatedEvent extends ChatMappedEventBase {
  type: "plan.updated";
  explanation: string;
  plan: ChatPlanStep[];
  text: string;
}

export interface ChatTokenUsageUpdatedEvent extends ChatMappedEventBase {
  type: "token.usage.updated";
  tokenUsage: ThreadTokenUsage;
}

export interface ChatApprovalRequiredEvent extends ChatMappedEventBase {
  type: "approval.required";
  requestId?: number | string;
  itemId: string;
  reason?: string;
  command?: string;
  cwd?: string;
}

export type ChatMappedEvent =
  | ChatTurnStartedEvent
  | ChatTurnCompletedEvent
  | ChatMessageDeltaEvent
  | ChatItemLifecycleEvent
  | ChatOutputDeltaEvent
  | ChatDiffUpdatedEvent
  | ChatPlanUpdatedEvent
  | ChatTokenUsageUpdatedEvent
  | ChatApprovalRequiredEvent;

export interface MapChatEventOptions {
  threadIdLookup?: ReadonlyMap<string, string>;
  resolveChatId?: (threadId: string) => string | undefined;
  messageBuffer?: Map<string, string>;
  idFactory?: () => string;
  now?: () => number;
}

export interface GatewayEventSource {
  onEvent(callback: (event: ChatGatewayEvent) => void): () => void;
}

export function buildPlanUpdateText(explanation: string, plan: ChatPlanStep[]) {
  const lines = [
    explanation ? `Note: ${explanation}` : "",
    ...plan.map((entry) => `- [${entry.status}] ${entry.step}`),
  ].filter(Boolean);
  return lines.join("\n");
}

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function resolveChatId(threadId: string, options: MapChatEventOptions): string {
  if (options.resolveChatId) {
    const chatId = options.resolveChatId(threadId);
    if (chatId) return chatId;
  }
  const mapped = options.threadIdLookup?.get(threadId);
  return mapped ?? threadId;
}

function resolveIdFactory(options: MapChatEventOptions): () => string {
  return options.idFactory ?? (() => createLocalId("chat-event"));
}

function resolveActivityTime(options: MapChatEventOptions): number {
  return options.now ? options.now() : Date.now();
}

function baseEvent(
  topic: string,
  chatId: string,
  threadId: string,
  params: Record<string, unknown>,
  options: MapChatEventOptions,
): ChatMappedEventBase {
  return {
    topic,
    chatId,
    threadId,
    turnId: getTurnId(params),
    activityAt: resolveActivityTime(options),
    rawParams: params,
  };
}

function mapItemEvent(
  topic: "chat.item.started" | "chat.item.completed",
  base: ChatMappedEventBase,
  params: Record<string, unknown>,
  options: MapChatEventOptions,
): ChatItemLifecycleEvent | null {
  const itemRecord = asRecord(params.item);
  if (!itemRecord) return null;
  const idFactory = resolveIdFactory(options);
  const itemId = typeof itemRecord.id === "string" ? itemRecord.id : idFactory();
  const mapped = itemsFromThread(
    {
      turns: [
        {
          id: base.turnId,
          items: [{ ...itemRecord, id: itemId }],
        },
      ],
    },
    { idFactory },
  )[0];
  if (!mapped) return null;

  if (mapped.kind === "assistant" && mapped.text && options.messageBuffer) {
    options.messageBuffer.set(mapped.id, mapped.text);
  }

  return {
    ...base,
    type: topic === "chat.item.started" ? "item.started" : "item.completed",
    item: mapped,
  };
}

function mapPlanSteps(plan: unknown): ChatPlanStep[] {
  if (!Array.isArray(plan)) return [];
  return plan
    .map((entry) => {
      const step = asRecord(entry);
      if (!step) return null;
      return {
        step: typeof step.step === "string" ? step.step : "step",
        status: typeof step.status === "string" ? step.status : "pending",
      } satisfies ChatPlanStep;
    })
    .filter((entry): entry is ChatPlanStep => entry !== null);
}

export function mapChatEvent(
  event: ChatGatewayEvent,
  options: MapChatEventOptions = {},
): ChatMappedEvent | null {
  if (!event.topic.startsWith("chat.")) return null;
  const params = asRecord(event.params) ?? {};
  const threadId = getThreadId(params);
  if (!threadId) return null;

  const chatId = resolveChatId(threadId, options);
  const base = baseEvent(event.topic, chatId, threadId, params, options);

  if (event.topic === "chat.turn.started") {
    return {
      ...base,
      type: "turn.started",
    };
  }

  if (event.topic === "chat.turn.completed") {
    return {
      ...base,
      type: "turn.completed",
    };
  }

  if (event.topic === "chat.message.delta") {
    const itemId = getItemId(params);
    const delta = typeof params.delta === "string" ? params.delta : "";
    let text = delta;
    if (itemId && options.messageBuffer) {
      text = `${options.messageBuffer.get(itemId) ?? ""}${delta}`;
      options.messageBuffer.set(itemId, text);
    }
    return {
      ...base,
      type: "message.delta",
      itemId,
      delta,
      text,
    };
  }

  if (event.topic === "chat.item.started" || event.topic === "chat.item.completed") {
    return mapItemEvent(event.topic, base, params, options);
  }

  if (event.topic === "chat.command.output" || event.topic === "chat.file.output") {
    return {
      ...base,
      type: event.topic === "chat.command.output" ? "command.output" : "file.output",
      itemId: getItemId(params),
      delta: typeof params.delta === "string" ? params.delta : "",
    };
  }

  if (event.topic === "chat.diff.updated") {
    return {
      ...base,
      type: "diff.updated",
      diff: typeof params.diff === "string" ? params.diff : "",
    };
  }

  if (event.topic === "chat.plan.updated") {
    const plan = mapPlanSteps(params.plan);
    const explanation = typeof params.explanation === "string" ? params.explanation : "";
    return {
      ...base,
      type: "plan.updated",
      explanation,
      plan,
      text: buildPlanUpdateText(explanation, plan),
    };
  }

  if (event.topic === "chat.token.usage.updated") {
    const tokenUsage = asRecord(params.tokenUsage ?? params.token_usage);
    if (!tokenUsage) return null;
    return {
      ...base,
      type: "token.usage.updated",
      tokenUsage: normalizeTokenUsage(tokenUsage),
    };
  }

  if (event.topic === "chat.approval.required") {
    return {
      ...base,
      type: "approval.required",
      requestId: resolveApprovalRequestId(params),
      itemId: getItemId(params) ?? resolveIdFactory(options)(),
      reason: typeof params.reason === "string" ? params.reason : undefined,
      command: typeof params.command === "string" ? params.command : undefined,
      cwd: typeof params.cwd === "string" ? params.cwd : undefined,
    };
  }

  return null;
}

export async function subscribeToChatEvents(call: ChatRpcCall, topic = "chat.*") {
  await call("events.subscribe", { topic });
}

export function bindMappedChatEvents(
  source: GatewayEventSource,
  handler: (event: ChatMappedEvent) => void,
  options: MapChatEventOptions = {},
) {
  return source.onEvent((event) => {
    const mapped = mapChatEvent(event, options);
    if (mapped) handler(mapped);
  });
}
