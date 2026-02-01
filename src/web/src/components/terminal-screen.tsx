import type { RefObject } from "react";
import { ArrowLeft } from "lucide-react";
import { ThemeSelector } from "@/components/theme-selector";
import { StatusDot } from "@/components/status-dot";
import { TerminalView, type AttachedSession } from "@/components/terminal-view";
import type { ConnectionStatus } from "@/hooks/use-gateway";
import type { SessionInfo } from "@/lib/protocol";

interface TerminalScreenProps {
  status: ConnectionStatus;
  attachedSessions: AttachedSession[];
  onBack: () => void;
  onDetach: (sessionId: string) => void;
  call: (method: string, params?: unknown) => Promise<unknown>;
  onBinaryMessage: (callback: (data: ArrayBuffer) => void) => () => void;
  previewNamespace: string;
  focusSessionId?: string | null;
  sessionMenu?: {
    isOpen: boolean;
    onToggle: () => void;
    onClose: () => void;
    triggerRef: RefObject<HTMLButtonElement | null>;
    menuRef: RefObject<HTMLDivElement | null>;
    firstItemRef: RefObject<HTMLButtonElement | null>;
    sessions: SessionInfo[];
    loading: boolean;
    error: string | null;
    onStartNewSession: () => void | Promise<void>;
    onOpenSession: (session: SessionInfo) => void | Promise<void>;
  };
}

export function TerminalScreen({
  status,
  attachedSessions,
  onBack,
  onDetach,
  call,
  onBinaryMessage,
  previewNamespace,
  focusSessionId,
  sessionMenu,
}: TerminalScreenProps) {
  return (
    <div className="h-screen w-screen flex flex-col bg-background overflow-hidden">
      <div className="flex items-center justify-between p-2 bg-muted/50 border-b border-border shrink-0">
        <div className="flex items-center gap-4">
          <button
            onClick={onBack}
            className="p-1 hover:bg-muted rounded text-muted-foreground hover:text-foreground transition-colors"
            title="Back to dashboard"
            aria-label="Back to dashboard"
          >
            <ArrowLeft size={20} />
          </button>
          <h1 className="text-sm font-bold text-foreground">Homie Terminal</h1>
        </div>
        <div className="flex items-center gap-4">
          <ThemeSelector />
          <StatusDot status={status} className="h-2.5 w-2.5" />
        </div>
      </div>
      <div className="flex-1 min-h-0">
        <TerminalView
          attachedSessions={attachedSessions}
          onDetach={onDetach}
          call={call}
          onBinaryMessage={onBinaryMessage}
          previewNamespace={previewNamespace}
          focusSessionId={focusSessionId}
          sessionMenu={sessionMenu}
        />
      </div>
    </div>
  );
}
