import { uuid } from "@/lib/uuid";

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
export type ChatWebToolName = "web_fetch" | "web_search";

export interface ChatSettings {
  model?: string;
  effort: ChatEffort;
  permission: ChatPermissionMode;
  agentMode: ChatAgentMode;
  attachedFolder?: string;
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
  changes?: Array<{ path: string; diff: string; kind?: string }>;
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

export interface ThreadTokenUsage {
  total: {
    totalTokens: number;
    inputTokens: number;
    cachedInputTokens: number;
    outputTokens: number;
    reasoningOutputTokens: number;
  };
  last: {
    totalTokens: number;
    inputTokens: number;
    cachedInputTokens: number;
    outputTokens: number;
    reasoningOutputTokens: number;
  };
  modelContextWindow: number | null;
}

export function shortId(id: string) {
  return id.slice(0, 8);
}

export function truncateText(text: string, limit: number) {
  const trimmed = text.trim();
  if (trimmed.length <= limit) return trimmed;
  return `${trimmed.slice(0, limit).trim()}…`;
}

export function parseCreatedAt(value?: string) {
  if (!value) return undefined;
  const num = Number.parseInt(value, 10);
  if (Number.isNaN(num)) return undefined;
  return num * 1000;
}

export function formatRelativeTime(timestamp?: number) {
  if (!timestamp) return "";
  const diff = Date.now() - timestamp;
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
  const nested = (params.thread as { id?: string } | undefined)?.id;
  if (typeof nested === "string") return nested;
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

function extractUserText(content: Array<Record<string, unknown>>): string {
  if (!Array.isArray(content)) return "";
  const parts = content.map((block) => {
    const type = block.type;
    if (type === "text" && typeof block.text === "string") return block.text;
    if (type === "image" && typeof block.url === "string") return `[image] ${block.url}`;
    if (type === "localImage" && typeof block.path === "string") return `[image] ${block.path}`;
    if (type === "skill" && typeof block.name === "string") return `[skill] ${block.name}`;
    if (type === "mention" && typeof block.name === "string") return `@${block.name}`;
    return `[${String(type ?? "input")}]`;
  });
  return parts.filter(Boolean).join("\n");
}

export function itemsFromThread(thread: Record<string, unknown>): ChatItem[] {
  const turns = Array.isArray(thread.turns) ? thread.turns : [];
  const items: ChatItem[] = [];
  for (const turn of turns) {
    const turnId = typeof turn?.id === "string" ? turn.id : undefined;
    const turnItems = Array.isArray(turn?.items) ? turn.items : [];
    for (const item of turnItems) {
      if (!item || typeof item !== "object") continue;
      const type = item.type;
      const id = typeof item.id === "string" ? item.id : uuid();
      if (type === "userMessage") {
        items.push({
          id,
          kind: "user",
          role: "user",
          turnId,
          text: extractUserText(item.content as Array<Record<string, unknown>>),
        });
      } else if (type === "agentMessage") {
        items.push({
          id,
          kind: "assistant",
          role: "assistant",
          turnId,
          text: typeof item.text === "string" ? item.text : "",
        });
      } else if (type === "plan") {
        items.push({
          id,
          kind: "plan",
          turnId,
          text: typeof item.text === "string" ? item.text : "",
        });
      } else if (type === "reasoning") {
        items.push({
          id,
          kind: "reasoning",
          turnId,
          summary: Array.isArray(item.summary) ? item.summary : [],
          content: Array.isArray(item.content) ? item.content : [],
        });
      } else if (type === "commandExecution") {
        items.push({
          id,
          kind: "command",
          turnId,
          command: typeof item.command === "string" ? item.command : "",
          cwd: typeof item.cwd === "string" ? item.cwd : undefined,
          output: typeof item.aggregatedOutput === "string" ? item.aggregatedOutput : undefined,
          status: typeof item.status === "string" ? item.status : undefined,
        });
      } else if (type === "fileChange") {
        const changes = Array.isArray(item.changes)
          ? item.changes
              .map((change: unknown) => {
                if (!change || typeof change !== "object") return null;
                const record = change as Record<string, unknown>;
                const path = typeof record.path === "string" ? record.path : "unknown";
                const diff = typeof record.diff === "string" ? record.diff : "";
                if (!path && !diff) return null;
                return { path, diff, kind: typeof record.kind === "string" ? record.kind : undefined };
              })
              .filter(Boolean) as Array<{ path: string; diff: string; kind?: string }>
          : [];
        items.push({
          id,
          kind: "file",
          turnId,
          status: typeof item.status === "string" ? item.status : undefined,
          changes,
        });
      } else if (type === "mcpToolCall") {
        items.push({
          id,
          kind: "tool",
          turnId,
          text: typeof item.tool === "string" ? item.tool : "Tool call",
          status: typeof item.status === "string" ? item.status : undefined,
          raw: item,
        });
      } else if (type === "webSearch") {
        items.push({
          id,
          kind: "system",
          turnId,
          text: typeof item.query === "string" ? `Web search: ${item.query}` : "Web search",
          raw: item,
        });
      } else if (type === "diff") {
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

function asNumber(value: unknown): number {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    if (Number.isFinite(parsed)) return parsed;
  }
  return 0;
}

export function normalizeTokenUsage(raw: Record<string, unknown>): ThreadTokenUsage {
  const total = (raw.total as Record<string, unknown>) ?? {};
  const last = (raw.last as Record<string, unknown>) ?? {};
  return {
    total: {
      totalTokens: asNumber(total.totalTokens ?? total.total_tokens),
      inputTokens: asNumber(total.inputTokens ?? total.input_tokens),
      cachedInputTokens: asNumber(total.cachedInputTokens ?? total.cached_input_tokens),
      outputTokens: asNumber(total.outputTokens ?? total.output_tokens),
      reasoningOutputTokens: asNumber(
        total.reasoningOutputTokens ?? total.reasoning_output_tokens,
      ),
    },
    last: {
      totalTokens: asNumber(last.totalTokens ?? last.total_tokens),
      inputTokens: asNumber(last.inputTokens ?? last.input_tokens),
      cachedInputTokens: asNumber(last.cachedInputTokens ?? last.cached_input_tokens),
      outputTokens: asNumber(last.outputTokens ?? last.output_tokens),
      reasoningOutputTokens: asNumber(
        last.reasoningOutputTokens ?? last.reasoning_output_tokens,
      ),
    },
    modelContextWindow: (() => {
      const value = raw.modelContextWindow ?? raw.model_context_window;
      if (typeof value === "number") return value;
      if (typeof value === "string") {
        const parsed = Number(value);
        return Number.isFinite(parsed) ? parsed : null;
      }
      return null;
    })(),
  };
}

function asString(value: unknown): string {
  return typeof value === "string" ? value : value ? String(value) : "";
}

function asBoolean(value: unknown): boolean | null {
  if (typeof value === "boolean") return value;
  if (typeof value === "number") return value !== 0;
  if (typeof value === "string") {
    const normalized = value.trim().toLowerCase();
    if (!normalized) return null;
    if (["true", "1", "enabled", "available", "active", "on", "ok"].includes(normalized)) return true;
    if (["false", "0", "disabled", "unavailable", "inactive", "off"].includes(normalized)) return false;
  }
  return null;
}

function normalizeWebToolName(value: unknown): ChatWebToolName | null {
  const normalized = asString(value).trim().toLowerCase();
  if (normalized === "web_fetch" || normalized === "web-fetch" || normalized === "webfetch") return "web_fetch";
  if (normalized === "web_search" || normalized === "web-search" || normalized === "websearch") return "web_search";
  return null;
}

export function normalizeEnabledWebTools(raw: unknown): ChatWebToolName[] {
  if (!raw || typeof raw !== "object") return [];

  const enabled = new Set<ChatWebToolName>();
  const apply = (name: ChatWebToolName, state: boolean | null) => {
    if (state === false) {
      enabled.delete(name);
      return;
    }
    enabled.add(name);
  };

  const ingestEntry = (entry: unknown) => {
    if (typeof entry === "string") {
      const name = normalizeWebToolName(entry);
      if (name) enabled.add(name);
      return;
    }
    if (!entry || typeof entry !== "object") return;
    const record = entry as Record<string, unknown>;

    const directFetch = asBoolean(record.web_fetch ?? record.webFetch);
    if (directFetch !== null) apply("web_fetch", directFetch);
    const directSearch = asBoolean(record.web_search ?? record.webSearch);
    if (directSearch !== null) apply("web_search", directSearch);

    const name = normalizeWebToolName(record.name ?? record.tool ?? record.id ?? record.slug);
    if (name) {
      const state =
        asBoolean(
          record.enabled ??
            record.is_enabled ??
            record.isEnabled ??
            record.available ??
            record.active ??
            record.status ??
            record.state,
        ) ?? true;
      apply(name, state);
    }
  };

  const root = raw as Record<string, unknown>;
  const result = (root.result as Record<string, unknown> | undefined) ?? {};
  const data = root.data;
  const dataRecord = data && typeof data === "object" && !Array.isArray(data) ? (data as Record<string, unknown>) : {};
  const candidateBuckets: unknown[] = [
    root,
    root.tools,
    root.enabled_tools,
    root.enabledTools,
    data,
    dataRecord.tools,
    dataRecord.enabled_tools,
    dataRecord.enabledTools,
    result,
    result.data,
    result.tools,
    result.enabled_tools,
    result.enabledTools,
  ];

  for (const bucket of candidateBuckets) {
    if (Array.isArray(bucket)) {
      bucket.forEach(ingestEntry);
    } else {
      ingestEntry(bucket);
    }
  }

  return ["web_fetch", "web_search"].filter((name) => enabled.has(name as ChatWebToolName)) as ChatWebToolName[];
}

export function normalizeModelOptions(raw: unknown): ModelOption[] {
  if (!raw || typeof raw !== "object") return [];
  const record = raw as Record<string, unknown>;
  let data: unknown =
    record.data ??
    (record.result as Record<string, unknown> | undefined)?.data ??
    [];
  if (data && !Array.isArray(data) && typeof data === "object") {
    data = (data as Record<string, unknown>).data ?? data;
  }
  if (!Array.isArray(data)) return [];
  return data.map((item) => {
    const model = asString((item as Record<string, unknown>).model ?? (item as Record<string, unknown>).id);
    const id = asString((item as Record<string, unknown>).id ?? model);
    const displayName = asString(
      (item as Record<string, unknown>).displayName ??
        (item as Record<string, unknown>).display_name ??
        model,
    );
    const supported = Array.isArray((item as Record<string, unknown>).supportedReasoningEfforts)
      ? ((item as Record<string, unknown>).supportedReasoningEfforts as unknown[])
      : Array.isArray((item as Record<string, unknown>).supported_reasoning_efforts)
        ? ((item as Record<string, unknown>).supported_reasoning_efforts as unknown[])
        : [];
    const supportedReasoningEfforts = supported.map((entry) => ({
      reasoningEffort: asString(
        (entry as Record<string, unknown>).reasoningEffort ??
          (entry as Record<string, unknown>).reasoning_effort,
      ),
      description: asString((entry as Record<string, unknown>).description),
    }));
    const defaultReasoningEffort = asString(
      (item as Record<string, unknown>).defaultReasoningEffort ??
        (item as Record<string, unknown>).default_reasoning_effort,
    );
    return {
      id,
      model,
      displayName,
      description: asString((item as Record<string, unknown>).description),
      supportedReasoningEfforts,
      defaultReasoningEffort: defaultReasoningEffort || null,
      isDefault: Boolean(
        (item as Record<string, unknown>).isDefault ?? (item as Record<string, unknown>).is_default,
      ),
    };
  });
}

export function normalizeCollaborationModes(raw: unknown): CollaborationModeOption[] {
  if (!raw || typeof raw !== "object") return [];
  const record = raw as Record<string, unknown>;
  let data: unknown =
    record.data ??
    (record.result as Record<string, unknown> | undefined)?.data ??
    [];
  if (data && !Array.isArray(data) && typeof data === "object") {
    data = (data as Record<string, unknown>).data ?? data;
  }
  if (!Array.isArray(data)) return [];
  return data.map((item) => {
    const entry = item as Record<string, unknown>;
    const name = asString(entry.name);
    const mode = asString(entry.mode);
    const id = name || mode;
    return {
      id: id || "mode",
      label: name || mode || "Mode",
      mode: mode || null,
      model: asString(entry.model) || null,
      reasoningEffort: asString(entry.reasoning_effort ?? entry.reasoningEffort) || null,
      developerInstructions: asString(entry.developer_instructions ?? entry.developerInstructions) || null,
    };
  });
}

export function normalizeSkillOptions(raw: unknown): SkillOption[] {
  if (!raw || typeof raw !== "object") return [];
  const record = raw as Record<string, unknown>;
  const dataBuckets = (record.result as Record<string, unknown> | undefined)?.data ?? record.data ?? [];
  const rawSkills =
    (record.result as Record<string, unknown> | undefined)?.skills ??
    record.skills ??
    (Array.isArray(dataBuckets)
      ? dataBuckets.flatMap((bucket) => (bucket as Record<string, unknown>)?.skills ?? [])
      : []);
  if (!Array.isArray(rawSkills)) return [];
  return rawSkills
    .map((item) => ({
      name: asString((item as Record<string, unknown>).name),
      path: asString((item as Record<string, unknown>).path),
      description: asString((item as Record<string, unknown>).description) || undefined,
    }))
    .filter((skill) => skill.name);
}

export function normalizeFileOptions(raw: unknown): FileOption[] {
  if (!raw || typeof raw !== "object") return [];
  const record = raw as Record<string, unknown>;
  const files = record.files ?? (record.result as Record<string, unknown> | undefined)?.files ?? [];
  if (!Array.isArray(files)) return [];
  return files
    .map((item) => {
      const entry = item as Record<string, unknown>;
      const type = asString(entry.type);
      return {
        name: asString(entry.name),
        path: asString(entry.path),
        relativePath: asString(entry.relative_path ?? entry.relativePath),
        type: type === "directory" ? "directory" : "file",
      } satisfies FileOption;
    })
    .filter((file) => file.name && file.path);
}

export function normalizeChatSettings(raw: unknown): Partial<ChatSettings> | null {
  if (!raw || typeof raw !== "object") return null;
  const record = raw as Record<string, unknown>;
  const model = asString(record.model);
  const effort = asString(record.effort);
  const approval = asString(record.approval_policy ?? record.approvalPolicy);
  const collaboration = record.collaboration_mode ?? record.collaborationMode;
  const attachments = record.attachments as Record<string, unknown> | undefined;

  let permission: ChatPermissionMode | undefined;
  if (approval === "never") permission = "execute";
  else if (approval === "on-request") permission = "explore";
  else if (approval) permission = "ask";

  let agentMode: ChatAgentMode | undefined;
  if (collaboration && typeof collaboration === "object") {
    const mode = asString((collaboration as Record<string, unknown>).mode).toLowerCase();
    if (mode === "plan" || mode === "code") agentMode = mode;
    else if (mode) agentMode = "code";
  }

  const settings: Partial<ChatSettings> = {};
  if (model) settings.model = model;
  if (effort) settings.effort = effort as ChatEffort;
  if (permission) settings.permission = permission;
  if (agentMode) settings.agentMode = agentMode;
  if (attachments) {
    const folder =
      asString(attachments.folder) ||
      (Array.isArray(attachments.folders)
        ? asString(attachments.folders[0])
        : "");
    if (folder) settings.attachedFolder = folder;
  }

  return Object.keys(settings).length > 0 ? settings : null;
}
