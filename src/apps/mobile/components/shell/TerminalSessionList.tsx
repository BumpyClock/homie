import {
  formatRelativeTime,
  partitionTerminalSessions,
  sessionDisplayName,
  tmuxSessionName,
  type SessionInfo,
  type TmuxSessionInfo,
} from '@homie/shared';
import { memo, useCallback, useMemo, useRef } from 'react';
import { Pressable, SectionList, type SectionListRenderItemInfo, StyleSheet, Text, View } from 'react-native';
import Animated, { FadeInUp } from 'react-native-reanimated';

import { useAppTheme } from '@/hooks/useAppTheme';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion } from '@/theme/motion';
import { radius, spacing, typography } from '@/theme/tokens';

interface TerminalSessionListProps {
  sessions: SessionInfo[];
  tmuxSessions: TmuxSessionInfo[];
  tmuxSupported: boolean;
  tmuxError: string | null;
  loading: boolean;
  activeSessionId: string | null;
  onSelectSession: (sessionId: string) => void;
  onAttachTmux: (sessionName: string) => void;
}

type DrawerRow =
  | { id: string; type: 'tmux'; tmux: TmuxSessionInfo }
  | { id: string; type: 'session'; session: SessionInfo };

interface DrawerSection {
  key: string;
  title: string;
  emptyLabel: string;
  data: DrawerRow[];
}

export function TerminalSessionList({
  sessions,
  tmuxSessions,
  tmuxSupported,
  tmuxError,
  loading,
  activeSessionId,
  onSelectSession,
  onAttachTmux,
}: TerminalSessionListProps) {
  const { palette } = useAppTheme();
  const reducedMotion = useReducedMotion();
  const rowIndexRef = useRef(0);
  const { active, history } = useMemo(() => partitionTerminalSessions(sessions), [sessions]);
  const runningTmux = useMemo(
    () =>
      tmuxSupported
        ? [...tmuxSessions].sort((a, b) => {
            if (a.attached === b.attached) return a.name.localeCompare(b.name);
            return a.attached ? -1 : 1;
          })
        : [],
    [tmuxSessions, tmuxSupported],
  );

  if (loading && sessions.length === 0) {
    return (
      <View style={[styles.emptyCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
        <Text style={[styles.emptyTitle, { color: palette.text }]}>Loading terminals</Text>
        <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>Syncing active sessions and history...</Text>
      </View>
    );
  }

  const sections: DrawerSection[] = [
    {
      key: 'running-tmux',
      title: 'Running tmux',
      emptyLabel: tmuxSupported ? 'No running tmux sessions' : 'tmux not available',
      data: runningTmux.map((tmux) => ({
        id: `tmux:${tmux.name}`,
        type: 'tmux',
        tmux,
      })),
    },
    {
      key: 'active-sessions',
      title: 'Active sessions',
      emptyLabel: 'No active terminal sessions',
      data: active.map((session) => ({
        id: session.session_id,
        type: 'session',
        session,
      })),
    },
    {
      key: 'history',
      title: 'History',
      emptyLabel: 'No previous sessions',
      data: history.map((session) => ({
        id: session.session_id,
        type: 'session',
        session,
      })),
    },
  ];

  rowIndexRef.current = 0;

  const renderItem = ({ item }: SectionListRenderItemInfo<DrawerRow, DrawerSection>) => {
    const globalIndex = rowIndexRef.current++;
    const staggerDelay = reducedMotion
      ? 0
      : globalIndex < 10
        ? globalIndex * motion.stagger.tight
        : 10 * motion.stagger.tight;
    const entering = reducedMotion
      ? undefined
      : FadeInUp.delay(staggerDelay).duration(motion.duration.fast);

    if (item.type === 'tmux') {
      return (
        <Animated.View entering={entering}>
          <TmuxRow
            tmux={item.tmux}
            onAttach={onAttachTmux}
            palette={palette}
          />
        </Animated.View>
      );
    }
    return (
      <Animated.View entering={entering}>
        <TerminalSessionRow
          session={item.session}
          selected={item.session.session_id === activeSessionId}
          palette={palette}
          onSelect={onSelectSession}
        />
      </Animated.View>
    );
  };

  return (
    <SectionList
      accessibilityLabel="Terminal sessions list"
      accessibilityRole="list"
      sections={sections}
      keyExtractor={(item) => item.id}
      stickySectionHeadersEnabled={false}
      showsVerticalScrollIndicator={false}
      contentContainerStyle={styles.listContent}
      keyboardShouldPersistTaps="handled"
      renderSectionHeader={({ section }) => (
        <Text
          accessibilityRole="header"
          style={[styles.sectionTitle, { color: palette.textSecondary }]}
        >
          {section.title}
        </Text>
      )}
      renderSectionFooter={({ section }) => {
        if (section.data.length > 0) return <View style={styles.sectionGap} />;
        return (
          <View style={[styles.emptySectionCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
            <Text style={[styles.emptySectionLabel, { color: palette.textSecondary }]}>{section.emptyLabel}</Text>
          </View>
        );
      }}
      renderItem={renderItem}
      ListHeaderComponent={
        tmuxError ? (
          <View
            accessible
            accessibilityRole="alert"
            style={[styles.errorCard, { backgroundColor: palette.dangerDim, borderColor: palette.danger }]}
          >
            <Text style={[styles.errorLabel, { color: palette.danger }]}>{tmuxError}</Text>
          </View>
        ) : null
      }
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
  const title = sessionDisplayName(session);
  const isTmuxBacked = Boolean(tmuxSessionName(session.shell));
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ selected }}
      accessibilityLabel={`Session ${title}`}
      accessibilityHint="Opens this terminal session"
      onPress={() => onSelect(session.session_id)}
      style={({ pressed }) => [
        styles.rowCard,
        {
          backgroundColor: selected ? palette.surface1 : palette.surface0,
          borderColor: selected ? palette.accent : palette.border,
          opacity: pressed ? 0.86 : 1,
        },
      ]}>
      <View style={styles.rowHeader}>
        <Text numberOfLines={1} style={[styles.rowTitle, { color: palette.text }]}>
          {title}
        </Text>
        <Text style={[styles.rowMeta, { color: palette.textSecondary }]}>{updatedLabel}</Text>
      </View>
      <Text numberOfLines={1} style={[styles.rowDetail, { color: palette.textSecondary }]}>
        {`${session.cols}x${session.rows} • ${session.status}`}
      </Text>
      <Text numberOfLines={1} style={[styles.rowHint, { color: palette.textTertiary }]}>
        {isTmuxBacked ? 'tmux session' : session.shell}
      </Text>
    </Pressable>
  );
});

interface TmuxRowProps {
  tmux: TmuxSessionInfo;
  palette: ReturnType<typeof useAppTheme>['palette'];
  onAttach: (sessionName: string) => void;
}

const TmuxRow = memo(function TmuxRow({ tmux, palette, onAttach }: TmuxRowProps) {
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityLabel={`Attach tmux ${tmux.name}`}
      accessibilityHint={tmux.attached ? 'Opens the attached tmux session' : 'Attaches and opens this tmux session'}
      onPress={() => onAttach(tmux.name)}
      style={({ pressed }) => [
        styles.rowCard,
        {
          backgroundColor: palette.surface0,
          borderColor: palette.border,
          opacity: pressed ? 0.86 : 1,
        },
      ]}>
      <View style={styles.rowHeader}>
        <Text numberOfLines={1} style={[styles.rowTitle, { color: palette.text }]}>
          {tmux.name}
        </Text>
        <Text style={[styles.rowMeta, { color: palette.textSecondary }]}>{`${tmux.windows} windows`}</Text>
      </View>
      <Text numberOfLines={1} style={[styles.rowDetail, { color: palette.textSecondary }]}>
        {tmux.attached ? 'attached' : 'detached'}
      </Text>
      <Text numberOfLines={1} style={[styles.rowHint, { color: palette.textTertiary }]}>
        {tmux.attached ? 'Tap to open' : 'Tap to attach'}
      </Text>
    </Pressable>
  );
});

const styles = StyleSheet.create({
  listContent: {
    gap: spacing.sm,
    paddingHorizontal: spacing.sm,
    paddingTop: spacing.xs,
    paddingBottom: spacing.md,
  },
  sectionTitle: {
    ...typography.overline,
    marginBottom: spacing.xs,
    marginTop: spacing.sm,
    paddingHorizontal: spacing.xs,
    textTransform: 'uppercase',
  },
  sectionGap: {
    height: spacing.sm,
  },
  rowCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    gap: spacing.xs,
    minHeight: 76,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
  },
  rowHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
  },
  rowTitle: {
    ...typography.label,
    flex: 1,
    fontSize: 14,
  },
  rowMeta: {
    ...typography.monoSmall,
    fontSize: 11,
  },
  rowDetail: {
    ...typography.body,
    fontSize: 12,
    fontWeight: '500',
  },
  rowHint: {
    ...typography.caption,
    fontSize: 11,
  },
  emptyCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.xs,
  },
  emptyTitle: {
    ...typography.title,
    fontSize: 17,
  },
  emptyBody: {
    ...typography.body,
    fontSize: 13,
  },
  emptySectionCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
    marginBottom: spacing.sm,
  },
  emptySectionLabel: {
    ...typography.caption,
    fontWeight: '500',
  },
  errorCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    marginBottom: spacing.sm,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
  },
  errorLabel: {
    ...typography.caption,
    fontWeight: '600',
  },
});
