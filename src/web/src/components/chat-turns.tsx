import { useMemo, useState } from "react";
import {
  ChevronDown,
  MessageCircleDashed,
  Terminal,
  FileDiff,
  ClipboardList,
  Wrench,
  Globe,
  ExternalLink,
  CheckCircle2,
  XCircle,
  Loader2,
} from "lucide-react";
import type { ChatItem } from "@/lib/chat-utils";
import { ChatMarkdown } from "@/components/chat-markdown";
import {
  groupTurns,
  previewFromTurn,
  getReasoningPreview,
  getActivityPreview,
} from "@/lib/chat-turns-utils";
import {
  friendlyToolLabelFromItem,
  normalizeChatToolName,
  rawToolNameFromItem,
} from "@/lib/chat-utils";

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

function asRecord(value: unknown): Record<string, unknown> | null {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null;
  return value as Record<string, unknown>;
}

function pickString(...values: unknown[]): string | undefined {
  for (const value of values) {
    if (typeof value === "string" && value.trim()) return value.trim();
  }
  return undefined;
}

function summarizeResult(value: unknown, maxLength = 720): string | null {
  if (value === null || value === undefined) return null;
  const text = typeof value === "string" ? value : JSON.stringify(value, null, 2);
  if (!text) return null;
  return text.length > maxLength ? `${text.slice(0, maxLength)}...` : text;
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
    <div className="flex justify-end homie-message-in">
      <div className="max-w-[min(720px,85%)] rounded-[14px] bg-foreground/5 px-4 py-3 text-sm text-foreground">
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
      <div className="rounded-lg border border-amber-300/60 bg-amber-50/60 dark:bg-amber-500/10 p-3 text-sm">
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
      <div className="rounded-lg border border-border bg-card/40 p-3 text-sm">
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
      <div className="rounded-lg border border-border bg-card/40 p-3 text-sm">
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
      <div className="rounded-lg border border-border bg-card/40 p-3 text-sm">
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
      <div className="rounded-lg border border-border bg-card/40 p-3 text-sm">
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
      <div className="rounded-lg border border-border bg-card/40 p-3 text-sm">
        <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground mb-2">
          <FileDiff className="h-4 w-4" />
          Diff
        </div>
        <pre className="text-xs font-mono whitespace-pre-wrap">{item.text || ""}</pre>
      </div>
    );
  }

  if (item.kind === "tool") {
    const toolLabel = friendlyToolLabelFromItem(item);
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
    const toolDetail = renderToolDetail(item);
    return (
      <div className="rounded-lg border border-border bg-card/30 p-3 text-sm">
        <div className="flex items-center gap-2 text-xs uppercase tracking-wide text-muted-foreground mb-1">
          {statusIcon}
          Tool
        </div>
        <div className="flex flex-wrap items-center gap-2">
          <div className="text-sm">{toolLabel}</div>
          {intent && (
            <span className="rounded-full border border-border bg-muted/40 px-2 py-0.5 text-[11px] text-muted-foreground">
              {intent}
            </span>
          )}
        </div>
        {toolDetail}
      </div>
    );
  }

  if (item.kind === "system") {
    return (
      <div className="rounded-lg border border-border bg-card/30 p-3 text-sm text-muted-foreground">
        <div className="flex items-center gap-2 text-xs uppercase tracking-wide mb-1">
          <Globe className="h-4 w-4" />
          System
        </div>
        {item.text || ""}
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-border bg-card/30 p-3 text-sm text-muted-foreground">
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

function renderOpenClawBrowserDetail(
  input: Record<string, unknown> | null,
  result: Record<string, unknown> | null,
) {
  const data = asRecord(result?.data) ?? result;
  const action = pickString(input?.action, data?.action);
  const target = pickString(input?.target, data?.target);
  const targetUrl = pickString(input?.targetUrl, data?.url, data?.targetUrl);
  const message = pickString(data?.message, result?.message);

  const tabs = Array.isArray(data?.tabs) ? data.tabs.length : undefined;
  const profiles = Array.isArray(data?.profiles) ? data.profiles.length : undefined;
  const excerptSource = data ?? result;
  const excerpt = summarizeResult(excerptSource);

  return (
    <div className="mt-2 rounded-md border border-border bg-card/40 px-3 py-2 text-xs space-y-2">
      <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
        {action && (
          <span className="rounded-full border border-border bg-muted/40 px-2 py-0.5 uppercase tracking-wide">
            {action}
          </span>
        )}
        {target && (
          <span className="rounded-full border border-border bg-muted/40 px-2 py-0.5 uppercase tracking-wide">
            {target}
          </span>
        )}
        {targetUrl && (
          <a
            href={targetUrl}
            target="_blank"
            rel="noreferrer"
            className="inline-flex items-center gap-1 truncate text-foreground/80 hover:text-foreground"
          >
            <span className="truncate">{targetUrl}</span>
            <ExternalLink className="h-3 w-3" />
          </a>
        )}
      </div>
      {message && <div className="text-muted-foreground">{message}</div>}
      {(tabs !== undefined || profiles !== undefined) && (
        <div className="text-muted-foreground">
          {tabs !== undefined ? `${tabs} tab${tabs === 1 ? "" : "s"}` : ""}
          {tabs !== undefined && profiles !== undefined ? " • " : ""}
          {profiles !== undefined ? `${profiles} profile${profiles === 1 ? "" : "s"}` : ""}
        </div>
      )}
      {excerpt && (
        <pre className="max-h-52 overflow-auto whitespace-pre-wrap rounded border border-border bg-black/5 dark:bg-white/5 p-2 text-[11px] text-muted-foreground">
          {excerpt}
        </pre>
      )}
    </div>
  );
}

function renderToolDetail(item: ChatItem) {
  if (item.kind !== "tool") return null;
  const raw = asRecord(item.raw);
  if (!raw) return null;
  const rawToolName = rawToolNameFromItem(item);
  const normalizedTool = normalizeChatToolName(rawToolName);
  const tool = normalizedTool ?? rawToolName ?? item.text ?? "";
  const input = asRecord(raw.input);
  const result = asRecord(raw.result);
  const isError =
    Boolean(raw.error) ||
    item.status?.toLowerCase() === "failed" ||
    result?.ok === false;
  const errorRecord = asRecord(result?.error);
  const errorMessage = pickString(errorRecord?.message, result?.message, result?.error);
  if (isError && typeof errorMessage === "string" && errorMessage.trim()) {
    return (
      <div className="mt-2 rounded-md border border-destructive/30 bg-destructive/5 px-3 py-2 text-xs text-destructive">
        {errorMessage}
      </div>
    );
  }

  if (tool === "openclaw_browser") {
    return renderOpenClawBrowserDetail(input, result);
  }

  const payload = asRecord(result?.data) ?? result;

  if (tool === "web_search" && payload && typeof payload === "object") {
    const query = typeof payload.query === "string" ? payload.query : "";
    const provider = typeof payload.provider === "string" ? payload.provider : "";
    const count = typeof payload.count === "number" ? payload.count : undefined;
    const items = Array.isArray(payload.results) ? payload.results : [];
    const rows = items.slice(0, 5).filter((entry) => entry && typeof entry === "object");
    return (
      <div className="mt-2 space-y-2">
        <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
          {query && <span className="truncate">Query: {query}</span>}
          {provider && (
            <span className="rounded-full border border-border bg-muted/40 px-2 py-0.5 uppercase tracking-wide">
              {provider}
            </span>
          )}
          {typeof count === "number" && <span>{count} results</span>}
        </div>
        <div className="space-y-2">
          {rows.map((entry, index) => {
            const rec = entry as Record<string, unknown>;
            const title = typeof rec.title === "string" ? rec.title : "Untitled";
            const url = typeof rec.url === "string" ? rec.url : "";
            const snippet = typeof rec.snippet === "string" ? rec.snippet : "";
            const siteName = typeof rec.siteName === "string" ? rec.siteName : "";
            return (
              <div key={`${url}-${index}`} className="rounded-md border border-border bg-card/40 px-3 py-2 text-xs">
                <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
                  {siteName && <span className="truncate">{siteName}</span>}
                  {url && (
                    <a
                      href={url}
                      target="_blank"
                      rel="noreferrer"
                      className="inline-flex items-center gap-1 truncate text-foreground/80 hover:text-foreground"
                    >
                      <span className="truncate">{url}</span>
                      <ExternalLink className="h-3 w-3" />
                    </a>
                  )}
                </div>
                <div className="mt-1 text-foreground">{title}</div>
                {snippet && <div className="mt-1 text-muted-foreground">{snippet}</div>}
              </div>
            );
          })}
          {rows.length === 0 && (
            <div className="rounded-md border border-border bg-card/40 px-3 py-2 text-xs text-muted-foreground">
              No results.
            </div>
          )}
        </div>
      </div>
    );
  }

  if (tool === "web_fetch" && payload && typeof payload === "object") {
    const title = typeof payload.title === "string" ? payload.title : "";
    const url = typeof payload.finalUrl === "string" ? payload.finalUrl : "";
    const text = typeof payload.text === "string" ? payload.text : "";
    const truncated = payload.truncated === true;
    const excerpt = text ? text.slice(0, 420) : "";
    return (
      <div className="mt-2 rounded-md border border-border bg-card/40 px-3 py-2 text-xs space-y-2">
        <div className="flex flex-wrap items-center gap-2 text-[11px] text-muted-foreground">
          {title && <span className="text-foreground/80 truncate">{title}</span>}
          {url && (
            <a
              href={url}
              target="_blank"
              rel="noreferrer"
              className="inline-flex items-center gap-1 truncate text-foreground/80 hover:text-foreground"
            >
              <span className="truncate">{url}</span>
              <ExternalLink className="h-3 w-3" />
            </a>
          )}
          {truncated && <span className="rounded-full border border-border bg-muted/40 px-2 py-0.5">Truncated</span>}
        </div>
        {excerpt && <div className="text-muted-foreground whitespace-pre-wrap">{excerpt}</div>}
        {!excerpt && (
          <div className="text-muted-foreground">No extractable text returned.</div>
        )}
      </div>
    );
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
    <div className="flex justify-start homie-message-in">
      <div className="w-full max-w-[min(720px,85%)] space-y-3">
        {hasActivities && (
          <button
            type="button"
            onClick={() => setExpanded((prev) => !prev)}
            className="inline-flex max-w-full items-center gap-2 rounded-full border border-border/70 bg-card/40 px-3 py-1.5 text-left text-xs text-muted-foreground hover:bg-muted/40 transition-colors motion-reduce:transition-none"
          >
            <ChevronDown className={`h-3.5 w-3.5 shrink-0 transition-transform ${expanded ? "rotate-180" : ""}`} />
            <span className="truncate">{preview}</span>
            <span className="shrink-0 text-[10px] uppercase tracking-wide opacity-70">
              {activities.length} steps
            </span>
          </button>
        )}

        {!expanded && lastActivity && (
          <div className="rounded-lg border border-border bg-card/30 px-3 py-2 text-xs text-muted-foreground flex items-center gap-2 homie-fade-in">
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
          <div className="rounded-lg border border-border bg-card/20 px-3 py-2 text-xs text-muted-foreground flex items-center gap-2 homie-fade-in">
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
            data-expanded={expanded ? "true" : "false"}
            className={`homie-activity-collapse transition-opacity duration-200 ease-out motion-reduce:transition-none ${
              expanded ? "opacity-100" : "opacity-0"
            }`}
          >
            <div>
              <div className="space-y-3 pb-1">
                {nonApprovalActivities.map((item) => (
                  <ActivityRow key={item.id} item={item} onApprove={onApprove} />
                ))}
              </div>
            </div>
          </div>
        )}

        <div className="relative rounded-[14px] border border-border bg-muted/30 px-4 py-3 text-sm text-foreground overflow-hidden">
          <span aria-hidden="true" className="absolute left-0 top-2 bottom-2 w-[2px] rounded-full bg-primary/30" />
          {response ? (
            <div className="homie-fade-in">
              <ChatMarkdown content={response} />
            </div>
          ) : (
            <div className="text-sm text-muted-foreground inline-flex items-center gap-1.5">
              <span>{isStreaming ? "Thinking" : "Awaiting response"}</span>
              {isStreaming ? (
                <span className="homie-dots inline-flex items-center gap-1 align-middle" aria-hidden="true">
                  <span className="h-2 w-2 rounded-full bg-muted-foreground/70" />
                  <span className="h-2 w-2 rounded-full bg-muted-foreground/70" />
                  <span className="h-2 w-2 rounded-full bg-muted-foreground/70" />
                </span>
              ) : null}
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
