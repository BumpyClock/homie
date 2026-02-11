// ABOUTME: Scrollable list of chat thread summary cards for the drawer sidebar.
// ABOUTME: Shows thread title, preview, relative time, running status with haptic feedback on interactions.

import { formatRelativeTime, type ChatThreadSummary } from '@homie/shared';
import * as Haptics from 'expo-haptics';
import { Platform, Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface ThreadListProps {
  threads: ChatThreadSummary[];
  activeChatId: string | null;
  loading: boolean;
  onSelect: (chatId: string) => void;
  onLongPressThread?: (thread: ChatThreadSummary) => void;
}

export function ThreadList({
  threads,
  activeChatId,
  loading,
  onSelect,
  onLongPressThread,
}: ThreadListProps) {
  const { palette } = useAppTheme();

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
    <ScrollView
      showsVerticalScrollIndicator={false}
      contentContainerStyle={styles.list}>
      {threads.map((thread) => {
        const selected = thread.chatId === activeChatId;
        const updatedLabel = formatRelativeTime(thread.lastActivityAt) || 'now';
        return (
          <Pressable
            key={thread.chatId}
            accessibilityRole="button"
            accessibilityState={{ selected }}
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
              <Text style={[styles.updatedAt, { color: palette.textSecondary }]}>
                {updatedLabel}
              </Text>
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
      })}
    </ScrollView>
  );
}

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
