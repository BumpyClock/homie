export type ChatStatus = "active" | "inactive" | "exited";
export type ChatItemKind = "user" | "assistant" | "plan" | "reasoning" | "command" | "file" | "diff" | "approval" | "tool" | "system";
export type ChatPermissionMode = "explore" | "ask" | "execute";
export type ChatAgentMode = "code" | "plan";
export type ChatEffort = "auto" | "none" | "minimal" | "low" | "medium" | "high" | "xhigh";
export type ChatWebToolName = "web_fetch" | "web_search";
export interface ChatSettings {
    model?: string;
    effort: ChatEffort;
    permission: ChatPermissionMode;
    agentMode: ChatAgentMode;
    attachedFolder?: string;
}
export interface ChatItemChange {
    path: string;
    diff: string;
    kind?: string;
}
export interface ChatItem {
    id: string;
    kind: ChatItemKind;
    text?: string;
    summary?: string[];
    content?: string[];
    command?: string;
    cwd?: string;
    output?: string;
    changes?: ChatItemChange[];
    status?: string;
    turnId?: string;
    role?: "user" | "assistant";
    requestId?: number | string;
    reason?: string;
    optimistic?: boolean;
    raw?: unknown;
}
export interface ChatThreadSummary {
    chatId: string;
    threadId: string;
    title: string;
    preview: string;
    status: ChatStatus;
    lastActivityAt?: number;
    running: boolean;
}
export interface ActiveChatThread {
    chatId: string;
    threadId: string;
    title: string;
    items: ChatItem[];
    running: boolean;
    activeTurnId?: string;
}
export interface ReasoningEffortOption {
    reasoningEffort: string;
    description: string;
}
export interface ModelOption {
    id: string;
    model: string;
    displayName: string;
    description: string;
    supportedReasoningEfforts: ReasoningEffortOption[];
    defaultReasoningEffort: string | null;
    isDefault: boolean;
}
export interface SkillOption {
    name: string;
    description?: string;
    path?: string;
}
export interface FileOption {
    name: string;
    path: string;
    relativePath: string;
    type: "file" | "directory";
}
export interface CollaborationModeOption {
    id: string;
    label: string;
    mode?: string | null;
    model?: string | null;
    reasoningEffort?: string | null;
    developerInstructions?: string | null;
}
export interface ThreadTokenUsageSnapshot {
    totalTokens: number;
    inputTokens: number;
    cachedInputTokens: number;
    outputTokens: number;
    reasoningOutputTokens: number;
}
export interface ThreadTokenUsage {
    total: ThreadTokenUsageSnapshot;
    last: ThreadTokenUsageSnapshot;
    modelContextWindow: number | null;
}
export interface ChatThreadRecord {
    chat_id: string;
    thread_id: string;
    created_at: string;
    status: ChatStatus;
    settings?: unknown;
}
export interface ChatGatewayEvent {
    topic: string;
    params?: unknown;
}
export declare function createLocalId(prefix?: string): string;
export declare function shortId(id: string): string;
export declare function truncateText(text: string, limit: number): string;
export declare function parseCreatedAt(value?: string): number | undefined;
export declare function formatRelativeTime(timestamp?: number, now?: number): string;
export declare function getThreadId(params: Record<string, unknown> | undefined): string | undefined;
export declare function getTurnId(params: Record<string, unknown> | undefined): string | undefined;
export declare function getItemId(params: Record<string, unknown> | undefined): string | undefined;
export declare function resolveApprovalRequestId(params: Record<string, unknown>): number | string | undefined;
export interface ItemsFromThreadOptions {
    idFactory?: () => string;
}
export declare function itemsFromThread(thread: Record<string, unknown>, options?: ItemsFromThreadOptions): ChatItem[];
export declare function extractLastMessage(items: ChatItem[]): string;
export declare function deriveTitleFromThread(thread: Record<string, unknown>, fallback: string): string;
export declare function normalizeTokenUsage(raw: Record<string, unknown>): ThreadTokenUsage;
//# sourceMappingURL=chat-types.d.ts.map