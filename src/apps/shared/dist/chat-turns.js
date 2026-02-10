/**
 * Friendly tool labels for tool chips/cards in chat UIs.
 */
export const FRIENDLY_CHAT_TOOL_LABELS = {
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
const CHAT_TOOL_ALIASES = {
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
function isRecord(value) {
    return Boolean(value) && typeof value === "object" && !Array.isArray(value);
}
function toSnakeCase(value) {
    return value
        .replace(/([a-z0-9])([A-Z])/g, "$1_$2")
        .replace(/[\s-]+/g, "_")
        .toLowerCase();
}
function stripToolNamespace(toolName) {
    const segments = toolName.split(/[./:]/).filter(Boolean);
    if (segments.length === 0)
        return toolName;
    return segments[segments.length - 1];
}
/**
 * Normalizes a raw tool name into one of the canonical chat tool names.
 */
export function normalizeChatToolName(toolName) {
    if (!toolName || !toolName.trim())
        return undefined;
    const stripped = stripToolNamespace(toolName.trim());
    const normalized = toSnakeCase(stripped).replace(/^mcp_+/, "");
    const resolved = CHAT_TOOL_ALIASES[normalized] ?? normalized;
    if (resolved in FRIENDLY_CHAT_TOOL_LABELS) {
        return resolved;
    }
    return undefined;
}
/**
 * Maps a raw tool name to a user-friendly label.
 */
export function friendlyChatToolLabel(toolName, fallback = "Tool call") {
    const normalized = normalizeChatToolName(toolName);
    if (!normalized)
        return fallback;
    return FRIENDLY_CHAT_TOOL_LABELS[normalized];
}
export function friendlyToolLabel(toolName, fallback = "Tool call") {
    return friendlyChatToolLabel(toolName, fallback);
}
/**
 * Resolves the raw tool name from a chat item when available.
 */
export function rawToolNameFromItem(item) {
    if (item.kind !== "tool")
        return undefined;
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
export function friendlyToolLabelFromItem(item, fallback = "Tool call") {
    return friendlyChatToolLabel(rawToolNameFromItem(item), fallback);
}
/**
 * Returns true for explicit user-message items.
 */
export function isUserChatItem(item) {
    return item.kind === "user";
}
/**
 * Returns true for assistant-response items.
 */
export function isAssistantChatItem(item) {
    return item.kind === "assistant";
}
/**
 * Returns true for non-user/non-assistant activity items.
 */
export function isActivityChatItem(item) {
    return !isUserChatItem(item) && !isAssistantChatItem(item);
}
/**
 * Groups chat items by turn while preserving input order.
 */
export function groupChatItemsByTurn(items) {
    const groups = new Map();
    const order = [];
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
    return order.map((key) => groups.get(key)).filter((group) => Boolean(group));
}
export function groupTurns(items) {
    return groupChatItemsByTurn(items);
}
//# sourceMappingURL=chat-turns.js.map