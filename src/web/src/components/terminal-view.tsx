
import { useState, useEffect, useCallback, useRef } from "react";
import { TerminalTab } from "./terminal-tab";
import { parseBinaryFrame, StreamType } from "@/lib/binary-protocol";
import { X, Terminal } from "lucide-react";

export interface AttachedSession {
  id: string;
  label: string;
}

interface TerminalViewProps {
  attachedSessions: AttachedSession[];
  onDetach: (sessionId: string) => void;
  call: (method: string, params?: unknown) => Promise<unknown>;
  onBinaryMessage: (cb: (data: ArrayBuffer) => void) => () => void;
  previewNamespace: string;
  focusSessionId?: string | null;
}

export function TerminalView({ attachedSessions, onDetach, call, onBinaryMessage, previewNamespace, focusSessionId }: TerminalViewProps) {
  const [userActiveSessionId, setUserActiveSessionId] = useState<string | null>(null);

  const attachedSessionIds = attachedSessions.map((session) => session.id);

  useEffect(() => {
    if (!focusSessionId) return;
    if (!attachedSessionIds.includes(focusSessionId)) return;
    setUserActiveSessionId(focusSessionId);
  }, [focusSessionId, attachedSessionIds]);

  // Derive the effective active session ID
  const activeSessionId = (userActiveSessionId && attachedSessionIds.includes(userActiveSessionId))
    ? userActiveSessionId
    : (attachedSessionIds.length > 0 ? attachedSessionIds[0] : null);

  const tabListeners = useRef<Map<string, (data: Uint8Array) => void>>(new Map());

  useEffect(() => {
    const cleanup = onBinaryMessage((buffer) => {
      try {
        const frame = parseBinaryFrame(buffer);
        // Only handle stdout/stderr for display
        if (frame.stream === StreamType.Stdout || frame.stream === StreamType.Stderr) {
             const listener = tabListeners.current.get(frame.sessionId);
             if (listener) {
                 listener(frame.payload);
             }
        }
      } catch (e) {
        console.error("Failed to parse binary frame", e);
      }
    });
    return cleanup;
  }, [onBinaryMessage]);

  const registerTabListener = useCallback((sessionId: string, listener: (data: Uint8Array) => void) => {
    tabListeners.current.set(sessionId, listener);
    return () => {
      tabListeners.current.delete(sessionId);
    };
  }, []);

  const handleInput = useCallback((sessionId: string, data: string) => {
    void call("terminal.session.input", { session_id: sessionId, data }).catch(() => {});
  }, [call]);

  const handleResize = useCallback((sessionId: string, cols: number, rows: number) => {
    void call("terminal.session.resize", { session_id: sessionId, cols, rows }).catch(() => {});
  }, [call]);

  const handleKeybarAction = async (action: string) => {
    if (!activeSessionId) return;
    
    let sequence = "";
    switch (action) {
        case "esc": sequence = "\x1b"; break;
        case "tab": sequence = "\t"; break;
        case "up": sequence = "\x1b[A"; break;
        case "down": sequence = "\x1b[B"; break;
        case "left": sequence = "\x1b[D"; break;
        case "right": sequence = "\x1b[C"; break;
        case "ctrl+c": sequence = "\x03"; break;
        case "paste":
            try {
                const text = await navigator.clipboard.readText();
                if (text) handleInput(activeSessionId, text);
            } catch (err) {
                console.error("Failed to read clipboard", err);
            }
            return;
        default: return;
    }
    handleInput(activeSessionId, sequence);
  };

  if (attachedSessions.length === 0) {
    return <div className="text-muted-foreground text-center p-10">No active terminal sessions</div>;
  }

  return (
    <div className="flex flex-col h-full w-full bg-background">
      {/* Tab Bar */}
      <div className="flex items-center bg-muted/50 border-b border-border overflow-x-auto">
        {attachedSessions.map((session) => (
          <div
            key={session.id}
            className={`
              flex items-center gap-2 px-4 py-2 text-sm cursor-pointer select-none
              ${activeSessionId === session.id ? "bg-card text-foreground border-t-2 border-primary" : "text-muted-foreground hover:bg-muted/80"}
            `}
            onClick={() => setUserActiveSessionId(session.id)}
          >
            <Terminal size={14} />
            <span className="max-w-[150px] truncate">{session.label}</span>
            <button
              onClick={(e) => {
                e.stopPropagation();
                onDetach(session.id);
              }}
              className="p-1 hover:bg-muted rounded-full"
              aria-label="Close session"
            >
              <X size={12} />
            </button>
          </div>
        ))}
      </div>

      {/* Terminal Content */}
      <div className="flex-1 relative min-h-0">
        {attachedSessionIds.map((sessionId) => (
          <TerminalTab
            key={sessionId}
            sessionId={sessionId}
            active={activeSessionId === sessionId}
            onInput={(data) => handleInput(sessionId, data)}
            onResize={(cols, rows) => handleResize(sessionId, cols, rows)}
            registerDataListener={(listener) => registerTabListener(sessionId, listener)}
            previewNamespace={previewNamespace}
          />
        ))}
      </div>

      {/* Keybar */}
      <div className="bg-muted/50 border-t border-border p-2 flex gap-2 overflow-x-auto">
          <KeyButton label="ESC" onClick={() => handleKeybarAction("esc")} />
          <KeyButton label="TAB" onClick={() => handleKeybarAction("tab")} />
          <KeyButton label="CTRL+C" onClick={() => handleKeybarAction("ctrl+c")} />
          <KeyButton label="PASTE" onClick={() => handleKeybarAction("paste")} />
          <div className="w-px bg-border mx-1" />
          <KeyButton label="←" onClick={() => handleKeybarAction("left")} />
          <KeyButton label="↓" onClick={() => handleKeybarAction("down")} />
          <KeyButton label="↑" onClick={() => handleKeybarAction("up")} />
          <KeyButton label="→" onClick={() => handleKeybarAction("right")} />
      </div>
    </div>
  );
}

function KeyButton({ label, onClick }: { label: string; onClick: () => void }) {
    return (
        <button 
            onClick={onClick}
            className="px-4 py-2 bg-card hover:bg-muted text-foreground border border-border rounded text-xs font-mono font-bold shadow-sm active:transform active:scale-95 transition-colors"
        >
            {label}
        </button>
    )
}
