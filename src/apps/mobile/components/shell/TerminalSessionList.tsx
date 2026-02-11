import { formatRelativeTime, type SessionInfo } from '@homie/shared';
import { memo } from 'react';
import { FlatList, Pressable, StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface TerminalSessionListProps {
  sessions: SessionInfo[];
  loading: boolean;
  activeSessionId: string | null;
  onSelect: (sessionId: string) => void;
}

export function TerminalSessionList({
  sessions,
  loading,
  activeSessionId,
  onSelect,
}: TerminalSessionListProps) {
  const { palette } = useAppTheme();

  if (loading && sessions.length === 0) {
    return (
      <View style={[styles.emptyCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
        <Text style={[styles.emptyTitle, { color: palette.text }]}>Loading terminals</Text>
        <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>
          Syncing running sessions...
        </Text>
      </View>
    );
  }

  if (!loading && sessions.length === 0) {
    return (
      <View style={[styles.emptyCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
        <Text style={[styles.emptyTitle, { color: palette.text }]}>No running terminals</Text>
        <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>
          Start a terminal session from desktop or web first.
        </Text>
      </View>
    );
  }

  return (
    <FlatList
      data={sessions}
      keyExtractor={(session) => session.session_id}
      renderItem={({ item }) => (
        <TerminalSessionRow
          session={item}
          selected={item.session_id === activeSessionId}
          palette={palette}
          onSelect={onSelect}
        />
      )}
      contentContainerStyle={styles.list}
      showsVerticalScrollIndicator={false}
      initialNumToRender={12}
      maxToRenderPerBatch={16}
      windowSize={8}
      removeClippedSubviews
      keyboardShouldPersistTaps="handled"
      getItemLayout={(_, index) => ({
        index,
        length: 90,
        offset: 90 * index,
      })}
    />
  );
}

interface TerminalSessionRowProps {
  session: SessionInfo;
  selected: boolean;
  palette: ReturnType<typeof useAppTheme>['palette'];
  onSelect: (sessionId: string) => void;
}

const TerminalSessionRow = memo(function TerminalSessionRow({
  session,
  selected,
  palette,
  onSelect,
}: TerminalSessionRowProps) {
  const numericStart = Number.parseInt(session.started_at, 10);
  const startedAt = Number.isFinite(numericStart)
    ? numericStart * 1000
    : new Date(session.started_at).getTime();
  const updatedLabel = Number.isFinite(startedAt) ? formatRelativeTime(startedAt) || 'now' : 'now';
  const sessionName = session.name || session.shell;
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ selected }}
      accessibilityLabel={`Terminal ${sessionName}`}
      accessibilityHint="Opens this terminal session view"
      onPress={() => onSelect(session.session_id)}
      style={({ pressed }) => [
        styles.sessionCard,
        {
          backgroundColor: selected ? palette.surface1 : palette.surface0,
          borderColor: selected ? palette.accent : palette.border,
          opacity: pressed ? 0.86 : 1,
        },
      ]}>
      <View style={styles.row}>
        <Text numberOfLines={1} style={[styles.shell, { color: palette.text }]}>
          {sessionName}
        </Text>
        <Text style={[styles.updatedAt, { color: palette.textSecondary }]}>{updatedLabel}</Text>
      </View>
      <Text numberOfLines={1} style={[styles.detail, { color: palette.textSecondary }]}>
        {session.cols}x{session.rows} • {session.status}
      </Text>
    </Pressable>
  );
});

const styles = StyleSheet.create({
  list: {
    gap: spacing.md,
    paddingHorizontal: spacing.sm,
    paddingVertical: spacing.sm,
  },
  sessionCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    minHeight: 78,
    padding: spacing.md,
    gap: spacing.xs,
  },
  row: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
  },
  shell: {
    ...typography.label,
    flex: 1,
    fontSize: 14,
  },
  updatedAt: {
    ...typography.data,
    fontSize: 12,
  },
  detail: {
    ...typography.body,
    fontSize: 13,
    fontWeight: '400',
  },
  emptyCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.xs,
  },
  emptyTitle: {
    ...typography.title,
    fontSize: 18,
  },
  emptyBody: {
    ...typography.body,
    fontWeight: '400',
  },
});
