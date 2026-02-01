import type { ChatItem } from "@/hooks/use-chat";

interface ChatItemProps {
  item: ChatItem;
  onApprove?: (requestId: number, decision: "accept" | "decline") => void;
}

export function ChatItemView({ item, onApprove }: ChatItemProps) {
  if (item.kind === "user" || item.kind === "assistant") {
    const isUser = item.kind === "user";
    return (
      <div className={`flex ${isUser ? "justify-end" : "justify-start"}`}>
        <div
          className={`max-w-[720px] whitespace-pre-wrap rounded-lg border px-4 py-3 text-sm leading-relaxed ${
            isUser
              ? "bg-primary text-primary-foreground border-primary/30"
              : "bg-muted/40 border-border text-foreground"
          }`}
        >
          <div className="text-[11px] uppercase tracking-wide opacity-70 mb-1">
            {isUser ? "You" : "Agent"}
          </div>
          {item.text || ""}
        </div>
      </div>
    );
  }

  if (item.kind === "approval") {
    return (
      <div className="rounded-lg border border-amber-300/60 bg-amber-50/60 dark:bg-amber-500/10 p-4 text-sm">
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
            onClick={() => item.requestId && onApprove?.(item.requestId, "accept")}
            className="px-3 py-2 min-h-[44px] rounded-md bg-amber-500 text-white text-sm font-medium hover:bg-amber-600 transition-colors"
          >
            Approve
          </button>
          <button
            type="button"
            onClick={() => item.requestId && onApprove?.(item.requestId, "decline")}
            className="px-3 py-2 min-h-[44px] rounded-md border border-amber-300 text-amber-700 dark:text-amber-200 hover:bg-amber-100/70 dark:hover:bg-amber-500/20 transition-colors"
          >
            Decline
          </button>
        </div>
      </div>
    );
  }

  if (item.kind === "command") {
    return (
      <div className="rounded-lg border border-border bg-card/60 p-4 text-sm">
        <div className="flex items-center justify-between mb-2">
          <div className="text-xs uppercase tracking-wide text-muted-foreground">Command</div>
          {item.status && (
            <span className="text-xs px-2 py-1 rounded-full bg-muted text-muted-foreground">
              {item.status}
            </span>
          )}
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
      <div className="rounded-lg border border-border bg-card/60 p-4 text-sm">
        <div className="flex items-center justify-between mb-2">
          <div className="text-xs uppercase tracking-wide text-muted-foreground">File changes</div>
          {item.status && (
            <span className="text-xs px-2 py-1 rounded-full bg-muted text-muted-foreground">
              {item.status}
            </span>
          )}
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
      <div className="rounded-lg border border-border bg-card/60 p-4 text-sm">
        <div className="text-xs uppercase tracking-wide text-muted-foreground mb-2">Plan</div>
        <pre className="text-xs font-mono whitespace-pre-wrap">{item.text || ""}</pre>
      </div>
    );
  }

  if (item.kind === "diff") {
    return (
      <div className="rounded-lg border border-border bg-card/60 p-4 text-sm">
        <div className="text-xs uppercase tracking-wide text-muted-foreground mb-2">Diff</div>
        <pre className="text-xs font-mono whitespace-pre-wrap">{item.text || ""}</pre>
      </div>
    );
  }

  if (item.kind === "reasoning") {
    return (
      <div className="rounded-lg border border-border bg-card/60 p-4 text-sm">
        <div className="text-xs uppercase tracking-wide text-muted-foreground mb-2">Reasoning</div>
        {item.summary && item.summary.length > 0 && (
          <div className="mb-3 space-y-1">
            {item.summary.map((line, idx) => (
              <div key={idx} className="text-sm">{line}</div>
            ))}
          </div>
        )}
        {item.content && item.content.length > 0 && (
          <pre className="text-xs font-mono whitespace-pre-wrap text-muted-foreground">
            {item.content.join("\n")}
          </pre>
        )}
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-border bg-card/50 p-4 text-sm text-muted-foreground">
      {item.text || "Unsupported item"}
    </div>
  );
}
