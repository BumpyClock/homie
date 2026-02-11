import { useEffect, useMemo, useState } from 'react';
import { StyleSheet, View, useWindowDimensions } from 'react-native';

import { AppShell } from '@/components/shell/AppShell';
import { useMobileShellData } from '@/components/shell/MobileShellDataContext';
import { TerminalSessionList } from '@/components/shell/TerminalSessionList';
import { MobileTerminalPane } from '@/components/terminal/MobileTerminalPane';
import { ActionButton } from '@/components/ui/ActionButton';
import { spacing } from '@/theme/tokens';

export default function TerminalsTabScreen() {
  const { width } = useWindowDimensions();
  const {
    loadingTarget,
    hasTarget,
    status,
    statusBadge,
    error,
    loadingTerminals,
    terminalSessions,
    tmuxSessions,
    tmuxSupported,
    tmuxError,
    refreshTerminals,
    startTerminalSession,
    attachTmuxSession,
    attachTerminalSession,
    resizeTerminalSession,
    sendTerminalInput,
    onTerminalBinary,
  } = useMobileShellData();
  const [activeTerminalSessionId, setActiveTerminalSessionId] = useState<string | null>(null);

  useEffect(() => {
    if (!hasTarget || status !== 'connected') return;
    void refreshTerminals();
  }, [hasTarget, refreshTerminals, status]);

  useEffect(() => {
    if (!terminalSessions.length) {
      setActiveTerminalSessionId(null);
      return;
    }
    if (!activeTerminalSessionId) {
      setActiveTerminalSessionId(terminalSessions[0].session_id);
      return;
    }
    if (!terminalSessions.some((session) => session.session_id === activeTerminalSessionId)) {
      setActiveTerminalSessionId(terminalSessions[0].session_id);
    }
  }, [activeTerminalSessionId, terminalSessions]);

  const activeTerminalSession = useMemo(
    () => terminalSessions.find((session) => session.session_id === activeTerminalSessionId) ?? null,
    [activeTerminalSessionId, terminalSessions],
  );

  const canRefreshTerminals = hasTarget && status === 'connected' && !loadingTerminals;
  const canStartSession = hasTarget && status === 'connected';
  const wideTerminalLayout = width >= 1200;

  return (
    <AppShell
      section="terminals"
      hasTarget={hasTarget}
      loadingTarget={loadingTarget}
      error={error}
      statusBadge={statusBadge}
      renderDrawerActions={() => (
        <>
          <ActionButton
            disabled={!canStartSession}
            label="New Session"
            onPress={() => {
              void startTerminalSession().then((sessionId) => {
                if (sessionId) {
                  setActiveTerminalSessionId(sessionId);
                }
              });
            }}
            variant="primary"
            flex
          />
          <ActionButton
            disabled={!canRefreshTerminals}
            label={loadingTerminals ? 'Refreshing...' : 'Refresh'}
            onPress={() => {
              void refreshTerminals();
            }}
          />
        </>
      )}
      renderDrawerContent={({ closeDrawer }) => (
        <TerminalSessionList
          sessions={terminalSessions}
          tmuxSessions={tmuxSessions}
          tmuxSupported={tmuxSupported}
          tmuxError={tmuxError}
          loading={loadingTerminals}
          activeSessionId={activeTerminalSessionId}
          onSelectSession={(sessionId) => {
            setActiveTerminalSessionId(sessionId);
            closeDrawer();
          }}
          onAttachTmux={(sessionName) => {
            void attachTmuxSession(sessionName).then((sessionId) => {
              if (sessionId) setActiveTerminalSessionId(sessionId);
            });
            closeDrawer();
          }}
        />
      )}>
      <View style={styles.shell}>
        <View style={[styles.terminalFrame, wideTerminalLayout ? styles.terminalFrameWide : null]}>
          <MobileTerminalPane
            connected={status === 'connected'}
            onAttach={attachTerminalSession}
            onBinaryMessage={onTerminalBinary}
            onInput={sendTerminalInput}
            onResize={resizeTerminalSession}
            sessionId={activeTerminalSessionId}
            sessionCols={activeTerminalSession?.cols ?? null}
            sessionRows={activeTerminalSession?.rows ?? null}
            sessionStatus={activeTerminalSession?.status ?? null}
            shellLabel={activeTerminalSession?.name || activeTerminalSession?.shell || null}
          />
        </View>
      </View>
    </AppShell>
  );
}

const styles = StyleSheet.create({
  shell: {
    flex: 1,
    minHeight: 0,
  },
  terminalFrame: {
    flex: 1,
    minHeight: 0,
    width: '100%',
  },
  terminalFrameWide: {
    alignSelf: 'center',
    maxWidth: 1320,
  },
});
