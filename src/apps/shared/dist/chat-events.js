import { createLocalId, getItemId, getThreadId, getTurnId, itemsFromThread, normalizeTokenUsage, resolveApprovalRequestId, } from "./chat-types";
export function buildPlanUpdateText(explanation, plan) {
    const lines = [
        explanation ? `Note: ${explanation}` : "",
        ...plan.map((entry) => `- [${entry.status}] ${entry.step}`),
    ].filter(Boolean);
    return lines.join("\n");
}
function asRecord(value) {
    if (!value || typeof value !== "object" || Array.isArray(value))
        return null;
    return value;
}
function resolveChatId(threadId, options) {
    if (options.resolveChatId) {
        const chatId = options.resolveChatId(threadId);
        if (chatId)
            return chatId;
    }
    const mapped = options.threadIdLookup?.get(threadId);
    return mapped ?? threadId;
}
function resolveIdFactory(options) {
    return options.idFactory ?? (() => createLocalId("chat-event"));
}
function resolveActivityTime(options) {
    return options.now ? options.now() : Date.now();
}
function baseEvent(topic, chatId, threadId, params, options) {
    return {
        topic,
        chatId,
        threadId,
        turnId: getTurnId(params),
        activityAt: resolveActivityTime(options),
        rawParams: params,
    };
}
function mapItemEvent(topic, base, params, options) {
    const itemRecord = asRecord(params.item);
    if (!itemRecord)
        return null;
    const idFactory = resolveIdFactory(options);
    const itemId = typeof itemRecord.id === "string" ? itemRecord.id : idFactory();
    const mapped = itemsFromThread({
        turns: [
            {
                id: base.turnId,
                items: [{ ...itemRecord, id: itemId }],
            },
        ],
    }, { idFactory })[0];
    if (!mapped)
        return null;
    if (mapped.kind === "assistant" && mapped.text && options.messageBuffer) {
        options.messageBuffer.set(mapped.id, mapped.text);
    }
    return {
        ...base,
        type: topic === "chat.item.started" ? "item.started" : "item.completed",
        item: mapped,
    };
}
function mapPlanSteps(plan) {
    if (!Array.isArray(plan))
        return [];
    return plan
        .map((entry) => {
        const step = asRecord(entry);
        if (!step)
            return null;
        return {
            step: typeof step.step === "string" ? step.step : "step",
            status: typeof step.status === "string" ? step.status : "pending",
        };
    })
        .filter((entry) => entry !== null);
}
export function mapChatEvent(event, options = {}) {
    if (!event.topic.startsWith("chat."))
        return null;
    const params = asRecord(event.params) ?? {};
    const threadId = getThreadId(params);
    if (!threadId)
        return null;
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
        if (!tokenUsage)
            return null;
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
export async function subscribeToChatEvents(call, topic = "chat.*") {
    await call("events.subscribe", { topic });
}
export function bindMappedChatEvents(source, handler, options = {}) {
    return source.onEvent((event) => {
        const mapped = mapChatEvent(event, options);
        if (mapped)
            handler(mapped);
    });
}
//# sourceMappingURL=chat-events.js.map