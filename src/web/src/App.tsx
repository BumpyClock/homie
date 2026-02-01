import { useCallback, useEffect, useRef, useState } from 'react';
import { useGateway } from '@/hooks/use-gateway'
import { useTargets } from '@/hooks/use-targets'
import { SessionList } from '@/components/session-list'
import type { AttachedSession } from '@/components/terminal-view'
import { PREVIEW_OPTIONS, PREVIEW_REFRESH_KEY, type PreviewRefresh, sessionDisplayName, shortSessionId } from '@/lib/session-utils';
import type { SessionInfo } from '@/lib/protocol';
import { TerminalScreen } from '@/components/terminal-screen';
import { GatewayDetailsModal } from '@/components/gateway-details-modal';
import { ChatPanel } from '@/components/chat-panel';
import { GatewayHeader } from '@/components/gateway-header';

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
  const [activeTab, setActiveTab] = useState<"terminals" | "chat">("terminals");

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
  const hasChatService = !!serverHello?.services?.some((service) => service.service === "chat");

  useEffect(() => {
    if (!isTargetOpen) return;
    targetPanelRef.current?.focus();
  }, [isTargetOpen]);

  useEffect(() => {
    if (!isSessionMenuOpen) return;
    sessionMenuFirstItemRef.current?.focus();
  }, [isSessionMenuOpen]);

  useEffect(() => {
    if (isSessionMenuOpen) return;
    sessionMenuTriggerRef.current?.focus();
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
    if (detailsTargetId && !detailsTarget) {
      closeDetails();
    }
  }, [detailsTargetId, detailsTarget]);

  useEffect(() => {
    if (activeTab === "chat" && !hasChatService) {
      setActiveTab("terminals");
    }
  }, [activeTab, hasChatService]);

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
        <TerminalScreen
          status={status}
          attachedSessions={attachedSessions}
          onBack={() => setAttachedSessions([])}
          onDetach={handleDetach}
          call={call}
          onBinaryMessage={onBinaryMessage}
          previewNamespace={previewNamespace}
          focusSessionId={terminalFocusSessionId}
          sessionMenu={{
            isOpen: isSessionMenuOpen,
            onToggle: () => setIsSessionMenuOpen((v) => !v),
            onClose: () => setIsSessionMenuOpen(false),
            triggerRef: sessionMenuTriggerRef,
            menuRef: sessionMenuRef,
            firstItemRef: sessionMenuFirstItemRef,
            sessions: runningActive,
            loading: runningSessionsLoading,
            error: runningSessionsError,
            onStartNewSession: handleStartSession,
            onOpenSession: handleOpenSession,
          }}
        />
      );
  }

  // Dashboard View
  return (
    <div className="min-h-screen bg-background text-foreground flex flex-col">
      <GatewayHeader
        status={status}
        serverHello={serverHello}
        rejection={rejection}
        error={error}
        targets={targets}
        activeTarget={activeTarget}
        activeTargetId={activeTargetId}
        isTargetOpen={isTargetOpen}
        setIsTargetOpen={setIsTargetOpen}
        targetTriggerRef={targetTriggerRef}
        targetPanelRef={targetPanelRef}
        onSelectTarget={setActiveTargetId}
        onDetailsTarget={(target) => {
          setDetailsTargetId(target.id);
          setDetailsName(target.name);
          setDetailsUrl(target.url);
        }}
        onAddTarget={addTarget}
        onRemoveTarget={removeTarget}
        hideLocal={hideLocal}
        onRestoreLocal={restoreLocal}
        previewRefresh={previewRefresh}
        onPreviewRefresh={setPreviewRefresh}
        onRefreshSessions={() => setRefreshToken((t) => t + 1)}
        onStartSession={handleStartSession}
        activeTab={activeTab}
        setActiveTab={setActiveTab}
        hasChatService={hasChatService}
      />

      <main className="flex-1 min-h-0 px-6 py-6 space-y-6">
        {activeTab === "terminals" && (
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
        )}

        {activeTab === "chat" && hasChatService && (
          <section className="min-h-[400px] h-full">
            <ChatPanel
              status={status}
              call={call}
              onEvent={onEvent}
              enabled={hasChatService}
              namespace={previewNamespace}
            />
          </section>
        )}
      </main>

      {detailsTarget && (
        <GatewayDetailsModal
          target={detailsTarget}
          detailsName={detailsName}
          detailsUrl={detailsUrl}
          onNameChange={setDetailsName}
          onUrlChange={setDetailsUrl}
          onRemove={() => {
            removeTarget(detailsTarget.id);
            closeDetails();
          }}
          onSave={() => {
            updateTarget(detailsTarget.id, { name: detailsName.trim(), url: detailsUrl.trim() });
            closeDetails();
          }}
          onClose={closeDetails}
          serverHello={serverHello}
          showActiveGatewayInfo={showActiveGatewayInfo}
        />
      )}
    </div>
  )
}

export default App
