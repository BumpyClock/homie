import { parseCreatedAt, shortId, truncateText, } from "./chat-types";
function resolveCall(transport) {
    if (typeof transport === "function")
        return transport;
    return transport.call.bind(transport);
}
function asObject(value) {
    if (!value || typeof value !== "object" || Array.isArray(value))
        return null;
    return value;
}
function asString(value) {
    return typeof value === "string" ? value : value ? String(value) : "";
}
function asBoolean(value) {
    if (typeof value === "boolean")
        return value;
    if (typeof value === "number")
        return value !== 0;
    if (typeof value === "string") {
        const normalized = value.trim().toLowerCase();
        if (!normalized)
            return null;
        if (["true", "1", "enabled", "available", "active", "on", "ok"].includes(normalized)) {
            return true;
        }
        if (["false", "0", "disabled", "unavailable", "inactive", "off"].includes(normalized)) {
            return false;
        }
    }
    return null;
}
function parseChatThreadRef(raw) {
    const record = asObject(raw) ?? {};
    return {
        chatId: asString(record.chat_id || record.chatId) || undefined,
        threadId: asString(record.thread_id || record.threadId) || undefined,
        raw,
    };
}
function parseTurnResult(raw) {
    const record = asObject(raw) ?? {};
    return {
        chatId: asString(record.chat_id || record.chatId) || undefined,
        turnId: asString(record.turn_id || record.turnId) || undefined,
        queued: typeof record.queued === "boolean" ? record.queued : undefined,
        raw,
    };
}
export function approvalPolicyForPermission(permission) {
    if (permission === "execute")
        return "never";
    if (permission === "explore")
        return "on-request";
    return "untrusted";
}
export function buildCollaborationPayload({ settings, collaborationModes, defaultModel, }) {
    if (collaborationModes.length === 0)
        return null;
    const selected = collaborationModes.find((mode) => {
        const value = (mode.mode ?? mode.id)?.toLowerCase();
        return value === settings.agentMode;
    });
    const modeValue = (selected?.mode ?? selected?.id ?? settings.agentMode).toLowerCase();
    if (!modeValue)
        return null;
    const payloadSettings = {
        developer_instructions: selected?.developerInstructions ?? null,
    };
    const resolvedModel = settings.model ?? defaultModel;
    if (resolvedModel)
        payloadSettings.model = resolvedModel;
    if (settings.effort !== "auto")
        payloadSettings.reasoning_effort = settings.effort;
    return {
        mode: modeValue,
        settings: payloadSettings,
    };
}
function toSettingsUpdatePayload(settings) {
    if ("approval_policy" in settings || "collaboration_mode" in settings || "attachments" in settings) {
        return settings;
    }
    const patch = settings;
    const payload = {};
    if (patch.model !== undefined)
        payload.model = patch.model;
    if (patch.effort !== undefined)
        payload.effort = patch.effort;
    const approvalPolicy = patch.approvalPolicy ?? (patch.permission ? approvalPolicyForPermission(patch.permission) : undefined);
    if (approvalPolicy !== undefined)
        payload.approval_policy = approvalPolicy;
    if (patch.collaborationMode !== undefined) {
        payload.collaboration_mode = patch.collaborationMode;
    }
    else if (patch.agentMode !== undefined) {
        payload.collaboration_mode = patch.agentMode
            ? { mode: patch.agentMode }
            : null;
    }
    if (patch.attachedFolder !== undefined) {
        payload.attachments = patch.attachedFolder ? { folder: patch.attachedFolder } : null;
    }
    return payload;
}
export function normalizeChatThreadRecords(raw) {
    const record = asObject(raw) ?? {};
    const list = Array.isArray(record.chats) ? record.chats : [];
    return list
        .map((entry) => {
        const item = asObject(entry);
        if (!item)
            return null;
        const chat_id = asString(item.chat_id);
        const thread_id = asString(item.thread_id);
        const created_at = asString(item.created_at);
        const statusRaw = asString(item.status).toLowerCase();
        const status = statusRaw === "inactive" || statusRaw === "exited" ? statusRaw : "active";
        if (!chat_id || !thread_id)
            return null;
        const parsed = {
            chat_id,
            thread_id,
            created_at,
            status,
        };
        if ("settings" in item) {
            parsed.settings = item.settings;
        }
        return parsed;
    })
        .filter((entry) => entry !== null);
}
export function buildChatThreadSummaries(records, options = {}) {
    const overrides = options.overrides ?? {};
    const runningByChatId = options.runningByChatId;
    const now = options.now?.() ?? Date.now();
    return records.map((rec) => {
        const fallbackTitle = `Chat ${shortId(rec.chat_id)}`;
        return {
            chatId: rec.chat_id,
            threadId: rec.thread_id,
            title: overrides[rec.chat_id] ?? fallbackTitle,
            preview: "",
            status: rec.status,
            lastActivityAt: parseCreatedAt(rec.created_at) ?? now,
            running: runningByChatId ? runningByChatId.has(rec.chat_id) : false,
        };
    });
}
function normalizeWebToolName(value) {
    const normalized = asString(value).trim().toLowerCase();
    if (normalized === "web_fetch" || normalized === "web-fetch" || normalized === "webfetch") {
        return "web_fetch";
    }
    if (normalized === "web_search" || normalized === "web-search" || normalized === "websearch") {
        return "web_search";
    }
    return null;
}
export function normalizeEnabledWebTools(raw) {
    const root = asObject(raw);
    if (!root)
        return [];
    const enabled = new Set();
    const apply = (name, state) => {
        if (state === false) {
            enabled.delete(name);
            return;
        }
        enabled.add(name);
    };
    const ingestEntry = (entry) => {
        if (typeof entry === "string") {
            const name = normalizeWebToolName(entry);
            if (name)
                enabled.add(name);
            return;
        }
        const record = asObject(entry);
        if (!record)
            return;
        const directFetch = asBoolean(record.web_fetch ?? record.webFetch);
        if (directFetch !== null)
            apply("web_fetch", directFetch);
        const directSearch = asBoolean(record.web_search ?? record.webSearch);
        if (directSearch !== null)
            apply("web_search", directSearch);
        const name = normalizeWebToolName(record.name ?? record.tool ?? record.id ?? record.slug);
        if (!name)
            return;
        const state = asBoolean(record.enabled ??
            record.is_enabled ??
            record.isEnabled ??
            record.available ??
            record.active ??
            record.status ??
            record.state) ?? true;
        apply(name, state);
    };
    const result = asObject(root.result) ?? {};
    const data = root.data;
    const dataRecord = asObject(data) ?? {};
    const candidateBuckets = [
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
        }
        else {
            ingestEntry(bucket);
        }
    }
    return ["web_fetch", "web_search"].filter((name) => enabled.has(name));
}
export function normalizeModelOptions(raw) {
    const record = asObject(raw);
    if (!record)
        return [];
    let data = record.data ?? asObject(record.result)?.data ?? [];
    if (data && !Array.isArray(data)) {
        data = asObject(data)?.data ?? data;
    }
    if (!Array.isArray(data))
        return [];
    return data.map((entry) => {
        const item = asObject(entry) ?? {};
        const model = asString(item.model || item.id);
        const id = asString(item.id || model);
        const displayName = asString(item.displayName || item.display_name || model);
        const supported = Array.isArray(item.supportedReasoningEfforts)
            ? item.supportedReasoningEfforts
            : Array.isArray(item.supported_reasoning_efforts)
                ? item.supported_reasoning_efforts
                : [];
        return {
            id,
            model,
            displayName,
            description: asString(item.description),
            supportedReasoningEfforts: supported
                .map((supportedEntry) => {
                const supportedItem = asObject(supportedEntry) ?? {};
                return {
                    reasoningEffort: asString(supportedItem.reasoningEffort ?? supportedItem.reasoning_effort),
                    description: asString(supportedItem.description),
                };
            })
                .filter((supportedItem) => supportedItem.reasoningEffort),
            defaultReasoningEffort: asString(item.defaultReasoningEffort ?? item.default_reasoning_effort) || null,
            isDefault: Boolean(item.isDefault ?? item.is_default),
        };
    });
}
export function normalizeCollaborationModes(raw) {
    const record = asObject(raw);
    if (!record)
        return [];
    let data = record.data ?? asObject(record.result)?.data ?? [];
    if (data && !Array.isArray(data)) {
        data = asObject(data)?.data ?? data;
    }
    if (!Array.isArray(data))
        return [];
    return data.map((entry) => {
        const item = asObject(entry) ?? {};
        const name = asString(item.name);
        const mode = asString(item.mode);
        const id = name || mode;
        return {
            id: id || "mode",
            label: name || mode || "Mode",
            mode: mode || null,
            model: asString(item.model) || null,
            reasoningEffort: asString(item.reasoning_effort ?? item.reasoningEffort) || null,
            developerInstructions: asString(item.developer_instructions ?? item.developerInstructions) || null,
        };
    });
}
export function normalizeSkillOptions(raw) {
    const record = asObject(raw);
    if (!record)
        return [];
    const result = asObject(record.result) ?? {};
    const dataBuckets = result.data ?? record.data ?? [];
    const rawSkills = result.skills ??
        record.skills ??
        (Array.isArray(dataBuckets)
            ? dataBuckets.flatMap((bucket) => asObject(bucket)?.skills ?? [])
            : []);
    if (!Array.isArray(rawSkills))
        return [];
    return rawSkills
        .map((entry) => {
        const item = asObject(entry) ?? {};
        return {
            name: asString(item.name),
            path: asString(item.path),
            description: asString(item.description) || undefined,
        };
    })
        .filter((skill) => skill.name);
}
export function normalizeFileOptions(raw) {
    const record = asObject(raw);
    if (!record)
        return [];
    const files = record.files ?? asObject(record.result)?.files ?? [];
    if (!Array.isArray(files))
        return [];
    return files
        .map((entry) => {
        const item = asObject(entry) ?? {};
        const type = asString(item.type);
        return {
            name: asString(item.name),
            path: asString(item.path),
            relativePath: asString(item.relative_path ?? item.relativePath),
            type: type === "directory" ? "directory" : "file",
        };
    })
        .filter((file) => file.name && file.path);
}
export function normalizeChatSettings(raw) {
    const record = asObject(raw);
    if (!record)
        return null;
    const model = asString(record.model);
    const effort = asString(record.effort);
    const approval = asString(record.approval_policy ?? record.approvalPolicy);
    const collaboration = record.collaboration_mode ?? record.collaborationMode;
    const attachments = asObject(record.attachments);
    let permission;
    if (approval === "never")
        permission = "execute";
    else if (approval === "on-request")
        permission = "explore";
    else if (approval)
        permission = "ask";
    let agentMode;
    const collaborationRecord = asObject(collaboration);
    if (collaborationRecord) {
        const mode = asString(collaborationRecord.mode).toLowerCase();
        if (mode === "plan" || mode === "code")
            agentMode = mode;
        else if (mode)
            agentMode = "code";
    }
    const settings = {};
    if (model)
        settings.model = model;
    if (effort)
        settings.effort = effort;
    if (permission)
        settings.permission = permission;
    if (agentMode)
        settings.agentMode = agentMode;
    if (attachments) {
        const folder = asString(attachments.folder) ||
            (Array.isArray(attachments.folders) ? asString(attachments.folders[0]) : "");
        if (folder)
            settings.attachedFolder = folder;
    }
    return Object.keys(settings).length > 0 ? settings : null;
}
export function parseThreadReadResult(raw) {
    const root = asObject(raw) ?? {};
    const threadCandidate = root.thread ?? raw;
    const thread = asObject(threadCandidate);
    const settings = normalizeChatSettings(root.settings);
    return {
        thread: thread ?? null,
        settings,
        raw,
    };
}
export function createChatClient(transport) {
    const call = resolveCall(transport);
    return {
        async create() {
            const raw = await call("chat.create");
            return parseChatThreadRef(raw);
        },
        async list() {
            const raw = await call("chat.list");
            return normalizeChatThreadRecords(raw);
        },
        async resume(chatId, threadId) {
            const raw = await call("chat.resume", {
                chat_id: chatId,
                thread_id: threadId,
            });
            return parseChatThreadRef(raw);
        },
        async readThread(chatId, threadId, includeTurns = true) {
            const raw = await call("chat.thread.read", {
                chat_id: chatId,
                thread_id: threadId,
                include_turns: includeTurns,
            });
            return parseThreadReadResult(raw);
        },
        async sendMessage(input) {
            const approvalPolicy = input.approvalPolicy ?? (input.permission ? approvalPolicyForPermission(input.permission) : undefined);
            const effort = input.effort === "auto" ? undefined : input.effort;
            const raw = await call("chat.message.send", {
                chat_id: input.chatId,
                message: input.message,
                model: input.model,
                effort,
                approval_policy: approvalPolicy,
                collaboration_mode: input.collaborationMode ?? undefined,
                inject: input.inject,
            });
            return parseTurnResult(raw);
        },
        cancel(input) {
            return call("chat.cancel", {
                chat_id: input.chatId,
                turn_id: input.turnId,
            });
        },
        archiveThread(input) {
            return call("chat.thread.archive", {
                chat_id: input.chatId,
                thread_id: input.threadId,
            });
        },
        renameThread(input) {
            return call("chat.thread.rename", {
                chat_id: input.chatId,
                thread_id: input.threadId,
                title: input.title,
            });
        },
        respondApproval(input) {
            return call("chat.approval.respond", {
                codex_request_id: input.requestId,
                decision: input.decision,
            });
        },
        async readAccount() {
            const raw = await call("chat.account.read");
            return asObject(raw) ?? {};
        },
        async listModels() {
            const raw = await call("chat.model.list");
            return normalizeModelOptions(raw);
        },
        async listCollaborationModes() {
            const raw = await call("chat.collaboration.mode.list");
            return normalizeCollaborationModes(raw);
        },
        async listSkills() {
            const raw = await call("chat.skills.list");
            return normalizeSkillOptions(raw);
        },
        async listTools(channel = "web") {
            const raw = await call("chat.tools.list", { channel });
            return normalizeEnabledWebTools(raw);
        },
        updateSettings(input) {
            return call("chat.settings.update", {
                chat_id: input.chatId,
                settings: toSettingsUpdatePayload(input.settings),
            });
        },
        async searchFiles(input) {
            const raw = await call("chat.files.search", {
                chat_id: input.chatId,
                query: input.query,
                limit: input.limit,
                base_path: input.basePath ?? undefined,
            });
            return normalizeFileOptions(raw);
        },
    };
}
export function deriveChatPreview(message, limit = 96) {
    return truncateText(message, limit);
}
//# sourceMappingURL=chat-client.js.map