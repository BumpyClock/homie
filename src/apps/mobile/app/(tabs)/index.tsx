import { Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';

import { ScreenSurface } from '@/components/ui/ScreenSurface';
import { StatusPill } from '@/components/ui/StatusPill';
import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

type ThreadPreview = {
  id: string;
  title: string;
  preview: string;
  updatedAt: string;
  running: boolean;
};

const mockThreads: ThreadPreview[] = [
  {
    id: 'chat-1',
    title: 'Fix ws reconnect issue',
    preview: 'I checked reconnection logs and found two duplicate subscriptions after resume.',
    updatedAt: '2m ago',
    running: true,
  },
  {
    id: 'chat-2',
    title: 'Add web search provider',
    preview: 'SearXNG provider works with format=json and self-hosted base URL settings.',
    updatedAt: '34m ago',
    running: false,
  },
  {
    id: 'chat-3',
    title: 'Terminal snapshot cadence',
    preview: 'Preview refresh set to 30s with immediate populate on list load.',
    updatedAt: 'Yesterday',
    running: false,
  },
];

export default function ChatTabScreen() {
  const { palette } = useAppTheme();

  return (
    <ScreenSurface>
      <View style={[styles.container, { backgroundColor: palette.background }]}> 
        <View style={styles.headerRow}>
          <View>
            <Text style={[styles.eyebrow, { color: palette.textSecondary }]}>Gateway</Text>
            <Text style={[styles.title, { color: palette.text }]}>Chat</Text>
          </View>
          <StatusPill label="Connected" tone="success" />
        </View>

        <Pressable
          accessibilityRole="button"
          style={({ pressed }) => [
            styles.newChatButton,
            {
              backgroundColor: palette.accent,
              opacity: pressed ? 0.86 : 1,
            },
          ]}>
          <Text style={[styles.newChatLabel, { color: palette.surface }]}>New Chat</Text>
        </Pressable>

        <ScrollView contentContainerStyle={styles.threadList} showsVerticalScrollIndicator={false}>
          {mockThreads.map((thread) => (
            <Pressable
              key={thread.id}
              accessibilityRole="button"
              style={({ pressed }) => [
                styles.threadCard,
                {
                  backgroundColor: palette.surface,
                  borderColor: palette.border,
                  opacity: pressed ? 0.93 : 1,
                },
              ]}>
              <View style={styles.threadHeader}>
                <Text numberOfLines={1} style={[styles.threadTitle, { color: palette.text }]}>
                  {thread.title}
                </Text>
                <Text style={[styles.threadTime, { color: palette.textSecondary }]}>{thread.updatedAt}</Text>
              </View>
              <Text numberOfLines={2} style={[styles.threadPreview, { color: palette.textSecondary }]}>
                {thread.preview}
              </Text>
              {thread.running ? (
                <View style={styles.runningRow}>
                  <View style={[styles.runningDot, { backgroundColor: palette.success }]} />
                  <Text style={[styles.runningText, { color: palette.success }]}>Turn running</Text>
                </View>
              ) : null}
            </Pressable>
          ))}
        </ScrollView>
      </View>
    </ScreenSurface>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    paddingTop: spacing.xxl,
    paddingHorizontal: spacing.lg,
    gap: spacing.lg,
  },
  headerRow: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
  },
  eyebrow: {
    ...typography.label,
    textTransform: 'uppercase',
  },
  title: {
    ...typography.display,
  },
  newChatButton: {
    borderRadius: radius.md,
    minHeight: 52,
    alignItems: 'center',
    justifyContent: 'center',
  },
  newChatLabel: {
    ...typography.body,
    fontWeight: '700',
  },
  threadList: {
    gap: spacing.md,
    paddingBottom: spacing.xxl,
  },
  threadCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.sm,
  },
  threadHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
    gap: spacing.sm,
  },
  threadTitle: {
    ...typography.title,
    flex: 1,
    fontSize: 17,
  },
  threadTime: {
    ...typography.data,
  },
  threadPreview: {
    ...typography.body,
    fontWeight: '400',
  },
  runningRow: {
    marginTop: spacing.xs,
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
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
});
