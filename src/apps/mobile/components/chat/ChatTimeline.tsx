import { type ChatItem } from '@homie/shared';
import { useCallback, useEffect, useState } from 'react';
import { Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import Markdown from 'react-native-marked';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface TimelineThread {
  chatId: string;
  threadId: string;
  title: string;
  items: ChatItem[];
  running: boolean;
}

interface ChatTimelineProps {
  thread: TimelineThread | null;
  loading: boolean;
  onApprovalDecision?: (
    requestId: number | string,
    decision: 'accept' | 'decline' | 'accept_for_session',
  ) => Promise<void> | void;
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

function approvalStatusLabel(status: string): string {
  if (status === 'accept' || status === 'accept_for_session') return 'Accepted';
  if (status === 'decline' || status === 'cancel') return 'Declined';
  return 'Pending';
}

export function ChatTimeline({ thread, loading, onApprovalDecision }: ChatTimelineProps) {
  const { palette } = useAppTheme();
  const [respondingItemId, setRespondingItemId] = useState<string | null>(null);
  const [localApprovalStatus, setLocalApprovalStatus] = useState<Record<string, string>>({});

  useEffect(() => {
    setRespondingItemId(null);
    setLocalApprovalStatus({});
  }, [thread?.chatId, thread?.threadId]);

  const handleDecision = useCallback(
    async (item: ChatItem, decision: 'accept' | 'decline' | 'accept_for_session') => {
      if (!onApprovalDecision || item.requestId === undefined) return;
      setRespondingItemId(item.id);
      try {
        await onApprovalDecision(item.requestId, decision);
        setLocalApprovalStatus((current) => ({
          ...current,
          [item.id]: decision,
        }));
      } finally {
        setRespondingItemId((current) => (current === item.id ? null : current));
      }
    },
    [onApprovalDecision],
  );

  if (!thread) {
    return (
      <View style={[styles.emptyWrap, { backgroundColor: palette.surface, borderColor: palette.border }]}>
        <Text style={[styles.emptyTitle, { color: palette.text }]}>Pick a chat</Text>
        <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>
          Open chats to load a conversation.
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
              if (item.kind === 'approval') {
                const status = localApprovalStatus[item.id] ?? item.status ?? 'pending';
                const resolved = status !== 'pending';
                const responding = respondingItemId === item.id;
                const canRespond =
                  !resolved &&
                  !responding &&
                  item.requestId !== undefined &&
                  onApprovalDecision !== undefined;
                return (
                  <View
                    key={item.id}
                    style={[
                      styles.approvalCard,
                      {
                        backgroundColor: palette.surfaceAlt,
                        borderColor: palette.warning,
                      },
                    ]}>
                    <View style={styles.approvalHeader}>
                      <Text style={[styles.approvalTitle, { color: palette.warning }]}>
                        Approval Required
                      </Text>
                      <Text style={[styles.approvalStatus, { color: palette.textSecondary }]}>
                        {approvalStatusLabel(status)}
                      </Text>
                    </View>
                    {item.reason ? (
                      <Text style={[styles.itemBody, { color: palette.text }]}>{item.reason}</Text>
                    ) : null}
                    {item.command ? (
                      <View
                        style={[
                          styles.commandCard,
                          {
                            backgroundColor: palette.surface,
                            borderColor: palette.border,
                          },
                        ]}>
                        <Text style={[styles.commandText, { color: palette.text }]}>
                          {`$ ${item.command}`}
                        </Text>
                      </View>
                    ) : null}
                    {!resolved ? (
                      <View style={styles.approvalActions}>
                        <Pressable
                          accessibilityRole="button"
                          accessibilityLabel="Accept approval request"
                          disabled={!canRespond}
                          onPress={() => {
                            void handleDecision(item, 'accept');
                          }}
                          style={({ pressed }) => [
                            styles.approvalButton,
                            {
                              backgroundColor: palette.success,
                              borderColor: palette.success,
                              opacity: pressed ? 0.86 : canRespond ? 1 : 0.58,
                            },
                          ]}>
                          <Text style={[styles.approvalLabel, { color: palette.surface }]}>
                            {responding ? 'Sending...' : 'Accept'}
                          </Text>
                        </Pressable>
                        <Pressable
                          accessibilityRole="button"
                          accessibilityLabel="Accept approval request for this chat session"
                          disabled={!canRespond}
                          onPress={() => {
                            void handleDecision(item, 'accept_for_session');
                          }}
                          style={({ pressed }) => [
                            styles.approvalButton,
                            {
                              backgroundColor: palette.accent,
                              borderColor: palette.accent,
                              opacity: pressed ? 0.86 : canRespond ? 1 : 0.58,
                            },
                          ]}>
                          <Text style={[styles.approvalLabel, { color: palette.surface }]}>
                            Always
                          </Text>
                        </Pressable>
                        <Pressable
                          accessibilityRole="button"
                          accessibilityLabel="Decline approval request"
                          disabled={!canRespond}
                          onPress={() => {
                            void handleDecision(item, 'decline');
                          }}
                          style={({ pressed }) => [
                            styles.approvalButton,
                            {
                              backgroundColor: palette.surface,
                              borderColor: palette.danger,
                              opacity: pressed ? 0.86 : canRespond ? 1 : 0.58,
                            },
                          ]}>
                          <Text style={[styles.approvalLabel, { color: palette.danger }]}>
                            Decline
                          </Text>
                        </Pressable>
                      </View>
                    ) : null}
                  </View>
                );
              }

              const body = bodyForItem(item);
              if (!body.trim()) return null;
              const user = item.kind === 'user';
              const renderMarkdown = !user && (item.kind === 'assistant' || item.kind === 'reasoning' || item.kind === 'tool' || item.kind === 'plan' || item.kind === 'diff');
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
                  {renderMarkdown ? (
                    <Markdown
                      value={body}
                      styles={{
                        text: {
                          ...styles.itemBody,
                          color: palette.text,
                        },
                        paragraph: {
                          marginBottom: spacing.xs,
                        },
                        code: {
                          backgroundColor: palette.surface,
                          borderColor: palette.border,
                          borderRadius: radius.sm,
                          borderWidth: 1,
                          padding: spacing.xs,
                        },
                        codespan: {
                          ...styles.commandText,
                          backgroundColor: palette.surface,
                          color: palette.text,
                        },
                        link: {
                          color: palette.accent,
                          textDecorationLine: 'underline',
                        },
                        blockquote: {
                          borderLeftColor: palette.border,
                          borderLeftWidth: 3,
                          paddingLeft: spacing.sm,
                        },
                        list: {
                          marginBottom: spacing.xs,
                        },
                        li: {
                          ...styles.itemBody,
                          color: palette.text,
                        },
                        h1: {
                          ...styles.itemBody,
                          color: palette.text,
                          fontSize: 18,
                          fontWeight: '700',
                        },
                        h2: {
                          ...styles.itemBody,
                          color: palette.text,
                          fontSize: 16,
                          fontWeight: '700',
                        },
                        h3: {
                          ...styles.itemBody,
                          color: palette.text,
                          fontSize: 15,
                          fontWeight: '600',
                        },
                      }}
                      flatListProps={{
                        scrollEnabled: false,
                      }}
                    />
                  ) : (
                    <Text style={[styles.itemBody, { color: user ? palette.surface : palette.text }]}>
                      {body}
                    </Text>
                  )}
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
  approvalCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    gap: spacing.sm,
    marginRight: spacing.xl,
    maxWidth: '94%',
    padding: spacing.md,
  },
  approvalHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
    gap: spacing.sm,
  },
  approvalTitle: {
    ...typography.label,
    textTransform: 'uppercase',
    fontSize: 11,
  },
  approvalStatus: {
    ...typography.data,
    fontSize: 12,
  },
  commandCard: {
    borderRadius: radius.sm,
    borderWidth: 1,
    padding: spacing.sm,
  },
  commandText: {
    ...typography.data,
    fontSize: 12,
  },
  approvalActions: {
    flexDirection: 'row',
    gap: spacing.sm,
  },
  approvalButton: {
    alignItems: 'center',
    borderRadius: radius.md,
    borderWidth: 1,
    flex: 1,
    justifyContent: 'center',
    minHeight: 44,
    paddingHorizontal: spacing.sm,
  },
  approvalLabel: {
    ...typography.label,
    fontSize: 13,
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
