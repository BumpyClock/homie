import type { ChatRpcCall } from "./chat-client";
import { type ChatGatewayEvent, type ChatItem, type ThreadTokenUsage } from "./chat-types";
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
export type ChatMappedEvent = ChatTurnStartedEvent | ChatTurnCompletedEvent | ChatMessageDeltaEvent | ChatItemLifecycleEvent | ChatOutputDeltaEvent | ChatDiffUpdatedEvent | ChatPlanUpdatedEvent | ChatTokenUsageUpdatedEvent | ChatApprovalRequiredEvent;
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
export declare function buildPlanUpdateText(explanation: string, plan: ChatPlanStep[]): string;
export declare function mapChatEvent(event: ChatGatewayEvent, options?: MapChatEventOptions): ChatMappedEvent | null;
export declare function subscribeToChatEvents(call: ChatRpcCall, topic?: string): Promise<void>;
export declare function bindMappedChatEvents(source: GatewayEventSource, handler: (event: ChatMappedEvent) => void, options?: MapChatEventOptions): () => void;
export {};
//# sourceMappingURL=chat-events.d.ts.map