import type { Dispatch, MutableRefObject, SetStateAction } from "react";
import {
  approvalPolicyForPermission,
  normalizeFileOptions,
  truncateText,
  type ActiveChatThread,
  type ChatCollaborationPayload,
  type ChatSettings,
  type ChatThreadSummary,
  type FileOption,
} from "@homie/shared";
import { uuid } from "@/lib/uuid";

type CallFn = (method: string, params?: unknown) => Promise<unknown>;

interface SendChatMessageArgs {
  activeThread: ActiveChatThread | null;
  message: string;
  call: CallFn;
  resolveSettings: (chatId: string | null) => ChatSettings;
  buildCollaborationPayload: (settings: ChatSettings) => ChatCollaborationPayload | null;
  overrides: Record<string, string>;
  updateOverrides: (chatId: string, title: string | null) => void;
  runningTurnsRef: MutableRefObject<Map<string, string>>;
  queuedTimersRef: MutableRefObject<Record<string, ReturnType<typeof setTimeout>>>;
  clearQueuedNotice: (chatId: string) => void;
  setQueuedNoticeByChatId: Dispatch<SetStateAction<Record<string, boolean>>>;
  setActiveThread: Dispatch<SetStateAction<ActiveChatThread | null>>;
  setThreads: Dispatch<SetStateAction<ChatThreadSummary[]>>;
  setError: Dispatch<SetStateAction<string | null>>;
}

export async function sendChatMessage({
  activeThread,
  message,
  call,
  resolveSettings,
  buildCollaborationPayload,
  overrides,
  updateOverrides,
  runningTurnsRef,
  queuedTimersRef,
  clearQueuedNotice,
  setQueuedNoticeByChatId,
  setActiveThread,
  setThreads,
  setError,
}: SendChatMessageArgs) {
  if (!activeThread) return;
  const trimmed = message.trim();
  if (!trimmed) return;

  const settings = resolveSettings(activeThread.chatId);
  const approvalPolicy = approvalPolicyForPermission(settings.permission);
  const effort = settings.effort !== "auto" ? settings.effort : undefined;
  const collaborationMode = buildCollaborationPayload(settings);
  const inject = activeThread.running;

  if (inject) {
    const chatId = activeThread.chatId;
    setQueuedNoticeByChatId((prev) => ({ ...prev, [chatId]: true }));
    const existingTimer = queuedTimersRef.current[chatId];
    if (existingTimer) clearTimeout(existingTimer);
    queuedTimersRef.current[chatId] = setTimeout(() => {
      clearQueuedNotice(chatId);
    }, 4000);
  }

  const localId = uuid();
  setActiveThread((prev) =>
    prev
      ? {
          ...prev,
          items: [
            ...prev.items,
            { id: localId, kind: "user", role: "user", text: trimmed, optimistic: true },
          ],
        }
      : prev,
  );

  if (!overrides[activeThread.chatId]) {
    const autoTitle = truncateText(trimmed, 42);
    updateOverrides(activeThread.chatId, autoTitle);
  }

  try {
    const res = (await call("chat.message.send", {
      chat_id: activeThread.chatId,
      message: trimmed,
      model: settings.model,
      effort,
      approval_policy: approvalPolicy,
      collaboration_mode: collaborationMode ?? undefined,
      inject,
    })) as { turn_id?: string };

    if (res?.turn_id) {
      runningTurnsRef.current.set(activeThread.chatId, res.turn_id);
      setActiveThread((prev) =>
        prev ? { ...prev, running: true, activeTurnId: res.turn_id } : prev,
      );
      setThreads((prev) =>
        prev.map((thread) =>
          thread.chatId === activeThread.chatId
            ? {
                ...thread,
                running: true,
                lastActivityAt: Date.now(),
                preview: truncateText(trimmed, 96),
              }
            : thread,
        ),
      );
    }
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    setError(msg || "Failed to send message");
  }
}

interface CancelActiveTurnArgs {
  activeThread: ActiveChatThread | null;
  call: CallFn;
}

export async function cancelActiveTurn({ activeThread, call }: CancelActiveTurnArgs) {
  if (!activeThread?.activeTurnId) return;
  try {
    await call("chat.cancel", {
      chat_id: activeThread.chatId,
      turn_id: activeThread.activeTurnId,
    });
  } catch {
    return;
  }
}

interface RespondToApprovalArgs {
  requestId: number | string;
  decision: "accept" | "decline";
  call: CallFn;
  setError: Dispatch<SetStateAction<string | null>>;
}

export async function respondToApproval({
  requestId,
  decision,
  call,
  setError,
}: RespondToApprovalArgs) {
  try {
    console.debug("[chat] approval respond", { requestId, decision });
    await call("chat.approval.respond", { codex_request_id: requestId, decision });
  } catch (err: unknown) {
    console.error("[chat] approval respond failed", err);
    const msg = err instanceof Error ? err.message : String(err);
    setError(msg || "Approval failed");
  }
}

interface UpdateChatAttachmentsArgs {
  chatId: string;
  folder: string | null;
  call: CallFn;
  updateSettings: (chatId: string, updates: Partial<ChatSettings>) => void;
  setError: Dispatch<SetStateAction<string | null>>;
}

export async function updateChatAttachments({
  chatId,
  folder,
  call,
  updateSettings,
  setError,
}: UpdateChatAttachmentsArgs) {
  updateSettings(chatId, { attachedFolder: folder ?? undefined });
  try {
    await call("chat.settings.update", {
      chat_id: chatId,
      settings: { attachments: folder ? { folder } : null },
    });
  } catch (err: unknown) {
    const msg = err instanceof Error ? err.message : String(err);
    setError(msg || "Failed to update attachments");
  }
}

interface SearchChatFilesArgs {
  chatId: string;
  query: string;
  basePath?: string | null;
  limit?: number;
  enabled: boolean;
  status: string;
  call: CallFn;
}

export async function searchChatFiles({
  chatId,
  query,
  basePath,
  limit = 40,
  enabled,
  status,
  call,
}: SearchChatFilesArgs): Promise<FileOption[]> {
  if (!enabled || status !== "connected") return [];
  if (!query.trim()) return [];
  try {
    console.debug("[chat] files search", { chatId, query, limit, basePath });
    const res = await call("chat.files.search", {
      chat_id: chatId,
      query,
      limit,
      base_path: basePath ?? undefined,
    });
    const normalized = normalizeFileOptions(res);
    console.debug("[chat] files search result", { count: normalized.length });
    return normalized;
  } catch {
    console.debug("[chat] files search failed");
    return [];
  }
}
