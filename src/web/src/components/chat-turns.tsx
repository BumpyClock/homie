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
}

interface TurnGroup {
  id: string;
  turnId?: string;
  items: ChatItem[];
}

function groupTurns(items: ChatItem[]): TurnGroup[] {
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
    <div className="flex justify-end">
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
    return (
      <div className="rounded-md border border-border bg-card/40 p-3 text-sm">
        <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground mb-2">
          <MessageCircleDashed className="h-4 w-4" />
          Reasoning
        </div>
        {summary.length > 0 && (
          <ul className="mb-2 list-disc pl-5 text-sm text-foreground">
            {summary.map((line, idx) => (
              <li key={idx}>{line}</li>
            ))}
          </ul>
        )}
        {content.length > 0 && (
          <pre className="text-xs font-mono whitespace-pre-wrap text-muted-foreground">
            {content.join("\n")}
          </pre>
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
    return (
      <div className="rounded-md border border-border bg-card/30 p-3 text-sm">
        <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground mb-1">
          <Wrench className="h-4 w-4" />
          Tool
        </div>
        <div className="text-sm">{item.text || "Tool call"}</div>
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
  const hasActivities = activities.length > 0;
  const preview = previewFromTurn(turn, isStreaming);
  const [expanded, setExpanded] = useState(false);

  return (
    <div className="flex justify-start">
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

        {hasActivities && (
          <div
            className={`overflow-hidden transition-[max-height,opacity] duration-200 ease-out motion-reduce:transition-none ${
              expanded ? "max-h-[1200px] opacity-100" : "max-h-0 opacity-0"
            }`}
          >
            <div className="space-y-3 pb-1">
              {activities.map((item) => (
                <ActivityRow key={item.id} item={item} onApprove={onApprove} />
              ))}
            </div>
          </div>
        )}

        <div className="rounded-[14px] border border-border bg-muted/30 px-4 py-3 text-sm text-foreground">
          {response ? (
            <ChatMarkdown content={response} />
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

export function ChatTurns({ items, activeTurnId, running, onApprove }: ChatTurnsProps) {
  const turns = useMemo(() => groupTurns(items), [items]);
  return (
    <div className="space-y-4">
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
