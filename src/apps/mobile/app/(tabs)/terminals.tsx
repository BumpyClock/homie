import { useEffect, useMemo, useState } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';

import { AppShell } from '@/components/shell/AppShell';
import { useMobileShellData } from '@/components/shell/MobileShellDataContext';
import { TerminalSessionList } from '@/components/shell/TerminalSessionList';
import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

type SettingRowProps = {
  label: string;
  value: string;
};

function SettingRow({ label, value }: SettingRowProps) {
  const { palette } = useAppTheme();

  return (
    <View style={[styles.settingRow, { borderColor: palette.border }]}> 
      <Text style={[styles.settingLabel, { color: palette.textSecondary }]}>{label}</Text>
      <Text style={[styles.settingValue, { color: palette.text }]}>{value}</Text>
    </View>
  );
}

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
        <Pressable
          accessibilityRole="button"
          disabled={!canRefreshTerminals}
          onPress={() => {
            void refreshTerminals();
          }}
          style={({ pressed }) => [
            styles.actionButton,
            {
              backgroundColor: palette.surface1,
              borderColor: palette.border,
              opacity: pressed ? 0.86 : canRefreshTerminals ? 1 : 0.58,
            },
          ]}>
          <Text style={[styles.actionLabel, { color: palette.text }]}> 
            {loadingTerminals ? 'Refreshing...' : 'Refresh Sessions'}
          </Text>
        </Pressable>
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
            <SettingRow label="Name" value={activeTerminalSession.name || activeTerminalSession.shell} />
            <SettingRow
              label="Resolution"
              value={`${activeTerminalSession.cols} x ${activeTerminalSession.rows}`}
            />
            <SettingRow label="Status" value={activeTerminalSession.status} />
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
  actionButton: {
    borderRadius: radius.md,
    borderWidth: 1,
    minHeight: 44,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: spacing.md,
    flex: 1,
  },
  actionLabel: {
    ...typography.label,
    fontSize: 13,
  },
  sectionCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.md,
  },
  sectionTitle: {
    ...typography.title,
  },
  settingRow: {
    borderBottomWidth: 1,
    paddingVertical: spacing.sm,
    gap: spacing.xs,
  },
  settingLabel: {
    ...typography.label,
    textTransform: 'uppercase',
  },
  settingValue: {
    ...typography.data,
  },
  meta: {
    ...typography.body,
    fontWeight: '400',
    fontSize: 13,
  },
});
