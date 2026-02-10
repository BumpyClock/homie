import type { RefObject } from "react";
import type { ServerHello, HelloReject } from "@homie/shared";
import type { Target } from "@/hooks/use-targets";
import type { ConnectionStatus } from "@/hooks/use-gateway";
import { TargetSelector } from "@/components/target-selector";
import { ThemeSelector } from "@/components/theme-selector";
import { StatusDot } from "@/components/status-dot";
import { ChevronDown, RefreshCw, Plus, MessageSquareText, TerminalSquare } from "lucide-react";
import { PREVIEW_OPTIONS, type PreviewRefresh } from "@/lib/session-utils";

interface GatewayHeaderProps {
  status: ConnectionStatus;
  serverHello: ServerHello | null;
  rejection: HelloReject | null;
  error: Event | null;
  targets: Target[];
  activeTarget: Target | null;
  activeTargetId: string;
  isTargetOpen: boolean;
  setIsTargetOpen: (open: boolean) => void;
  targetTriggerRef: RefObject<HTMLButtonElement | null>;
  targetPanelRef: RefObject<HTMLDivElement | null>;
  onSelectTarget: (id: string) => void;
  onDetailsTarget: (target: Target) => void;
  onAddTarget: (name: string, url: string) => void;
  onRemoveTarget: (id: string) => void;
  hideLocal: boolean;
  onRestoreLocal: () => void;
  previewRefresh: PreviewRefresh;
  onPreviewRefresh: (value: PreviewRefresh) => void;
  onRefreshSessions: () => void;
  onStartSession: () => void;
  activeTab: "terminals" | "chat";
  setActiveTab: (tab: "terminals" | "chat") => void;
  hasChatService: boolean;
}

export function GatewayHeader({
  status,
  serverHello,
  rejection,
  error,
  targets,
  activeTarget,
  activeTargetId,
  isTargetOpen,
  setIsTargetOpen,
  targetTriggerRef,
  targetPanelRef,
  onSelectTarget,
  onDetailsTarget,
  onAddTarget,
  onRemoveTarget,
  hideLocal,
  onRestoreLocal,
  previewRefresh,
  onPreviewRefresh,
  onRefreshSessions,
  onStartSession,
  activeTab,
  setActiveTab,
  hasChatService,
}: GatewayHeaderProps) {
  return (
    <header className="border-b border-border bg-card/40 backdrop-blur">
      <div className="flex flex-col gap-3 px-4 sm:px-6 py-3 sm:py-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div className="flex items-baseline gap-3">
            <div className="text-base sm:text-lg font-semibold">Homie Web</div>
            <div className="text-xs text-muted-foreground">Gateway Console</div>
          </div>
          <div className="flex items-center gap-2">
            <ThemeSelector />
          </div>
        </div>

        <div className="flex flex-wrap items-center gap-3">
          <div className="relative flex-1 min-w-[220px]">
            <button
              ref={targetTriggerRef}
              type="button"
              onClick={() => setIsTargetOpen(!isTargetOpen)}
              className="w-full flex items-center gap-2 px-3 py-2 min-h-[44px] bg-card/60 border border-border rounded-md text-sm text-foreground hover:bg-card/80 transition-colors"
              aria-haspopup="dialog"
              aria-expanded={isTargetOpen}
            >
              <span className="text-muted-foreground">Target:</span>
              <span className="flex items-center gap-2 min-w-0">
                <StatusDot status={status} className="h-2.5 w-2.5" />
                <span className="max-w-[220px] truncate font-medium">{activeTarget?.name ?? "Select"}</span>
              </span>
              <ChevronDown
                className={`w-4 h-4 text-muted-foreground transition-transform motion-reduce:transition-none ${
                  isTargetOpen ? "rotate-180" : ""
                }`}
              />
            </button>

            {isTargetOpen && (
              <div
                ref={targetPanelRef}
                tabIndex={-1}
                className="absolute left-0 mt-2 w-[min(420px,calc(100vw-3rem))] max-h-[70vh] overflow-auto bg-popover border border-border rounded-lg shadow-sm p-4 outline-none origin-top-left homie-popover"
                role="dialog"
                aria-label="Target selector"
              >
                <TargetSelector
                  targets={targets}
                  activeTargetId={activeTargetId}
                  onSelect={(id) => {
                    onSelectTarget(id);
                    setIsTargetOpen(false);
                    targetTriggerRef.current?.focus();
                  }}
                  onDetails={(target) => {
                    setIsTargetOpen(false);
                    onDetailsTarget(target);
                  }}
                  onAdd={onAddTarget}
                  onDelete={onRemoveTarget}
                  hideLocal={hideLocal}
                  onRestoreLocal={onRestoreLocal}
                  connectionStatus={status}
                  serverHello={serverHello}
                  rejection={rejection}
                  error={error}
                />
              </div>
            )}
          </div>

          {hasChatService && (
            <div className="flex items-center gap-1 rounded-md border border-border bg-muted/40 p-1">
              <button
                type="button"
                onClick={() => setActiveTab("terminals")}
                className={`flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md text-sm transition-colors ${
                  activeTab === "terminals"
                    ? "bg-background text-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground"
                }`}
                aria-pressed={activeTab === "terminals"}
              >
                <TerminalSquare className="w-4 h-4" />
                <span className="hidden sm:inline">Terminals</span>
              </button>
              <button
                type="button"
                onClick={() => setActiveTab("chat")}
                className={`flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md text-sm transition-colors ${
                  activeTab === "chat"
                    ? "bg-background text-foreground shadow-sm"
                    : "text-muted-foreground hover:text-foreground"
                }`}
                aria-pressed={activeTab === "chat"}
              >
                <MessageSquareText className="w-4 h-4" />
                <span className="hidden sm:inline">Chat</span>
              </button>
            </div>
          )}

          {serverHello && activeTab === "terminals" && (
            <div className="flex flex-wrap items-center gap-2">
              <div className="hidden sm:flex items-center gap-2 bg-muted/40 border border-border rounded px-2">
                <span className="text-[11px] uppercase tracking-wide text-muted-foreground">Preview</span>
                <select
                  className="bg-transparent text-xs text-foreground py-2 pr-2"
                  value={previewRefresh}
                  onChange={(e) => onPreviewRefresh(e.target.value as PreviewRefresh)}
                  aria-label="Preview refresh cadence"
                >
                  {PREVIEW_OPTIONS.map((opt) => (
                    <option key={opt.value} value={opt.value}>
                      {opt.label}
                    </option>
                  ))}
                </select>
              </div>

              <button
                type="button"
                onClick={onRefreshSessions}
                disabled={status !== "connected"}
                className="p-2 min-h-[44px] min-w-[44px] bg-muted hover:bg-muted/80 rounded text-muted-foreground disabled:opacity-50 transition-colors"
                title="Refresh"
                aria-label="Refresh sessions"
              >
                <RefreshCw className="w-4 h-4" />
              </button>

              <button
                type="button"
                onClick={onStartSession}
                disabled={status !== "connected"}
                className="flex items-center gap-1 px-3 py-2 min-h-[44px] bg-primary hover:bg-primary/90 rounded text-primary-foreground text-sm font-medium transition-colors disabled:opacity-50"
              >
                <Plus className="w-4 h-4" />
                <span className="hidden sm:inline">New Session</span>
                <span className="sm:hidden">New</span>
              </button>
            </div>
          )}
        </div>
      </div>
    </header>
  );
}
