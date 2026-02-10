import type { ChatItem } from "./chat-types";
/**
 * Canonical tool names emitted by current chat tool calls.
 */
export type ChatToolName = "exec" | "process" | "read" | "ls" | "find" | "grep" | "apply_patch" | "web_search" | "web_fetch";
/**
 * Friendly tool labels for tool chips/cards in chat UIs.
 */
export declare const FRIENDLY_CHAT_TOOL_LABELS: Record<ChatToolName, string>;
export declare const FRIENDLY_TOOL_LABELS: Record<ChatToolName, string>;
/**
 * Per-turn chat structure with pre-separated item groups for rendering.
 */
export interface ChatTurnGroup {
    id: string;
    turnId?: string;
    items: ChatItem[];
    userItems: ChatItem[];
    assistantItems: ChatItem[];
    activityItems: ChatItem[];
}
export type TurnGroup = ChatTurnGroup;
/**
 * Normalizes a raw tool name into one of the canonical chat tool names.
 */
export declare function normalizeChatToolName(toolName: string | null | undefined): ChatToolName | undefined;
/**
 * Maps a raw tool name to a user-friendly label.
 */
export declare function friendlyChatToolLabel(toolName: string | null | undefined, fallback?: string): string;
export declare function friendlyToolLabel(toolName: string | null | undefined, fallback?: string): string;
/**
 * Resolves the raw tool name from a chat item when available.
 */
export declare function rawToolNameFromItem(item: Pick<ChatItem, "kind" | "text" | "raw">): string | undefined;
/**
 * Friendly tool label for a tool chat item.
 */
export declare function friendlyToolLabelFromItem(item: Pick<ChatItem, "kind" | "text" | "raw">, fallback?: string): string;
/**
 * Returns true for explicit user-message items.
 */
export declare function isUserChatItem(item: Pick<ChatItem, "kind">): boolean;
/**
 * Returns true for assistant-response items.
 */
export declare function isAssistantChatItem(item: Pick<ChatItem, "kind">): boolean;
/**
 * Returns true for non-user/non-assistant activity items.
 */
export declare function isActivityChatItem(item: Pick<ChatItem, "kind">): boolean;
/**
 * Groups chat items by turn while preserving input order.
 */
export declare function groupChatItemsByTurn(items: readonly ChatItem[]): ChatTurnGroup[];
export declare function groupTurns(items: readonly ChatItem[]): ChatTurnGroup[];
//# sourceMappingURL=chat-turns.d.ts.map