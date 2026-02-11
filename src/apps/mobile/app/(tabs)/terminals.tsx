import { useEffect, useMemo, useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';

import { AppShell } from '@/components/shell/AppShell';
import { useMobileShellData } from '@/components/shell/MobileShellDataContext';
import { TerminalSessionList } from '@/components/shell/TerminalSessionList';
import { ActionButton } from '@/components/ui/ActionButton';
import { LabeledValueRow } from '@/components/ui/LabeledValueRow';
import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

export default function TerminalsTabScreen() {
  const { palette } = useAppTheme();
  const {
    loadingTarget,
    hasTarget,
    status,
    statusBadge,
    error,
    loadingTerminals,
    terminalSessions,
    refreshTerminals,
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

  return (
    <AppShell
      section="terminals"
      hasTarget={hasTarget}
      loadingTarget={loadingTarget}
      error={error}
      statusBadge={statusBadge}
      renderDrawerActions={() => (
        <ActionButton
          disabled={!canRefreshTerminals}
          label={loadingTerminals ? 'Refreshing...' : 'Refresh Sessions'}
          onPress={() => {
            void refreshTerminals();
          }}
          flex
        />
      )}
      renderDrawerContent={({ closeDrawer }) => (
        <TerminalSessionList
          sessions={terminalSessions}
          loading={loadingTerminals}
          activeSessionId={activeTerminalSessionId}
          onSelect={(sessionId) => {
            setActiveTerminalSessionId(sessionId);
            closeDrawer();
          }}
        />
      )}>
      <View style={[styles.sectionCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}> 
        <Text style={[styles.sectionTitle, { color: palette.text }]}>Terminal Session</Text>
        {activeTerminalSession ? (
          <>
            <LabeledValueRow label="Name" value={activeTerminalSession.name || activeTerminalSession.shell} />
            <LabeledValueRow
              label="Resolution"
              value={`${activeTerminalSession.cols} x ${activeTerminalSession.rows}`}
            />
            <LabeledValueRow label="Status" value={activeTerminalSession.status} last />
            <Text style={[styles.meta, { color: palette.textSecondary }]}>Terminal rendering ships in the next milestone.</Text>
          </>
        ) : (
          <Text style={[styles.meta, { color: palette.textSecondary }]}>Pick a terminal from the left list.</Text>
        )}
      </View>
    </AppShell>
  );
}

const styles = StyleSheet.create({
  sectionCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.md,
  },
  sectionTitle: {
    ...typography.title,
  },
  meta: {
    ...typography.body,
    fontWeight: '400',
    fontSize: 13,
  },
});
