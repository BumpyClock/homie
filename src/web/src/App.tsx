import { useCallback, useEffect, useRef, useState } from 'react';
import { useGateway, type ConnectionStatus } from '@/hooks/use-gateway'
import { useTargets } from '@/hooks/use-targets'
import { TargetSelector } from '@/components/target-selector'
import { SessionList } from '@/components/session-list'
import { TerminalView, type AttachedSession } from '@/components/terminal-view'
import { ThemeSelector } from '@/components/theme-selector'
import { ArrowLeft, ChevronDown, Check, Trash2, X, RefreshCw, Plus, Terminal as TerminalIcon } from 'lucide-react';
import { PREVIEW_OPTIONS, PREVIEW_REFRESH_KEY, type PreviewRefresh, sessionDisplayName, shortSessionId } from '@/lib/session-utils';
import type { SessionInfo } from '@/lib/protocol';

function StatusDot({ status, className }: { status: ConnectionStatus; className?: string }) {
  const color =
    status === "connected"
      ? "bg-green-500"
      : status === "connecting" || status === "handshaking"
        ? "bg-yellow-500"
        : "bg-red-500";

  const shouldPulse = status === "connecting" || status === "handshaking";

  return (
    <span
      className={`inline-block rounded-full ${color} ${shouldPulse ? "animate-pulse motion-reduce:animate-none" : ""} ${className ?? "h-2.5 w-2.5"}`}
      role="img"
      aria-label={`Connection status: ${status}`}
      title={`Connection: ${status}`}
    />
  );
}

function App() {
  const {
    targets,
    activeTarget,
    activeTargetId,
    setActiveTargetId,
    addTarget,
    updateTarget,
    removeTarget,
    hideLocal,
    restoreLocal
  } = useTargets();
  const { status, serverHello, rejection, error, call, onBinaryMessage, onEvent } = useGateway({ url: activeTarget?.url ?? "" });
  const [attachedSessions, setAttachedSessions] = useState<AttachedSession[]>([]);
  const prevAttachedRef = useRef<string[]>([]);
  const previewNamespace = activeTargetId ?? "default";

  const [previewRefresh, setPreviewRefresh] = useState<PreviewRefresh>(() => {
    if (typeof window === "undefined") return "1m";
    const stored = window.localStorage.getItem(PREVIEW_REFRESH_KEY) as PreviewRefresh | null;
    return stored && PREVIEW_OPTIONS.some((o) => o.value === stored) ? stored : "1m";
  });
  const [refreshToken, setRefreshToken] = useState(0);

  const [terminalFocusSessionId, setTerminalFocusSessionId] = useState<string | null>(null);

  const [isSessionMenuOpen, setIsSessionMenuOpen] = useState(false);
  const sessionMenuTriggerRef = useRef<HTMLButtonElement | null>(null);
  const sessionMenuRef = useRef<HTMLDivElement | null>(null);
  const sessionMenuFirstItemRef = useRef<HTMLButtonElement | null>(null);
  const [runningSessions, setRunningSessions] = useState<SessionInfo[]>([]);
  const [runningSessionsLoading, setRunningSessionsLoading] = useState(false);
  const [runningSessionsError, setRunningSessionsError] = useState<string | null>(null);

  const fetchRunningSessions = useCallback(async () => {
    if (status !== 'connected') return;
    setRunningSessionsLoading(true);
    try {
      const res = await call('terminal.session.list') as { sessions?: SessionInfo[] };
      setRunningSessions(res.sessions ?? []);
      setRunningSessionsError(null);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : String(err);
      setRunningSessionsError(msg || 'Failed to list sessions');
    } finally {
      setRunningSessionsLoading(false);
    }
  }, [call, status]);

  const [isTargetOpen, setIsTargetOpen] = useState(false);
  const targetTriggerRef = useRef<HTMLButtonElement | null>(null);
  const targetPanelRef = useRef<HTMLDivElement | null>(null);

  const [detailsTargetId, setDetailsTargetId] = useState<string | null>(null);
  const [detailsName, setDetailsName] = useState('');
  const [detailsUrl, setDetailsUrl] = useState('');
  const detailsModalRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setIsTargetOpen(false);
  }, [activeTargetId]);

  useEffect(() => {
    setAttachedSessions([]);
  }, [activeTargetId]);

  useEffect(() => {
    if (attachedSessions.length > 0) return;
    setIsSessionMenuOpen(false);
    setTerminalFocusSessionId(null);
  }, [attachedSessions.length]);

  useEffect(() => {
    if (typeof window === "undefined") return;
    window.localStorage.setItem(PREVIEW_REFRESH_KEY, previewRefresh);
  }, [previewRefresh]);

  const closeDetails = () => {
    setDetailsTargetId(null);
    targetTriggerRef.current?.focus();
  };

  const detailsTarget = detailsTargetId ? targets.find((t) => t.id === detailsTargetId) ?? null : null;
  const showActiveGatewayInfo = !!detailsTarget && detailsTarget.id === activeTargetId && !!serverHello;

  useEffect(() => {
    if (!isTargetOpen) return;
    targetPanelRef.current?.focus();
  }, [isTargetOpen]);

  useEffect(() => {
    if (!isSessionMenuOpen) return;
    sessionMenuFirstItemRef.current?.focus();
  }, [isSessionMenuOpen]);

  useEffect(() => {
    if (!isSessionMenuOpen) return;
    void fetchRunningSessions();
  }, [isSessionMenuOpen, fetchRunningSessions, refreshToken]);

  useEffect(() => {
    if (!isSessionMenuOpen) return;

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsSessionMenuOpen(false);
        sessionMenuTriggerRef.current?.focus();
      }
    };

    const onPointerDown = (e: PointerEvent) => {
      const t = e.target as Node | null;
      if (!t) return;

      const inside = sessionMenuRef.current?.contains(t) || sessionMenuTriggerRef.current?.contains(t);
      if (!inside) setIsSessionMenuOpen(false);
    };

    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('pointerdown', onPointerDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('pointerdown', onPointerDown);
    };
  }, [isSessionMenuOpen]);

  useEffect(() => {
    if (!isTargetOpen) return;

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        setIsTargetOpen(false);
        targetTriggerRef.current?.focus();
      }
    };

    const onPointerDown = (e: PointerEvent) => {
      const t = e.target as Node | null;
      if (!t) return;

      const insideTarget = targetPanelRef.current?.contains(t) || targetTriggerRef.current?.contains(t);
      if (!insideTarget) setIsTargetOpen(false);
    };

    window.addEventListener('keydown', onKeyDown);
    window.addEventListener('pointerdown', onPointerDown);
    return () => {
      window.removeEventListener('keydown', onKeyDown);
      window.removeEventListener('pointerdown', onPointerDown);
    };
  }, [isTargetOpen]);

  useEffect(() => {
    if (!detailsTargetId) return;
    detailsModalRef.current?.focus();

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        closeDetails();
      }
    };

    window.addEventListener('keydown', onKeyDown);
    return () => window.removeEventListener('keydown', onKeyDown);
  }, [detailsTargetId]);

  useEffect(() => {
    if (detailsTargetId && !detailsTarget) {
      closeDetails();
    }
  }, [detailsTargetId, detailsTarget]);

  useEffect(() => {
    const prev = prevAttachedRef.current;
    const attachedSessionIds = attachedSessions.map((session) => session.id);
    const removed = prev.filter((id) => !attachedSessionIds.includes(id));
    if (removed.length > 0 && status === "connected") {
      removed.forEach((id) => {
        void call("terminal.session.detach", { session_id: id }).catch(() => {});
      });
    }
    prevAttachedRef.current = attachedSessionIds;
  }, [attachedSessions, status, call]);

  useEffect(() => {
    if (status !== "connected") return;

    void call("events.subscribe", { topic: "terminal.*" }).catch(() => {});
    const cleanup = onEvent((evt) => {
      if (
        evt.topic === "terminal.session.exit" ||
        evt.topic === "terminal.session.start" ||
        evt.topic === "terminal.session.rename"
      ) {
        setRefreshToken((t) => t + 1);
      }

      if (evt.topic === "terminal.session.rename") {
        const p = evt.params as { session_id?: string; name?: string | null } | undefined;
        if (p?.session_id) {
          handleRename(p.session_id, p.name ?? null);
        }
      }
    });

    return cleanup;
  }, [status, call, onEvent]);

  const handleAttach = (session: { session_id: string; shell: string; name?: string | null }) => {
    const label = sessionDisplayName(session);
    setAttachedSessions((prev) => {
      if (prev.some((item) => item.id === session.session_id)) return prev;
      return [...prev, { id: session.session_id, label }];
    });
  };

  const handleDetach = (sessionId: string) => {
      setAttachedSessions(prev => prev.filter(session => session.id !== sessionId));
  };

  const handleRename = (sessionId: string, name: string | null) => {
    const label = name && name.trim().length > 0 ? name.trim() : shortSessionId(sessionId);
    setAttachedSessions((prev) =>
      prev.map((session) => session.id === sessionId ? { ...session, label } : session)
    );
  };

  const handleStartSession = async () => {
    try {
      const session = await call('terminal.session.start', {
        cols: 80,
        rows: 24
      }) as { session_id?: string };

      setRefreshToken((t) => t + 1);

      if (session?.session_id) {
        const info = await call('terminal.session.attach', { session_id: session.session_id }) as {
          session_id?: string;
          shell?: string;
          name?: string | null;
        };
        if (info?.session_id) {
          handleAttach({ session_id: info.session_id, shell: info.shell ?? "", name: info.name });
          setTerminalFocusSessionId(info.session_id);
        }
      }
    } catch (err: unknown) {
      console.error("Failed to start session", err);
      const msg = err instanceof Error ? err.message : String(err);
      alert('Failed to start session: ' + msg);
    }
  };

  // If we have attached sessions, show the Terminal View (Full Screen)
  if (attachedSessions.length > 0) {
      const runningActive = runningSessions.filter((s) => s.status === 'active');

      const handleOpenSession = async (session: SessionInfo) => {
        setIsSessionMenuOpen(false);

        if (attachedSessions.some((s) => s.id === session.session_id)) {
          setTerminalFocusSessionId(session.session_id);
          return;
        }

        try {
          const info = await call('terminal.session.attach', { session_id: session.session_id }) as {
            session_id?: string;
            shell?: string;
            name?: string | null;
          };
          handleAttach({ session_id: info?.session_id ?? session.session_id, shell: info?.shell ?? session.shell, name: info?.name ?? session.name });
          setTerminalFocusSessionId(session.session_id);
        } catch (err: unknown) {
          console.error('Failed to attach session', err);
          const msg = err instanceof Error ? err.message : String(err);
          alert('Failed to attach session: ' + msg);
        }
      };

      return (
          <div className="h-screen w-screen flex flex-col bg-background overflow-hidden">
              <div className="flex items-center justify-between p-2 bg-muted/50 border-b border-border shrink-0">
                  <div className="flex items-center gap-4">
                    <button 
                        onClick={() => setAttachedSessions([])}
                        className="p-1 hover:bg-muted rounded text-muted-foreground hover:text-foreground"
                        title="Back to Dashboard"
                        aria-label="Back to dashboard"
                    >
                        <ArrowLeft size={20} />
                    </button>
                    <h1 className="text-sm font-bold text-foreground">Homie Terminal</h1>
                  </div>
                  <div className="relative flex items-center gap-2">
                      <button
                        ref={sessionMenuTriggerRef}
                        type="button"
                        onClick={() => setIsSessionMenuOpen((v) => !v)}
                        disabled={status !== 'connected'}
                        className="p-2 min-h-[44px] min-w-[44px] rounded text-muted-foreground hover:text-foreground hover:bg-muted/60 transition-colors disabled:opacity-50"
                        aria-haspopup="menu"
                        aria-expanded={isSessionMenuOpen}
                        aria-label="Open sessions menu"
                        title="Sessions"
                      >
                        <Plus className="w-5 h-5" />
                      </button>

                      {isSessionMenuOpen && (
                        <div
                          ref={sessionMenuRef}
                          tabIndex={-1}
                          role="menu"
                          aria-label="Sessions"
                          className="absolute right-0 top-full mt-2 w-[min(360px,calc(100vw-2rem))] max-h-[70vh] overflow-auto bg-popover border border-border rounded-lg shadow-lg outline-none origin-top-right homie-popover"
                        >
                          <div className="p-2">
                            <button
                              ref={sessionMenuFirstItemRef}
                              type="button"
                              role="menuitem"
                              onClick={() => {
                                setIsSessionMenuOpen(false);
                                void handleStartSession();
                              }}
                              disabled={status !== 'connected'}
                              className="w-full flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors disabled:opacity-50"
                            >
                              <Plus className="w-4 h-4" />
                              <span className="text-sm font-medium">New session</span>
                            </button>

                            <div className="mt-3 px-3 text-[11px] uppercase tracking-wide text-muted-foreground">Running sessions</div>

                            {runningSessionsError && (
                              <div className="mt-2 px-3 py-2 text-xs text-destructive">
                                {runningSessionsError}
                              </div>
                            )}

                            {runningSessionsLoading ? (
                              <div className="mt-2 px-3 py-2 text-xs text-muted-foreground">Loading…</div>
                            ) : runningActive.length === 0 ? (
                              <div className="mt-2 px-3 py-2 text-xs text-muted-foreground">No running sessions</div>
                            ) : (
                              <div className="mt-1">
                                {runningActive.map((session) => {
                                  const label = sessionDisplayName(session);
                                  const isOpen = attachedSessions.some((s) => s.id === session.session_id);
                                  return (
                                    <button
                                      key={session.session_id}
                                      type="button"
                                      role="menuitem"
                                      onClick={() => void handleOpenSession(session)}
                                      className="w-full flex items-center justify-between gap-3 px-3 py-2 min-h-[44px] rounded-md hover:bg-muted/60 transition-colors text-left"
                                    >
                                      <span className="flex items-center gap-2 min-w-0">
                                        <TerminalIcon className="w-4 h-4 text-muted-foreground" />
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

                      <ThemeSelector />
                      <StatusDot status={status} className="h-2.5 w-2.5" />
                  </div>
              </div>
              <div className="flex-1 min-h-0">
                  <TerminalView 
                    attachedSessions={attachedSessions}
                    onDetach={handleDetach}
                    call={call}
                    onBinaryMessage={onBinaryMessage}
                    previewNamespace={previewNamespace}
                    focusSessionId={terminalFocusSessionId}
                  />
              </div>
          </div>
      );
  }

  // Dashboard View
  return (
    <div className="min-h-screen bg-background text-foreground">
      <header className="border-b border-border bg-card/40 backdrop-blur">
        <div className="flex flex-wrap items-center justify-between gap-3 px-6 py-4">
          <div className="flex items-center gap-4">
            <div className="flex items-baseline gap-3">
              <div className="text-lg font-semibold">Homie Web</div>
              <div className="text-xs text-muted-foreground">Gateway Console</div>
            </div>

            <div className="relative">
              <button
                ref={targetTriggerRef}
                type="button"
                onClick={() => {
                  setIsTargetOpen((v) => !v);
                }}
                className="flex items-center gap-2 px-3 py-2 min-h-[44px] bg-card/60 border border-border rounded-md text-sm text-foreground hover:bg-card/80 transition-colors"
                aria-haspopup="dialog"
                aria-expanded={isTargetOpen}
              >
                <span className="text-muted-foreground">Target:</span>
                <span className="flex items-center gap-2 min-w-0">
                  <StatusDot status={status} className="h-2.5 w-2.5" />
                  <span className="max-w-[220px] truncate font-medium">{activeTarget?.name ?? 'Select'}</span>
                </span>
                <ChevronDown className={`w-4 h-4 text-muted-foreground transition-transform motion-reduce:transition-none ${isTargetOpen ? 'rotate-180' : ''}`} />
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
                      setActiveTargetId(id);
                      setIsTargetOpen(false);
                      targetTriggerRef.current?.focus();
                    }}
                    onDetails={(target) => {
                      setIsTargetOpen(false);
                      setDetailsTargetId(target.id);
                      setDetailsName(target.name);
                      setDetailsUrl(target.url);
                    }}
                    onAdd={addTarget}
                    onDelete={removeTarget}
                    hideLocal={hideLocal}
                    onRestoreLocal={restoreLocal}
                    connectionStatus={status}
                    serverHello={serverHello}
                    rejection={rejection}
                    error={error}
                  />
                </div>
              )}
            </div>
          </div>

          <div className="flex items-center gap-2">
            {serverHello && (
              <>
                <div className="hidden sm:flex items-center gap-2 bg-muted/40 border border-border rounded px-2">
                  <span className="text-[11px] uppercase tracking-wide text-muted-foreground">Preview</span>
                  <select
                    className="bg-transparent text-xs text-foreground py-2 pr-2"
                    value={previewRefresh}
                    onChange={(e) => setPreviewRefresh(e.target.value as PreviewRefresh)}
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
                  onClick={() => setRefreshToken((t) => t + 1)}
                  disabled={status !== 'connected'}
                  className="p-2 min-h-[44px] min-w-[44px] bg-muted hover:bg-muted/80 rounded text-muted-foreground disabled:opacity-50 transition-colors"
                  title="Refresh"
                  aria-label="Refresh sessions"
                >
                  <RefreshCw className="w-4 h-4" />
                </button>

                <button
                  type="button"
                  onClick={handleStartSession}
                  disabled={status !== 'connected'}
                  className="flex items-center gap-1 px-3 py-2 min-h-[44px] bg-primary hover:bg-primary/90 rounded text-primary-foreground text-sm font-medium transition-colors disabled:opacity-50"
                >
                  <Plus className="w-4 h-4" />
                  New Session
                </button>
              </>
            )}

            <ThemeSelector />
          </div>
        </div>
      </header>

      <main className="px-6 py-6 space-y-6">

        <section className="min-h-[400px]">
          {serverHello && (
            <SessionList
              call={call}
              status={status}
              onAttach={handleAttach}
              onRename={handleRename}
              previewNamespace={previewNamespace}
              previewRefresh={previewRefresh}
              refreshToken={refreshToken}
            />
          )}
        </section>
      </main>

      {detailsTarget && (
        <div
          className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
          onClick={closeDetails}
          role="presentation"
        >
          <div
            ref={detailsModalRef}
            tabIndex={-1}
            role="dialog"
            aria-modal="true"
            aria-label="Gateway details"
            className="w-full max-w-[680px] max-h-[85vh] overflow-auto bg-popover border border-border rounded-lg shadow-lg outline-none homie-popover"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="p-4 border-b border-border flex items-center justify-between gap-3">
              <div>
                <div className="text-sm font-semibold">Gateway Details</div>
                <div className="text-xs text-muted-foreground">Edit name / URL, or remove this gateway.</div>
              </div>
              <button
                type="button"
                onClick={closeDetails}
                className="p-2 min-h-[44px] min-w-[44px] rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/60 transition-colors"
                aria-label="Close"
              >
                <X className="w-4 h-4" />
              </button>
            </div>

            <div className="p-4 space-y-4">
              <div className="grid gap-3 sm:grid-cols-2">
                <div>
                  <label className="block text-xs text-muted-foreground mb-1">Name</label>
                  <input
                    type="text"
                    value={detailsName}
                    onChange={(e) => setDetailsName(e.target.value)}
                    className="w-full bg-background border border-border rounded px-2 py-2 text-sm text-foreground focus:outline-none focus:border-primary"
                  />
                </div>

                <div>
                  <label className="block text-xs text-muted-foreground mb-1">WS URL</label>
                  <input
                    type="text"
                    value={detailsUrl}
                    onChange={(e) => setDetailsUrl(e.target.value)}
                    disabled={detailsTarget.type === 'local'}
                    className="w-full bg-background border border-border rounded px-2 py-2 text-sm text-foreground focus:outline-none focus:border-primary disabled:opacity-60"
                  />
                  {detailsTarget.type === 'local' && (
                    <div className="text-[11px] text-muted-foreground mt-1">
                      Local gateway URL is derived automatically.
                    </div>
                  )}
                </div>
              </div>

              <div className="flex flex-wrap items-center justify-end gap-2">
                <button
                  type="button"
                  onClick={() => {
                    const label = detailsTarget.type === 'local' ? 'Hide local gateway?' : 'Remove this gateway?';
                    if (!confirm(label)) return;
                    removeTarget(detailsTarget.id);
                    closeDetails();
                  }}
                  className="flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md border border-border text-destructive hover:bg-destructive/10 transition-colors"
                >
                  <Trash2 className="w-4 h-4" />
                  {detailsTarget.type === 'local' ? 'Hide Local' : 'Remove'}
                </button>

                <button
                  type="button"
                  onClick={() => {
                    updateTarget(detailsTarget.id, { name: detailsName.trim(), url: detailsUrl.trim() });
                    closeDetails();
                  }}
                  className="flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
                >
                  <Check className="w-4 h-4" />
                  Save
                </button>
              </div>

              <div className="border-t border-border pt-4">
                <div className="text-xs uppercase tracking-wide text-muted-foreground mb-2">Connection / Server</div>
                {showActiveGatewayInfo ? (
                  <div className="text-sm">
                    <div className="grid grid-cols-2 gap-2">
                      <span className="text-muted-foreground">ID:</span>
                      <span className="font-mono text-xs break-all">{serverHello.server_id}</span>
                      <span className="text-muted-foreground">Protocol:</span>
                      <span>v{serverHello.protocol_version}</span>
                      {serverHello.identity && (
                        <>
                          <span className="text-muted-foreground">Identity:</span>
                          <span>{serverHello.identity}</span>
                        </>
                      )}
                    </div>

                    <div className="mt-3">
                      <div className="font-semibold mb-1">Services</div>
                      <ul className="list-disc list-inside text-muted-foreground">
                        {serverHello.services.map((s, i) => (
                          <li key={i}>{s.service} (v{s.version})</li>
                        ))}
                      </ul>
                    </div>
                  </div>
                ) : (
                  <div className="text-sm text-muted-foreground">
                    Select this gateway to connect and load server details.
                  </div>
                )}
              </div>
            </div>
          </div>
        </div>
      )}
    </div>
  )
}

export default App
