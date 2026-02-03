
import { useState, useEffect, useCallback, useRef, type RefObject } from "react";
import { TerminalTab } from "./terminal-tab";
import { parseBinaryFrame, StreamType } from "@/lib/binary-protocol";
import { sessionDisplayName } from "@/lib/session-utils";
import type { SessionInfo } from "@/lib/protocol";
import type { ConnectionStatus } from "@/hooks/use-gateway";
import { Plus, X, Terminal } from "lucide-react";

export interface AttachedSession {
  id: string;
  label: string;
}

interface TerminalSessionMenu {
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
}

interface TerminalViewProps {
  status: ConnectionStatus;
  attachedSessions: AttachedSession[];
  onDetach: (sessionId: string) => void;
  call: (method: string, params?: unknown) => Promise<unknown>;
  onBinaryMessage: (cb: (data: ArrayBuffer) => void) => () => void;
  previewNamespace: string;
  focusSessionId?: string | null;
  sessionMenu?: TerminalSessionMenu;
}

export function TerminalView({ status, attachedSessions, onDetach, call, onBinaryMessage, previewNamespace, focusSessionId, sessionMenu }: TerminalViewProps) {
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
  const pendingOutput = useRef<Map<string, { chunks: Uint8Array[]; bytes: number }>>(new Map());
  const pendingAttach = useRef<Set<string>>(new Set());
  const attaching = useRef<Set<string>>(new Set());

  const flushPending = useCallback((sessionId: string, listener: (data: Uint8Array) => void) => {
    const entry = pendingOutput.current.get(sessionId);
    if (!entry || entry.chunks.length === 0) return;
    pendingOutput.current.delete(sessionId);
    for (const chunk of entry.chunks) {
      listener(chunk);
    }
  }, []);

  const bufferPending = useCallback((sessionId: string, payload: Uint8Array) => {
    // Per-session cap: avoid unbounded memory when UI is slow to mount.
    const MAX_BYTES = 1024 * 1024; // 1MB
    const copy = payload.slice();
    const existing = pendingOutput.current.get(sessionId);
    if (!existing) {
      pendingOutput.current.set(sessionId, { chunks: [copy], bytes: copy.byteLength });
      return;
    }
    existing.chunks.push(copy);
    existing.bytes += copy.byteLength;
    while (existing.chunks.length > 0 && existing.bytes > MAX_BYTES) {
      const dropped = existing.chunks.shift();
      if (dropped) existing.bytes -= dropped.byteLength;
    }
  }, []);

  const attachSession = useCallback((sessionId: string) => {
    if (status !== "connected") return;
    if (attaching.current.has(sessionId)) return;

    attaching.current.add(sessionId);
    void call("terminal.session.attach", { session_id: sessionId, replay: true })
      .then(() => {
        pendingAttach.current.delete(sessionId);
      })
      .catch((err) => {
        console.warn("[terminal] attach failed", { sessionId, err });
        pendingAttach.current.add(sessionId);
      })
      .finally(() => {
        attaching.current.delete(sessionId);
      });
  }, [call, status]);

  useEffect(() => {
    // Clear tracking for sessions that were removed (so re-open reattaches + replays).
    const ids = new Set(attachedSessionIds);
    for (const id of Array.from(pendingAttach.current)) {
      if (!ids.has(id)) pendingAttach.current.delete(id);
    }
    for (const id of Array.from(attaching.current)) {
      if (!ids.has(id)) attaching.current.delete(id);
    }
  }, [attachedSessionIds]);

  useEffect(() => {
    if (status !== "connected") return;
    // Retry any pending attaches now that the socket is connected.
    for (const sessionId of pendingAttach.current) {
      attachSession(sessionId);
    }
    // Ensure sessions with mounted tabs are attached (dev StrictMode can mount/unmount quickly).
    for (const sessionId of tabListeners.current.keys()) {
      attachSession(sessionId);
    }
  }, [status, attachSession]);

  useEffect(() => {
    const cleanup = onBinaryMessage((buffer) => {
      try {
        const frame = parseBinaryFrame(buffer);
        // Only handle stdout/stderr for display
        if (frame.stream === StreamType.Stdout || frame.stream === StreamType.Stderr) {
             const listener = tabListeners.current.get(frame.sessionId);
             if (listener) {
                 flushPending(frame.sessionId, listener);
                 listener(frame.payload);
             } else {
                 bufferPending(frame.sessionId, frame.payload);
             }
        }
      } catch (e) {
        console.error("Failed to parse binary frame", e);
      }
    });
    return cleanup;
  }, [onBinaryMessage, bufferPending, flushPending]);

  const registerTabListener = useCallback((sessionId: string, listener: (data: Uint8Array) => void) => {
    tabListeners.current.set(sessionId, listener);
    flushPending(sessionId, listener);
    pendingAttach.current.add(sessionId);
    attachSession(sessionId);
    return () => {
      tabListeners.current.delete(sessionId);
      pendingAttach.current.delete(sessionId);
    };
  }, [flushPending, attachSession]);

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
      <div className="flex items-center bg-muted/50 border-b border-border">
        <div className="flex-1 flex items-center overflow-x-auto">
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

        {sessionMenu && (
          <div className="relative shrink-0 pr-2">
            <button
              ref={sessionMenu.triggerRef}
              type="button"
              onClick={sessionMenu.onToggle}
              className="p-2 min-h-[44px] min-w-[44px] rounded text-muted-foreground hover:text-foreground hover:bg-muted/60 transition-colors"
              aria-haspopup="menu"
              aria-expanded={sessionMenu.isOpen}
              aria-label="Open sessions menu"
              title="Sessions"
            >
              <Plus className="w-5 h-5" />
            </button>

            {sessionMenu.isOpen && (
              <div
                ref={sessionMenu.menuRef}
                tabIndex={-1}
                role="menu"
                aria-label="Sessions"
                className="absolute right-0 top-full mt-2 z-50 w-[min(360px,calc(100vw-2rem))] max-h-[70vh] overflow-auto bg-popover border border-border rounded-lg shadow-lg outline-none origin-top-right homie-popover"
              >
                <div className="p-2">
                  <button
                    ref={sessionMenu.firstItemRef}
                    type="button"
                    role="menuitem"
                    onClick={() => {
                      sessionMenu.onClose();
                      void sessionMenu.onStartNewSession();
                    }}
                    className="w-full flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
                  >
                    <Plus className="w-4 h-4" />
                    <span className="text-sm font-medium">New session</span>
                  </button>

                  <div className="mt-3 px-3 text-[11px] uppercase tracking-wide text-muted-foreground">Running sessions</div>

                  {sessionMenu.error && (
                    <div className="mt-2 px-3 py-2 text-xs text-destructive">
                      {sessionMenu.error}
                    </div>
                  )}

                  {sessionMenu.loading ? (
                    <div className="mt-2 px-3 py-2 text-xs text-muted-foreground">Loading…</div>
                  ) : sessionMenu.sessions.length === 0 ? (
                    <div className="mt-2 px-3 py-2 text-xs text-muted-foreground">No running sessions</div>
                  ) : (
                    <div className="mt-1">
                      {sessionMenu.sessions.map((session) => {
                        const label = sessionDisplayName(session);
                        const isOpen = attachedSessionIds.includes(session.session_id);
                        return (
                          <button
                            key={session.session_id}
                            type="button"
                            role="menuitem"
                            onClick={() => {
                              sessionMenu.onClose();
                              void sessionMenu.onOpenSession(session);
                            }}
                            className="w-full flex items-center justify-between gap-3 px-3 py-2 min-h-[44px] rounded-md hover:bg-muted/60 transition-colors text-left"
                          >
                            <span className="flex items-center gap-2 min-w-0">
                              <Terminal className="w-4 h-4 text-muted-foreground" />
                              <span className="text-sm text-foreground truncate">{label}</span>
                            </span>
                            {isOpen && (
                              <span className="text-[11px] text-muted-foreground">Open</span>
                            )}
                          </button>
                        );
                      })}
                    </div>
                  )}
                </div>
              </div>
            )}
          </div>
        )}
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
            className="px-3 sm:px-4 py-2 min-h-[44px] bg-card hover:bg-muted text-foreground border border-border rounded text-xs font-mono font-bold shadow-sm active:transform active:scale-95 transition-colors"
        >
            {label}
        </button>
    )
}
