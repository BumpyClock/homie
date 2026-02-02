import { useMemo, useState } from "react";
import {
  ChevronDown,
  MessageCircleDashed,
  Terminal,
  FileDiff,
  ClipboardList,
  Wrench,
  Globe,
  CheckCircle2,
  XCircle,
  Loader2,
} from "lucide-react";
import type { ChatItem } from "@/lib/chat-utils";
import { ChatMarkdown } from "@/components/chat-markdown";

interface ChatTurnsProps {
  items: ChatItem[];
  activeTurnId?: string;
  running: boolean;
  onApprove?: (requestId: number | string, decision: "accept" | "decline") => void;
  visibleTurnCount?: number;
}

interface TurnGroup {
  id: string;
  turnId?: string;
  items: ChatItem[];
}

export function groupTurns(items: ChatItem[]): TurnGroup[] {
  const order: string[] = [];
  const map = new Map<string, TurnGroup>();
  items.forEach((item) => {
    const key = item.turnId ?? item.id;
    if (!map.has(key)) {
      map.set(key, { id: key, turnId: item.turnId, items: [] });
      order.push(key);
    }
    map.get(key)?.items.push(item);
  });
  return order.map((id) => map.get(id)!).filter(Boolean);
}

function stripMarkdown(text: string) {
  return text.replace(/[#_*`>~-]/g, "").replace(/\s+/g, " ").trim();
}

function previewFromTurn(turn: TurnGroup, isStreaming: boolean) {
  const assistant = turn.items.find((item) => item.kind === "assistant");
  if (assistant?.text) return stripMarkdown(assistant.text).slice(0, 140);
  const reasoning = turn.items.find((item) => item.kind === "reasoning");
  if (reasoning?.summary?.length) return stripMarkdown(reasoning.summary[0]).slice(0, 140);
  const command = turn.items.find((item) => item.kind === "command");
  if (command?.command) return `Command: ${stripMarkdown(command.command).slice(0, 100)}`;
  if (isStreaming) return "Thinking…";
  return "Steps completed";
}

function getReasoningPreview(item?: ChatItem) {
  if (!item) return "";
  const summary = item.summary?.filter(Boolean) ?? [];
  if (summary.length > 0) return summary[0];
  const content = item.content?.filter(Boolean) ?? [];
  if (content.length > 0) return content[0];
  return "";
}

function getActivityPreview(item: ChatItem) {
  switch (item.kind) {
    case "approval":
      return item.reason || item.command || "Approval required";
    case "reasoning":
      return getReasoningPreview(item) || "Reasoning update";
    case "command":
      return item.command ? `Command: ${item.command}` : "Command execution";
    case "file":
      return item.changes?.[0]?.path ? `File: ${item.changes[0].path}` : "File changes";
    case "plan":
      return item.text ? stripMarkdown(item.text).slice(0, 120) : "Plan update";
    case "diff":
      return item.text ? stripMarkdown(item.text).slice(0, 120) : "Diff update";
    case "tool":
      return item.text || "Tool call";
    case "system":
      return item.text || "System update";
    default:
      return item.text || "Update";
  }
}

function statusBadge(status?: string) {
  if (!status) return null;
  const normalized = status.toLowerCase();
  const base = "inline-flex items-center gap-1 rounded-full px-2 py-0.5 text-[11px]";
  if (normalized === "running") {
    return (
      <span className={`${base} bg-amber-500/10 text-amber-700 dark:text-amber-300`}>
        <Loader2 className="h-3 w-3 animate-spin" />
        Running
      </span>
    );
  }
  if (normalized === "error" || normalized === "failed") {
    return (
      <span className={`${base} bg-destructive/10 text-destructive`}>
        <XCircle className="h-3 w-3" />
        Error
      </span>
    );
  }
  return (
    <span className={`${base} bg-emerald-500/10 text-emerald-700 dark:text-emerald-300`}>
      <CheckCircle2 className="h-3 w-3" />
      {status}
    </span>
  );
}

function UserBubble({ text }: { text: string }) {
  return (
    <div className="flex justify-end homie-fade-in">
      <div className="max-w-[720px] rounded-[14px] bg-foreground/5 px-4 py-3 text-sm text-foreground">
        <ChatMarkdown content={text} compact />
      </div>
    </div>
  );
}

function ActivityRow({
  item,
  onApprove,
}: {
  item: ChatItem;
  onApprove?: (requestId: number | string, decision: "accept" | "decline") => void;
}) {
  const intent = extractToolIntent(item);
  if (item.kind === "approval") {
    const canRespond = item.requestId !== undefined;
    return (
      <div className="rounded-md border border-amber-300/60 bg-amber-50/60 dark:bg-amber-500/10 p-3 text-sm">
        <div className="text-xs uppercase tracking-wide text-amber-700 dark:text-amber-300 mb-2">
          Approval required
        </div>
        {item.reason && <div className="text-amber-800 dark:text-amber-200 mb-2">{item.reason}</div>}
        {item.command && (
          <pre className="text-xs font-mono bg-black/5 dark:bg-white/5 p-2 rounded border border-amber-200/60 mb-3">
            {item.command}
          </pre>
        )}
        <div className="flex flex-wrap gap-2">
          <button
            type="button"
            disabled={!canRespond}
            onClick={() => {
              if (item.requestId === undefined) return;
              onApprove?.(item.requestId, "accept");
            }}
            className="px-3 py-2 min-h-[44px] rounded-md bg-amber-500 text-white text-sm font-medium hover:bg-amber-600 transition-colors disabled:opacity-60"
          >
            Approve
          </button>
          <button
            type="button"
            disabled={!canRespond}
            onClick={() => {
              if (item.requestId === undefined) return;
              onApprove?.(item.requestId, "decline");
            }}
            className="px-3 py-2 min-h-[44px] rounded-md border border-amber-300 text-amber-700 dark:text-amber-200 hover:bg-amber-100/70 dark:hover:bg-amber-500/20 transition-colors disabled:opacity-60"
          >
            Decline
          </button>
        </div>
      </div>
    );
  }

  if (item.kind === "reasoning") {
    const summary = item.summary?.filter(Boolean) ?? [];
    const content = item.content?.filter(Boolean) ?? [];
    const summaryText = summary.join("\n");
    const contentText = content.join("\n");
    return (
      <div className="rounded-md border border-border bg-card/40 p-3 text-sm">
        <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground mb-2">
          <MessageCircleDashed className="h-4 w-4" />
          Reasoning
        </div>
        {summaryText && (
          <div className="mb-2 text-sm text-foreground">
            <ChatMarkdown content={summaryText} compact />
          </div>
        )}
        {contentText && (
          <div className="text-xs text-muted-foreground">
            <ChatMarkdown content={contentText} compact />
          </div>
        )}
      </div>
    );
  }

  if (item.kind === "command") {
    return (
      <div className="rounded-md border border-border bg-card/40 p-3 text-sm">
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground">
            <Terminal className="h-4 w-4" />
            Command
          </div>
          {statusBadge(item.status)}
        </div>
        <pre className="text-xs font-mono bg-black/5 dark:bg-white/5 p-2 rounded border border-border">
          {item.command || ""}
        </pre>
        {item.output && (
          <pre className="mt-3 text-xs font-mono bg-black/5 dark:bg-white/5 p-2 rounded border border-border whitespace-pre-wrap">
            {item.output}
          </pre>
        )}
      </div>
    );
  }

  if (item.kind === "file") {
    return (
      <div className="rounded-md border border-border bg-card/40 p-3 text-sm">
        <div className="flex items-center justify-between mb-2">
          <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground">
            <FileDiff className="h-4 w-4" />
            File changes
          </div>
          {statusBadge(item.status)}
        </div>
        {item.changes && item.changes.length > 0 ? (
          <div className="space-y-3">
            {item.changes.map((change) => (
              <div key={change.path} className="space-y-2">
                <div className="text-xs font-mono text-muted-foreground">{change.path}</div>
                <pre className="text-xs font-mono bg-black/5 dark:bg-white/5 p-2 rounded border border-border whitespace-pre-wrap">
                  {change.diff}
                </pre>
              </div>
            ))}
          </div>
        ) : (
          <div className="text-xs text-muted-foreground">No diff available.</div>
        )}
      </div>
    );
  }

  if (item.kind === "plan") {
    return (
      <div className="rounded-md border border-border bg-card/40 p-3 text-sm">
        <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground mb-2">
          <ClipboardList className="h-4 w-4" />
          Plan
        </div>
        <pre className="text-xs font-mono whitespace-pre-wrap">{item.text || ""}</pre>
      </div>
    );
  }

  if (item.kind === "diff") {
    return (
      <div className="rounded-md border border-border bg-card/40 p-3 text-sm">
        <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground mb-2">
          <FileDiff className="h-4 w-4" />
          Diff
        </div>
        <pre className="text-xs font-mono whitespace-pre-wrap">{item.text || ""}</pre>
      </div>
    );
  }

  if (item.kind === "tool") {
    const status = item.status?.toLowerCase();
    const statusIcon =
      status === "running" ? (
        <Loader2 className="h-3.5 w-3.5 animate-spin text-muted-foreground" />
      ) : status === "error" || status === "failed" ? (
        <XCircle className="h-3.5 w-3.5 text-destructive" />
      ) : status ? (
        <CheckCircle2 className="h-3.5 w-3.5 text-emerald-600 dark:text-emerald-400" />
      ) : (
        <Wrench className="h-3.5 w-3.5 text-muted-foreground" />
      );
    return (
      <div className="rounded-md border border-border bg-card/30 p-3 text-sm">
        <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground mb-1">
          {statusIcon}
          Tool
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="text-sm">{item.text || "Tool call"}</div>
          {intent && (
            <span className="rounded-full border border-border bg-muted/40 px-2 py-0.5 text-[11px] text-muted-foreground">
              {intent}
            </span>
          )}
        </div>
      </div>
    );
  }

  if (item.kind === "system") {
    return (
      <div className="rounded-md border border-border bg-card/30 p-3 text-sm text-muted-foreground">
        <div className="flex items-center gap-2 text-xs uppercase tracking-wide mb-1">
          <Globe className="h-4 w-4" />
          System
        </div>
        {item.text || ""}
      </div>
    );
  }

  return (
    <div className="rounded-md border border-border bg-card/30 p-3 text-sm text-muted-foreground">
      {item.text || "Unsupported item"}
    </div>
  );
}

function extractToolIntent(item: ChatItem) {
  if (item.kind !== "tool" && item.kind !== "command") return null;
  const raw = item.raw as Record<string, unknown> | undefined;
  if (!raw) return null;
  const input = raw.input as Record<string, unknown> | undefined;
  const candidates = [
    raw.intent,
    raw.description,
    input?.description,
    input?.intent,
    input?.query,
    input?.path,
  ];
  for (const candidate of candidates) {
    if (typeof candidate === "string" && candidate.trim()) {
      return candidate.trim();
    }
  }
  return null;
}

function AssistantTurn({
  turn,
  isStreaming,
  onApprove,
}: {
  turn: TurnGroup;
  isStreaming: boolean;
  onApprove?: (requestId: number | string, decision: "accept" | "decline") => void;
}) {
  const assistant = turn.items.filter((item) => item.kind === "assistant");
  const response = assistant[assistant.length - 1]?.text ?? "";
  const activities = turn.items.filter((item) => item.kind !== "user" && item.kind !== "assistant");
  const approvalItems = activities.filter((item) => item.kind === "approval");
  const nonApprovalActivities = activities.filter((item) => item.kind !== "approval");
  const hasActivities = nonApprovalActivities.length > 0 || approvalItems.length > 0;
  const preview = previewFromTurn(turn, isStreaming);
  const [expanded, setExpanded] = useState(false);
  const lastActivity = nonApprovalActivities[nonApprovalActivities.length - 1] ?? approvalItems[approvalItems.length - 1];
  const reasoningItem = [...activities].reverse().find((item) => item.kind === "reasoning");
  const showReasoningPreview = !expanded && reasoningItem && reasoningItem !== lastActivity;
  const showStreamingDots = isStreaming && !expanded;

  return (
    <div className="flex justify-start homie-fade-in">
      <div className="w-full max-w-[720px] space-y-3">
        {hasActivities && (
          <button
            type="button"
            onClick={() => setExpanded((prev) => !prev)}
            className="w-full flex items-center justify-between gap-3 rounded-md border border-border bg-card/40 px-3 py-2 text-left text-xs text-muted-foreground hover:bg-muted/40 transition-colors"
          >
            <div className="flex items-center gap-2 min-w-0">
              <ChevronDown className={`h-4 w-4 transition-transform ${expanded ? "rotate-180" : ""}`} />
              <span className="truncate">{preview}</span>
            </div>
            <span className="text-[11px] uppercase tracking-wide">{activities.length} steps</span>
          </button>
        )}

        {!expanded && lastActivity && (
          <div className="rounded-md border border-border bg-card/30 px-3 py-2 text-xs text-muted-foreground flex items-center gap-2 homie-fade-in">
            <span className="text-[11px] uppercase tracking-wide">Last step</span>
            <span className="text-foreground/80 truncate flex-1">
              {getActivityPreview(lastActivity)}
              {showStreamingDots && (
                <span className="homie-dots ml-1 inline-flex items-center gap-1 align-middle" aria-hidden="true">
                  <span className="h-2 w-2 rounded-full bg-muted-foreground/70" />
                  <span className="h-2 w-2 rounded-full bg-muted-foreground/70" />
                  <span className="h-2 w-2 rounded-full bg-muted-foreground/70" />
                </span>
              )}
            </span>
          </div>
        )}

        {!expanded && showReasoningPreview && reasoningItem && (
          <div className="rounded-md border border-border bg-card/20 px-3 py-2 text-xs text-muted-foreground flex items-center gap-2 homie-fade-in">
            <span className="text-[11px] uppercase tracking-wide">Reasoning</span>
            <span className="text-foreground/80 truncate flex-1">
              {getReasoningPreview(reasoningItem)}
              {showStreamingDots && (
                <span className="homie-dots ml-1 inline-flex items-center gap-1 align-middle" aria-hidden="true">
                  <span className="h-2 w-2 rounded-full bg-muted-foreground/70" />
                  <span className="h-2 w-2 rounded-full bg-muted-foreground/70" />
                  <span className="h-2 w-2 rounded-full bg-muted-foreground/70" />
                </span>
              )}
            </span>
          </div>
        )}

        {approvalItems.length > 0 && !expanded && (
          <div className="space-y-2">
            {approvalItems.map((item) => (
              <ActivityRow key={item.id} item={item} onApprove={onApprove} />
            ))}
          </div>
        )}

        {hasActivities && (
          <div
            className={`overflow-hidden transition-[max-height,opacity] duration-200 ease-out motion-reduce:transition-none ${
              expanded ? "max-h-[1200px] opacity-100" : "max-h-0 opacity-0"
            }`}
          >
            <div className="space-y-3 pb-1">
              {nonApprovalActivities.map((item) => (
                <ActivityRow key={item.id} item={item} onApprove={onApprove} />
              ))}
            </div>
          </div>
        )}

        <div className="rounded-[14px] border border-border bg-muted/30 px-4 py-3 text-sm text-foreground">
          {response ? (
            <div className="homie-fade-in">
              <ChatMarkdown content={response} />
            </div>
          ) : (
            <div className="text-sm text-muted-foreground">
              {isStreaming ? "Thinking…" : "Awaiting response"}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

export function ChatTurns({
  items,
  activeTurnId,
  running,
  onApprove,
  visibleTurnCount,
}: ChatTurnsProps) {
  const allTurns = useMemo(() => groupTurns(items), [items]);
  const startIndex = useMemo(() => {
    if (!visibleTurnCount) return 0;
    return Math.max(0, allTurns.length - visibleTurnCount);
  }, [allTurns.length, visibleTurnCount]);
  const turns = useMemo(() => allTurns.slice(startIndex), [allTurns, startIndex]);
  const hasMoreAbove = visibleTurnCount !== undefined && allTurns.length > turns.length;
  return (
    <div className="space-y-4">
      {hasMoreAbove && (
        <div className="text-center text-xs text-muted-foreground/70">
          ↑ Scroll up for earlier messages ({startIndex} more)
        </div>
      )}
      {turns.map((turn) => {
        const turnId = turn.turnId ?? turn.id;
        const userItems = turn.items.filter((item) => item.kind === "user");
        const hasAssistant = turn.items.some((item) => item.kind !== "user");
        const isStreaming = running && activeTurnId === turnId;
        return (
          <div key={turn.id} className="space-y-3">
            {userItems.map((item) => (
              <UserBubble key={item.id} text={item.text || ""} />
            ))}
            {hasAssistant && (
              <AssistantTurn turn={turn} isStreaming={isStreaming} onApprove={onApprove} />
            )}
          </div>
        );
      })}
    </div>
  );
}
