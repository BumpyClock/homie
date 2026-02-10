import { type ChatAgentMode, type ChatEffort, type ChatPermissionMode, type ChatSettings, type ChatThreadRecord, type ChatThreadSummary, type ChatWebToolName, type CollaborationModeOption, type FileOption, type ModelOption, type SkillOption } from "./chat-types";
export type ChatRpcCall = <TResult = unknown>(method: string, params?: unknown) => Promise<TResult>;
export interface ChatRpcTransport {
    call: ChatRpcCall;
}
export type ChatApprovalPolicy = "never" | "on-request" | "untrusted";
export type ChatApprovalDecision = "accept" | "decline" | "accept_for_session" | "cancel";
export interface ChatCollaborationPayload {
    mode: string;
    settings?: Record<string, unknown>;
}
export interface ChatTurnResult {
    chatId?: string;
    turnId?: string;
    queued?: boolean;
    raw: unknown;
}
export interface ChatCreateResult {
    chatId?: string;
    threadId?: string;
    raw: unknown;
}
export interface ChatThreadReadResult {
    thread: Record<string, unknown> | null;
    settings: Partial<ChatSettings> | null;
    raw: unknown;
}
export interface SendChatMessageInput {
    chatId: string;
    message: string;
    model?: string;
    effort?: ChatEffort;
    permission?: ChatPermissionMode;
    approvalPolicy?: ChatApprovalPolicy;
    collaborationMode?: ChatCollaborationPayload | null;
    inject?: boolean;
}
export interface CancelChatTurnInput {
    chatId: string;
    turnId: string;
}
export interface ArchiveChatThreadInput {
    chatId: string;
    threadId: string;
}
export interface RenameChatThreadInput {
    chatId: string;
    title: string;
    threadId?: string;
}
export interface RespondToApprovalInput {
    requestId: number | string;
    decision: ChatApprovalDecision;
}
export interface SearchChatFilesInput {
    chatId: string;
    query: string;
    limit?: number;
    basePath?: string | null;
}
export interface ChatSettingsPatch {
    model?: string | null;
    effort?: ChatEffort | null;
    permission?: ChatPermissionMode | null;
    approvalPolicy?: ChatApprovalPolicy | null;
    agentMode?: ChatAgentMode | null;
    collaborationMode?: ChatCollaborationPayload | null;
    attachedFolder?: string | null;
}
export interface UpdateChatSettingsInput {
    chatId: string;
    settings: ChatSettingsPatch | Record<string, unknown>;
}
export interface BuildThreadSummaryOptions {
    overrides?: Record<string, string>;
    runningByChatId?: ReadonlyMap<string, string>;
    now?: () => number;
}
export interface BuildCollaborationPayloadInput {
    settings: Pick<ChatSettings, "agentMode" | "model" | "effort">;
    collaborationModes: CollaborationModeOption[];
    defaultModel?: string;
}
export interface ChatClient {
    create(): Promise<ChatCreateResult>;
    list(): Promise<ChatThreadRecord[]>;
    resume(chatId: string, threadId?: string): Promise<ChatCreateResult>;
    readThread(chatId: string, threadId?: string, includeTurns?: boolean): Promise<ChatThreadReadResult>;
    sendMessage(input: SendChatMessageInput): Promise<ChatTurnResult>;
    cancel(input: CancelChatTurnInput): Promise<unknown>;
    archiveThread(input: ArchiveChatThreadInput): Promise<unknown>;
    renameThread(input: RenameChatThreadInput): Promise<unknown>;
    respondApproval(input: RespondToApprovalInput): Promise<unknown>;
    readAccount(): Promise<Record<string, unknown>>;
    listModels(): Promise<ModelOption[]>;
    listCollaborationModes(): Promise<CollaborationModeOption[]>;
    listSkills(): Promise<SkillOption[]>;
    listTools(channel?: string): Promise<ChatWebToolName[]>;
    updateSettings(input: UpdateChatSettingsInput): Promise<unknown>;
    searchFiles(input: SearchChatFilesInput): Promise<FileOption[]>;
}
export declare function approvalPolicyForPermission(permission: ChatPermissionMode): ChatApprovalPolicy;
export declare function buildCollaborationPayload({ settings, collaborationModes, defaultModel, }: BuildCollaborationPayloadInput): ChatCollaborationPayload | null;
export declare function normalizeChatThreadRecords(raw: unknown): ChatThreadRecord[];
export declare function buildChatThreadSummaries(records: ChatThreadRecord[], options?: BuildThreadSummaryOptions): ChatThreadSummary[];
export declare function normalizeEnabledWebTools(raw: unknown): ChatWebToolName[];
export declare function normalizeModelOptions(raw: unknown): ModelOption[];
export declare function normalizeCollaborationModes(raw: unknown): CollaborationModeOption[];
export declare function normalizeSkillOptions(raw: unknown): SkillOption[];
export declare function normalizeFileOptions(raw: unknown): FileOption[];
export declare function normalizeChatSettings(raw: unknown): Partial<ChatSettings> | null;
export declare function parseThreadReadResult(raw: unknown): ChatThreadReadResult;
export declare function createChatClient(transport: ChatRpcCall | ChatRpcTransport): ChatClient;
export declare function deriveChatPreview(message: string, limit?: number): string;
//# sourceMappingURL=chat-client.d.ts.map