import { type ChatItem } from '@homie/shared';
import { ScrollView, StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface TimelineThread {
  title: string;
  items: ChatItem[];
  running: boolean;
}

interface ChatTimelineProps {
  thread: TimelineThread | null;
  loading: boolean;
}

function bodyForItem(item: ChatItem): string {
  if (item.kind === 'reasoning') {
    const detail = [...(item.summary ?? []), ...(item.content ?? [])].join('\n');
    return detail || 'Reasoning update';
  }
  if (item.kind === 'command') {
    const command = item.command ? `$ ${item.command}` : '';
    const output = item.output ?? '';
    return [command, output].filter(Boolean).join('\n');
  }
  if (item.kind === 'file') {
    if (!item.changes || item.changes.length === 0) return 'File changes';
    return item.changes.map((change) => change.path).join('\n');
  }
  if (item.kind === 'approval') {
    if (item.command) return `Approval required: ${item.command}`;
    return 'Approval required';
  }
  if (item.kind === 'tool') {
    return item.text || 'Tool call';
  }
  return item.text || '';
}

function labelForItem(item: ChatItem): string {
  if (item.kind === 'user') return 'You';
  if (item.kind === 'assistant') return 'Assistant';
  if (item.kind === 'plan') return 'Plan';
  if (item.kind === 'reasoning') return 'Reasoning';
  if (item.kind === 'command') return 'Command';
  if (item.kind === 'file') return 'Files';
  if (item.kind === 'approval') return 'Approval';
  if (item.kind === 'diff') return 'Diff';
  if (item.kind === 'tool') return 'Tool';
  return 'System';
}

export function ChatTimeline({ thread, loading }: ChatTimelineProps) {
  const { palette } = useAppTheme();

  if (!thread) {
    return (
      <View style={[styles.emptyWrap, { backgroundColor: palette.surface, borderColor: palette.border }]}>
        <Text style={[styles.emptyTitle, { color: palette.text }]}>Pick a chat</Text>
        <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>
          Select a thread above to load messages.
        </Text>
      </View>
    );
  }

  return (
    <View style={[styles.container, { backgroundColor: palette.surface, borderColor: palette.border }]}>
      <View style={[styles.header, { borderBottomColor: palette.border }]}>
        <Text numberOfLines={1} style={[styles.title, { color: palette.text }]}>
          {thread.title}
        </Text>
        {thread.running ? <Text style={[styles.running, { color: palette.success }]}>Running</Text> : null}
      </View>
      {loading ? (
        <View style={styles.loadingWrap}>
          <Text style={[styles.loadingLabel, { color: palette.textSecondary }]}>Loading messages...</Text>
        </View>
      ) : (
        <ScrollView
          contentContainerStyle={styles.content}
          showsVerticalScrollIndicator={false}>
          {thread.items.length === 0 ? (
            <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>No messages yet.</Text>
          ) : (
            thread.items.map((item) => {
              const body = bodyForItem(item);
              if (!body.trim()) return null;
              const user = item.kind === 'user';
              return (
                <View
                  key={item.id}
                  style={[
                    styles.item,
                    user ? styles.userItem : styles.agentItem,
                    {
                      alignSelf: user ? 'flex-end' : 'stretch',
                      backgroundColor: user ? palette.accent : palette.surfaceAlt,
                      borderColor: user ? palette.accent : palette.border,
                    },
                  ]}>
                  <Text style={[styles.itemLabel, { color: user ? palette.surface : palette.textSecondary }]}>
                    {labelForItem(item)}
                  </Text>
                  <Text style={[styles.itemBody, { color: user ? palette.surface : palette.text }]}>
                    {body}
                  </Text>
                </View>
              );
            })
          )}
        </ScrollView>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    borderRadius: radius.lg,
    borderWidth: 1,
    overflow: 'hidden',
  },
  header: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
    gap: spacing.sm,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
    borderBottomWidth: 1,
  },
  title: {
    ...typography.title,
    flex: 1,
    fontSize: 18,
  },
  running: {
    ...typography.label,
    fontSize: 12,
  },
  content: {
    gap: spacing.sm,
    padding: spacing.md,
    paddingBottom: spacing.xl,
  },
  item: {
    borderRadius: radius.md,
    borderWidth: 1,
    gap: spacing.xs,
    maxWidth: '90%',
    padding: spacing.sm,
  },
  userItem: {
    marginLeft: spacing.xl,
  },
  agentItem: {
    marginRight: spacing.xl,
  },
  itemLabel: {
    ...typography.label,
    fontSize: 11,
    textTransform: 'uppercase',
  },
  itemBody: {
    ...typography.body,
    fontSize: 14,
    fontWeight: '400',
  },
  loadingWrap: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    padding: spacing.lg,
  },
  loadingLabel: {
    ...typography.body,
    fontWeight: '400',
  },
  emptyWrap: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.xs,
  },
  emptyTitle: {
    ...typography.title,
  },
  emptyBody: {
    ...typography.body,
    fontWeight: '400',
  },
});
