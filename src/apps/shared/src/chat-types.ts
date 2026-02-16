export type ChatStatus = "active" | "inactive" | "exited";

export type ChatItemKind =
  | "user"
  | "assistant"
  | "plan"
  | "reasoning"
  | "command"
  | "file"
  | "diff"
  | "approval"
  | "tool"
  | "system";

export type ChatPermissionMode = "explore" | "ask" | "execute";
export type ChatAgentMode = "code" | "plan";
export type ChatEffort = "auto" | "none" | "minimal" | "low" | "medium" | "high" | "xhigh";
export type ChatWebToolName = "web_fetch" | "web_search" | "browser";

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
  provider: string;
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

const LOCAL_ID_SEED = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 10)}`;
let localIdCounter = 0;

export function createLocalId(prefix = "chat") {
  localIdCounter += 1;
  return `${prefix}-${LOCAL_ID_SEED}-${localIdCounter.toString(36)}`;
}

export function shortId(id: string) {
  return id.slice(0, 8);
}

export function truncateText(text: string, limit: number) {
  const trimmed = text.trim();
  if (trimmed.length <= limit) return trimmed;
  return `${trimmed.slice(0, limit).trim()}...`;
}

export function parseCreatedAt(value?: string) {
  if (!value) return undefined;
  const num = Number.parseInt(value, 10);
  if (Number.isNaN(num)) return undefined;
  return num * 1000;
}

export function formatRelativeTime(timestamp?: number, now = Date.now()) {
  if (!timestamp) return "";
  const diff = now - timestamp;
  if (diff < 60_000) return "just now";
  const mins = Math.floor(diff / 60_000);
  if (mins < 60) return `${mins}m`;
  const hrs = Math.floor(mins / 60);
  if (hrs < 24) return `${hrs}h`;
  const days = Math.floor(hrs / 24);
  if (days < 7) return `${days}d`;
  return new Date(timestamp).toLocaleDateString();
}

export function getThreadId(params: Record<string, unknown> | undefined): string | undefined {
  if (!params) return undefined;
  const direct = params.threadId ?? params.thread_id;
  if (typeof direct === "string") return direct;
  const thread = params.thread;
  if (thread && typeof thread === "object") {
    const nested = (thread as Record<string, unknown>).id;
    if (typeof nested === "string") return nested;
  }
  return undefined;
}

export function getTurnId(params: Record<string, unknown> | undefined): string | undefined {
  if (!params) return undefined;
  const direct = params.turnId ?? params.turn_id;
  return typeof direct === "string" ? direct : undefined;
}

export function getItemId(params: Record<string, unknown> | undefined): string | undefined {
  if (!params) return undefined;
  const direct = params.itemId ?? params.item_id;
  return typeof direct === "string" ? direct : undefined;
}

export function resolveApprovalRequestId(
  params: Record<string, unknown>,
): number | string | undefined {
  const candidates = [
    params.codex_request_id,
    params.codexRequestId,
    params.request_id,
    params.requestId,
    params.id,
  ];
  for (const candidate of candidates) {
    if (typeof candidate === "number" || typeof candidate === "string") {
      return candidate;
    }
  }
  return undefined;
}

function extractUserText(content: unknown): string {
  if (!Array.isArray(content)) return "";
  const parts = content
    .map((entry) => {
      if (!entry || typeof entry !== "object") return "";
      const block = entry as Record<string, unknown>;
      const type = block.type;
      if (type === "text" && typeof block.text === "string") return block.text;
      if (type === "image" && typeof block.url === "string") return `[image] ${block.url}`;
      if (type === "localImage" && typeof block.path === "string") return `[image] ${block.path}`;
      if (type === "skill" && typeof block.name === "string") return `[skill] ${block.name}`;
      if (type === "mention" && typeof block.name === "string") return `@${block.name}`;
      return `[${String(type ?? "input")}]`;
    })
    .filter(Boolean);
  return parts.join("\n");
}

export interface ItemsFromThreadOptions {
  idFactory?: () => string;
}

export function itemsFromThread(
  thread: Record<string, unknown>,
  options: ItemsFromThreadOptions = {},
): ChatItem[] {
  const idFactory = options.idFactory ?? (() => createLocalId("chat-item"));
  const turns = Array.isArray(thread.turns) ? thread.turns : [];
  const items: ChatItem[] = [];

  for (const turnEntry of turns) {
    if (!turnEntry || typeof turnEntry !== "object") continue;
    const turn = turnEntry as Record<string, unknown>;
    const turnId = typeof turn.id === "string" ? turn.id : undefined;
    const turnItems = Array.isArray(turn.items) ? turn.items : [];

    for (const itemEntry of turnItems) {
      if (!itemEntry || typeof itemEntry !== "object") continue;
      const item = itemEntry as Record<string, unknown>;
      const type = item.type;
      const id = typeof item.id === "string" ? item.id : idFactory();

      if (type === "userMessage") {
        items.push({
          id,
          kind: "user",
          role: "user",
          turnId,
          text: extractUserText(item.content),
        });
        continue;
      }

      if (type === "agentMessage") {
        items.push({
          id,
          kind: "assistant",
          role: "assistant",
          turnId,
          text: typeof item.text === "string" ? item.text : "",
        });
        continue;
      }

      if (type === "plan") {
        items.push({
          id,
          kind: "plan",
          turnId,
          text: typeof item.text === "string" ? item.text : "",
        });
        continue;
      }

      if (type === "reasoning") {
        items.push({
          id,
          kind: "reasoning",
          turnId,
          summary: Array.isArray(item.summary)
            ? item.summary.filter((entry): entry is string => typeof entry === "string")
            : [],
          content: Array.isArray(item.content)
            ? item.content.filter((entry): entry is string => typeof entry === "string")
            : [],
        });
        continue;
      }

      if (type === "commandExecution") {
        items.push({
          id,
          kind: "command",
          turnId,
          command: typeof item.command === "string" ? item.command : "",
          cwd: typeof item.cwd === "string" ? item.cwd : undefined,
          output: typeof item.aggregatedOutput === "string" ? item.aggregatedOutput : undefined,
          status: typeof item.status === "string" ? item.status : undefined,
        });
        continue;
      }

      if (type === "fileChange") {
        const changes = Array.isArray(item.changes)
          ? item.changes
              .map((change): ChatItemChange | null => {
                if (!change || typeof change !== "object") return null;
                const record = change as Record<string, unknown>;
                const path = typeof record.path === "string" ? record.path : "unknown";
                const diff = typeof record.diff === "string" ? record.diff : "";
                if (!path && !diff) return null;
                const parsed: ChatItemChange = {
                  path,
                  diff,
                  kind: typeof record.kind === "string" ? record.kind : undefined,
                };
                return parsed;
              })
              .filter((change): change is ChatItemChange => change !== null)
          : [];
        items.push({
          id,
          kind: "file",
          turnId,
          status: typeof item.status === "string" ? item.status : undefined,
          changes,
        });
        continue;
      }

      if (type === "mcpToolCall") {
        items.push({
          id,
          kind: "tool",
          turnId,
          text: typeof item.tool === "string" ? item.tool : "Tool call",
          status: typeof item.status === "string" ? item.status : undefined,
          raw: item,
        });
        continue;
      }

      if (type === "webSearch") {
        items.push({
          id,
          kind: "system",
          turnId,
          text: typeof item.query === "string" ? `Web search: ${item.query}` : "Web search",
          raw: item,
        });
        continue;
      }

      if (type === "diff") {
        items.push({
          id,
          kind: "diff",
          turnId,
          text: typeof item.text === "string" ? item.text : "",
        });
      }
    }
  }

  return items;
}

export function extractLastMessage(items: ChatItem[]): string {
  let last = "";
  for (const item of items) {
    if ((item.kind === "user" || item.kind === "assistant") && item.text) {
      last = item.text;
    }
  }
  return last;
}

export function deriveTitleFromThread(thread: Record<string, unknown>, fallback: string) {
  const preview = typeof thread.preview === "string" ? thread.preview : "";
  if (preview.trim().length > 0) return truncateText(preview, 42);
  const items = itemsFromThread(thread);
  const firstMessage = items.find((item) => item.kind === "user" && item.text);
  if (firstMessage?.text) return truncateText(firstMessage.text, 42);
  return fallback;
}

function asFiniteNumber(value: unknown): number {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return 0;
}

function readModelContextWindow(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string") {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return null;
}

export function normalizeTokenUsage(raw: Record<string, unknown>): ThreadTokenUsage {
  const total = (raw.total as Record<string, unknown> | undefined) ?? {};
  const last = (raw.last as Record<string, unknown> | undefined) ?? {};

  return {
    total: {
      totalTokens: asFiniteNumber(total.totalTokens ?? total.total_tokens),
      inputTokens: asFiniteNumber(total.inputTokens ?? total.input_tokens),
      cachedInputTokens: asFiniteNumber(total.cachedInputTokens ?? total.cached_input_tokens),
      outputTokens: asFiniteNumber(total.outputTokens ?? total.output_tokens),
      reasoningOutputTokens: asFiniteNumber(
        total.reasoningOutputTokens ?? total.reasoning_output_tokens,
      ),
    },
    last: {
      totalTokens: asFiniteNumber(last.totalTokens ?? last.total_tokens),
      inputTokens: asFiniteNumber(last.inputTokens ?? last.input_tokens),
      cachedInputTokens: asFiniteNumber(last.cachedInputTokens ?? last.cached_input_tokens),
      outputTokens: asFiniteNumber(last.outputTokens ?? last.output_tokens),
      reasoningOutputTokens: asFiniteNumber(
        last.reasoningOutputTokens ?? last.reasoning_output_tokens,
      ),
    },
    modelContextWindow: readModelContextWindow(
      raw.modelContextWindow ?? raw.model_context_window,
    ),
  };
}
