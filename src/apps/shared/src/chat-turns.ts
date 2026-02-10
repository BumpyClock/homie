import type { ChatItem } from "./chat-types";

/**
 * Canonical tool names emitted by current chat tool calls.
 */
export type ChatToolName =
  | "exec"
  | "process"
  | "read"
  | "ls"
  | "find"
  | "grep"
  | "apply_patch"
  | "web_search"
  | "web_fetch";

/**
 * Friendly tool labels for tool chips/cards in chat UIs.
 */
export const FRIENDLY_CHAT_TOOL_LABELS: Record<ChatToolName, string> = {
  exec: "Run command",
  process: "Run process",
  read: "Read file",
  ls: "List files",
  find: "Find files",
  grep: "Search text",
  apply_patch: "Apply patch",
  web_search: "Search web",
  web_fetch: "Fetch page",
};

export const FRIENDLY_TOOL_LABELS = FRIENDLY_CHAT_TOOL_LABELS;

const CHAT_TOOL_ALIASES: Readonly<Record<string, ChatToolName>> = {
  exec_command: "exec",
  run_command: "exec",
  run_process: "process",
  read_file: "read",
  list_files: "ls",
  file_search: "find",
  search_files: "find",
  applypatch: "apply_patch",
  websearch: "web_search",
  webfetch: "web_fetch",
};

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

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}

function toSnakeCase(value: string): string {
  return value
    .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
    .replace(/[\s-]+/g, "_")
    .toLowerCase();
}

function stripToolNamespace(toolName: string): string {
  const segments = toolName.split(/[./:]/).filter(Boolean);
  if (segments.length === 0) return toolName;
  return segments[segments.length - 1];
}

/**
 * Normalizes a raw tool name into one of the canonical chat tool names.
 */
export function normalizeChatToolName(toolName: string | null | undefined): ChatToolName | undefined {
  if (!toolName || !toolName.trim()) return undefined;
  const stripped = stripToolNamespace(toolName.trim());
  const normalized = toSnakeCase(stripped).replace(/^mcp_+/, "");
  const resolved = CHAT_TOOL_ALIASES[normalized] ?? normalized;
  if (resolved in FRIENDLY_CHAT_TOOL_LABELS) {
    return resolved as ChatToolName;
  }
  return undefined;
}

/**
 * Maps a raw tool name to a user-friendly label.
 */
export function friendlyChatToolLabel(
  toolName: string | null | undefined,
  fallback = "Tool call",
): string {
  const normalized = normalizeChatToolName(toolName);
  if (!normalized) return fallback;
  return FRIENDLY_CHAT_TOOL_LABELS[normalized];
}

export function friendlyToolLabel(toolName: string | null | undefined, fallback = "Tool call"): string {
  return friendlyChatToolLabel(toolName, fallback);
}

/**
 * Resolves the raw tool name from a chat item when available.
 */
export function rawToolNameFromItem(item: Pick<ChatItem, "kind" | "text" | "raw">): string | undefined {
  if (item.kind !== "tool") return undefined;
  if (isRecord(item.raw) && typeof item.raw.tool === "string" && item.raw.tool.trim()) {
    return item.raw.tool;
  }
  if (typeof item.text === "string" && item.text.trim()) {
    return item.text;
  }
  return undefined;
}

/**
 * Friendly tool label for a tool chat item.
 */
export function friendlyToolLabelFromItem(
  item: Pick<ChatItem, "kind" | "text" | "raw">,
  fallback = "Tool call",
): string {
  return friendlyChatToolLabel(rawToolNameFromItem(item), fallback);
}

/**
 * Returns true for explicit user-message items.
 */
export function isUserChatItem(item: Pick<ChatItem, "kind">): boolean {
  return item.kind === "user";
}

/**
 * Returns true for assistant-response items.
 */
export function isAssistantChatItem(item: Pick<ChatItem, "kind">): boolean {
  return item.kind === "assistant";
}

/**
 * Returns true for non-user/non-assistant activity items.
 */
export function isActivityChatItem(item: Pick<ChatItem, "kind">): boolean {
  return !isUserChatItem(item) && !isAssistantChatItem(item);
}

/**
 * Groups chat items by turn while preserving input order.
 */
export function groupChatItemsByTurn(items: readonly ChatItem[]): ChatTurnGroup[] {
  const groups = new Map<string, ChatTurnGroup>();
  const order: string[] = [];

  for (const item of items) {
    const key = item.turnId ? `turn:${item.turnId}` : `item:${item.id}`;
    let group = groups.get(key);
    if (!group) {
      group = {
        id: item.turnId ?? item.id,
        turnId: item.turnId,
        items: [],
        userItems: [],
        assistantItems: [],
        activityItems: [],
      };
      groups.set(key, group);
      order.push(key);
    }

    group.items.push(item);
    if (isUserChatItem(item)) {
      group.userItems.push(item);
      continue;
    }
    if (isAssistantChatItem(item)) {
      group.assistantItems.push(item);
      continue;
    }
    group.activityItems.push(item);
  }

  return order.map((key) => groups.get(key)).filter((group): group is ChatTurnGroup => Boolean(group));
}

export function groupTurns(items: readonly ChatItem[]): ChatTurnGroup[] {
  return groupChatItemsByTurn(items);
}
