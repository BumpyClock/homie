// ABOUTME: Scrollable list of chat thread summary cards for the drawer sidebar.
// ABOUTME: Shows thread title, preview, relative time, running status with haptic feedback on interactions.

import { formatRelativeTime, type ChatThreadSummary } from '@homie/shared';
import * as Haptics from 'expo-haptics';
import { TriangleAlert } from 'lucide-react-native';
import { memo, useCallback } from 'react';
import { type ListRenderItemInfo, FlatList, Platform, Pressable, StyleSheet, Text, View } from 'react-native';
import Animated, { FadeInUp } from 'react-native-reanimated';

import { useAppTheme } from '@/hooks/useAppTheme';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion } from '@/theme/motion';
import { radius, spacing, typography } from '@/theme/tokens';

interface ThreadListProps {
  threads: ChatThreadSummary[];
  activeChatId: string | null;
  loading: boolean;
  onSelect: (chatId: string) => void;
  onLongPressThread?: (thread: ChatThreadSummary) => void;
  getApprovalCount?: (chatId: string) => number;
}

export function ThreadList({
  threads,
  activeChatId,
  loading,
  onSelect,
  onLongPressThread,
  getApprovalCount,
}: ThreadListProps) {
  const { palette } = useAppTheme();
  const reducedMotion = useReducedMotion();

  const renderItem = useCallback(
    ({ item: thread, index }: ListRenderItemInfo<ChatThreadSummary>) => {
      const staggerDelay = reducedMotion
        ? 0
        : index < 10
          ? index * motion.stagger.tight
          : 10 * motion.stagger.tight;
      const entering = reducedMotion
        ? undefined
        : FadeInUp.delay(staggerDelay).duration(motion.duration.fast);

      return (
        <Animated.View entering={entering}>
          <ThreadRow
            thread={thread}
            selected={thread.chatId === activeChatId}
            palette={palette}
            approvalCount={getApprovalCount?.(thread.chatId) ?? 0}
            onLongPressThread={onLongPressThread}
            onSelect={onSelect}
          />
        </Animated.View>
      );
    },
    [activeChatId, getApprovalCount, onLongPressThread, onSelect, palette, reducedMotion],
  );

  if (!loading && threads.length === 0) {
    return (
      <View
        style={[
          styles.emptyCard,
          {
            backgroundColor: palette.surface0,
            borderColor: palette.border,
          },
        ]}>
        <Text style={[styles.emptyTitle, { color: palette.text }]}>No chats yet</Text>
        <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>
          Start a new chat to connect to the gateway.
        </Text>
      </View>
    );
  }

  if (loading && threads.length === 0) {
    return (
      <View
        style={[
          styles.emptyCard,
          {
            backgroundColor: palette.surface0,
            borderColor: palette.border,
          },
        ]}>
        <Text style={[styles.emptyTitle, { color: palette.text }]}>Loading chats</Text>
        <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>
          Syncing conversation history from the gateway.
        </Text>
      </View>
    );
  }

  return (
    <FlatList
      accessibilityLabel="Conversation list"
      accessibilityRole="list"
      data={threads}
      keyExtractor={(thread) => thread.chatId}
      renderItem={renderItem}
      contentContainerStyle={styles.list}
      showsVerticalScrollIndicator={false}
      initialNumToRender={10}
      maxToRenderPerBatch={12}
      windowSize={8}
      removeClippedSubviews
      keyboardShouldPersistTaps="handled"
      getItemLayout={(_, index) => ({
        index,
        length: 100,
        offset: 100 * index,
      })}
    />
  );
}

interface ThreadRowProps {
  thread: ChatThreadSummary;
  selected: boolean;
  palette: ReturnType<typeof useAppTheme>['palette'];
  approvalCount: number;
  onSelect: (chatId: string) => void;
  onLongPressThread?: (thread: ChatThreadSummary) => void;
}

const ThreadRow = memo(function ThreadRow({
  thread,
  selected,
  palette,
  approvalCount,
  onSelect,
  onLongPressThread,
}: ThreadRowProps) {
  const updatedLabel = formatRelativeTime(thread.lastActivityAt) || 'now';
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ selected }}
      accessibilityLabel={`Conversation ${thread.title}`}
      accessibilityHint="Opens this conversation"
      delayLongPress={300}
      onLongPress={() => {
        if (Platform.OS !== 'web') {
          Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium);
        }
        onLongPressThread?.(thread);
      }}
      onPress={() => {
        if (Platform.OS !== 'web') {
          Haptics.selectionAsync();
        }
        onSelect(thread.chatId);
      }}
      style={({ pressed }) => [
        styles.threadCard,
        {
          backgroundColor: selected ? palette.surface1 : palette.surface0,
          borderColor: selected ? palette.accent : palette.border,
          opacity: pressed ? 0.88 : 1,
        },
      ]}>
      <View style={styles.headerRow}>
        <Text numberOfLines={1} style={[styles.title, { color: palette.text }]}>
          {thread.title}
        </Text>
        {approvalCount > 0 ? (
          <View
            accessible
            accessibilityLabel={`${approvalCount} pending ${approvalCount === 1 ? 'approval' : 'approvals'}`}
            style={[
              styles.approvalPill,
              {
                backgroundColor: palette.warningDim,
                borderColor: palette.warning,
              },
            ]}>
            <TriangleAlert size={13} color={palette.warning} />
            <Text style={[styles.approvalCount, { color: palette.warning }]}>
              {approvalCount}
            </Text>
          </View>
        ) : null}
        <Text style={[styles.updatedAt, { color: palette.textSecondary }]}>{updatedLabel}</Text>
      </View>
      <Text numberOfLines={2} style={[styles.preview, { color: palette.textSecondary }]}>
        {thread.preview || 'Tap to load conversation'}
      </Text>
      {thread.running ? (
        <View style={styles.runningRow}>
          <View style={[styles.runningDot, { backgroundColor: palette.success }]} />
          <Text style={[styles.runningText, { color: palette.success }]}>Running</Text>
        </View>
      ) : null}
    </Pressable>
  );
});

const styles = StyleSheet.create({
  list: {
    gap: spacing.md,
    paddingVertical: spacing.sm,
    paddingHorizontal: spacing.sm,
  },
  threadCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.md,
    gap: spacing.xs,
    minHeight: 88,
  },
  headerRow: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
  },
  title: {
    ...typography.label,
    flex: 1,
    fontSize: 14,
  },
  updatedAt: {
    ...typography.data,
    fontSize: 12,
  },
  preview: {
    ...typography.body,
    fontSize: 14,
    lineHeight: 20,
    fontWeight: '400',
    minHeight: 40,
  },
  approvalPill: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacing.xs,
    borderWidth: 1,
    borderRadius: radius.pill,
    paddingHorizontal: spacing.sm,
    paddingVertical: 2,
  },
  approvalCount: {
    fontSize: 11,
    fontWeight: '600',
  },
  runningRow: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.xs,
  },
  runningDot: {
    width: 8,
    height: 8,
    borderRadius: radius.pill,
  },
  runningText: {
    ...typography.label,
    fontSize: 12,
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
