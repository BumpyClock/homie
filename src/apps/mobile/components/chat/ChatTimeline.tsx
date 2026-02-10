import { groupChatItemsByTurn, type ChatItem } from '@homie/shared';
import { Feather } from '@expo/vector-icons';
import * as Clipboard from 'expo-clipboard';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Alert,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  View,
  type NativeScrollEvent,
  type NativeSyntheticEvent,
} from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';
import { ChatMarkdown } from './ChatMarkdown';
import { ChatTurnActivity } from './ChatTurnActivity';

interface TimelineThread {
  chatId: string;
  threadId: string;
  title: string;
  items: ChatItem[];
  running: boolean;
  activeTurnId?: string;
}

interface ChatTimelineProps {
  thread: TimelineThread | null;
  loading: boolean;
  onApprovalDecision?: (
    requestId: number | string,
    decision: 'accept' | 'decline' | 'accept_for_session',
  ) => Promise<void> | void;
}

type ScrollPinState = 'pinned' | 'detached';

interface ScrollMetrics {
  contentHeight: number;
  viewportHeight: number;
  offsetY: number;
}

const PIN_DISTANCE_PX = 88;
const UNPIN_DISTANCE_PX = 132;

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

function buildTailRevision(items: ChatItem[]): string {
  if (items.length === 0) return '0';
  const tail = items[items.length - 1];
  const approvalSuffix =
    tail.kind === 'approval' ? `:${tail.status ?? 'pending'}:${tail.requestId ?? ''}` : '';
  return `${items.length}:${tail.id}:${tail.kind}:${bodyForItem(tail).length}${approvalSuffix}`;
}

export function ChatTimeline({ thread, loading, onApprovalDecision }: ChatTimelineProps) {
  const { palette } = useAppTheme();
  const [respondingItemId, setRespondingItemId] = useState<string | null>(null);
  const [localApprovalStatus, setLocalApprovalStatus] = useState<Record<string, string>>({});
  const [copiedItemId, setCopiedItemId] = useState<string | null>(null);
  const [activeActionItemId, setActiveActionItemId] = useState<string | null>(null);
  const [showJumpToLatest, setShowJumpToLatest] = useState(false);
  const copyResetTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const scrollViewRef = useRef<ScrollView | null>(null);
  const scrollPinStateRef = useRef<ScrollPinState>('pinned');
  const scrollMetricsRef = useRef<ScrollMetrics>({
    contentHeight: 0,
    viewportHeight: 0,
    offsetY: 0,
  });
  const tailRevisionRef = useRef('0');

  const setPinState = useCallback((next: ScrollPinState) => {
    if (scrollPinStateRef.current === next) return;
    scrollPinStateRef.current = next;
    setShowJumpToLatest(next === 'detached');
  }, []);

  const snapToLatest = useCallback((animated: boolean) => {
    requestAnimationFrame(() => {
      scrollViewRef.current?.scrollToEnd({ animated });
    });
  }, []);

  const reconcilePinState = useCallback(() => {
    const { contentHeight, viewportHeight, offsetY } = scrollMetricsRef.current;
    const distanceFromBottom = Math.max(contentHeight - viewportHeight - offsetY, 0);
    if (scrollPinStateRef.current === 'pinned') {
      if (distanceFromBottom > UNPIN_DISTANCE_PX) setPinState('detached');
      return;
    }
    if (distanceFromBottom <= PIN_DISTANCE_PX) setPinState('pinned');
  }, [setPinState]);

  const updateScrollMetrics = useCallback(
    (next: Partial<ScrollMetrics>) => {
      const metrics = scrollMetricsRef.current;
      if (next.contentHeight !== undefined) metrics.contentHeight = next.contentHeight;
      if (next.viewportHeight !== undefined) metrics.viewportHeight = next.viewportHeight;
      if (next.offsetY !== undefined) metrics.offsetY = next.offsetY;
      reconcilePinState();
    },
    [reconcilePinState],
  );

  const handleScroll = useCallback(
    (event: NativeSyntheticEvent<NativeScrollEvent>) => {
      const { contentOffset, contentSize, layoutMeasurement } = event.nativeEvent;
      updateScrollMetrics({
        contentHeight: contentSize.height,
        viewportHeight: layoutMeasurement.height,
        offsetY: contentOffset.y,
      });
    },
    [updateScrollMetrics],
  );

  const jumpToLatest = useCallback(() => {
    setPinState('pinned');
    snapToLatest(true);
  }, [setPinState, snapToLatest]);

  useEffect(() => {
    setRespondingItemId(null);
    setLocalApprovalStatus({});
    setCopiedItemId(null);
    setActiveActionItemId(null);
    setShowJumpToLatest(false);
    scrollPinStateRef.current = 'pinned';
    scrollMetricsRef.current = {
      contentHeight: 0,
      viewportHeight: 0,
      offsetY: 0,
    };
    tailRevisionRef.current = '0';
    snapToLatest(false);
    if (copyResetTimeoutRef.current) {
      clearTimeout(copyResetTimeoutRef.current);
      copyResetTimeoutRef.current = null;
    }
  }, [snapToLatest, thread?.chatId, thread?.threadId]);

  const tailRevision = thread ? buildTailRevision(thread.items) : '0';
  const itemCount = thread?.items.length ?? 0;
  const turnGroups = useMemo(
    () => (thread ? groupChatItemsByTurn(thread.items) : []),
    [thread?.items],
  );

  useEffect(() => {
    if (!thread) {
      tailRevisionRef.current = '0';
      return;
    }
    if (tailRevisionRef.current === tailRevision) return;
    tailRevisionRef.current = tailRevision;
    if (scrollPinStateRef.current === 'pinned') {
      snapToLatest(false);
      return;
    }
    if (itemCount > 0) setShowJumpToLatest(true);
  }, [itemCount, snapToLatest, tailRevision, thread?.chatId, thread?.threadId]);

  useEffect(
    () => () => {
      if (copyResetTimeoutRef.current) clearTimeout(copyResetTimeoutRef.current);
    },
    [],
  );

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

  const handleCopy = useCallback(async (itemId: string, value: string) => {
    if (!value.trim()) return;
    try {
      await Clipboard.setStringAsync(value);
      setCopiedItemId(itemId);
      if (copyResetTimeoutRef.current) clearTimeout(copyResetTimeoutRef.current);
      copyResetTimeoutRef.current = setTimeout(() => {
        setCopiedItemId((current) => (current === itemId ? null : current));
        copyResetTimeoutRef.current = null;
      }, 1800);
    } catch {
      // Ignore copy errors; UI remains responsive.
    }
  }, []);

  const openMessageMenu = useCallback(
    (itemId: string, value: string) => {
      Alert.alert('Message actions', undefined, [
        {
          text: copiedItemId === itemId ? 'Copied' : 'Copy',
          onPress: () => {
            void handleCopy(itemId, value);
          },
        },
        { text: 'Cancel', style: 'cancel' },
      ]);
    },
    [copiedItemId, handleCopy],
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
        <View style={styles.scrollArea}>
          <ScrollView
            ref={scrollViewRef}
            contentContainerStyle={styles.content}
            onContentSizeChange={(_, contentHeight) => {
              updateScrollMetrics({ contentHeight });
            }}
            onScroll={handleScroll}
            scrollEventThrottle={16}
            showsVerticalScrollIndicator={false}>
            {turnGroups.length === 0 ? (
              <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>No messages yet.</Text>
            ) : (
              turnGroups.map((turnGroup) => {
                const toolItems = turnGroup.items.filter((entry) => entry.kind === 'tool');
                const firstToolIndex = turnGroup.items.findIndex((entry) => entry.kind === 'tool');
                return (
                  <View key={turnGroup.id} style={styles.turnGroup}>
                    {turnGroup.items.map((item, index) => {
                      if (item.kind === 'tool') {
                        if (index !== firstToolIndex || toolItems.length === 0) return null;
                        return (
                          <ChatTurnActivity
                            key={`activity-${turnGroup.id}`}
                            activeTurnId={thread.activeTurnId}
                            palette={palette}
                            running={thread.running}
                            toolItems={toolItems}
                            turnId={turnGroup.turnId}
                          />
                        );
                      }

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
                      const showActions = activeActionItemId === item.id;
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
                          <Pressable
                            accessibilityRole="button"
                            accessibilityHint="Tap for actions, long press for menu"
                            delayLongPress={280}
                            onPress={() => {
                              setActiveActionItemId((current) => (current === item.id ? null : item.id));
                            }}
                            onLongPress={() => {
                              setActiveActionItemId(item.id);
                              openMessageMenu(item.id, body);
                            }}
                            style={({ pressed }) => [
                              styles.messagePressArea,
                              {
                                opacity: pressed ? 0.96 : 1,
                              },
                            ]}>
                            <View style={styles.itemMeta}>
                              <Text
                                style={[
                                  styles.itemLabel,
                                  { color: user ? palette.surface : palette.textSecondary },
                                ]}>
                                {labelForItem(item)}
                              </Text>
                            </View>
                            {user ? (
                              <Text style={[styles.itemBody, { color: user ? palette.surface : palette.text }]}>
                                {body}
                              </Text>
                            ) : (
                              <ChatMarkdown content={body} itemKind={item.kind} palette={palette} />
                            )}
                          </Pressable>
                          {showActions ? (
                            <View
                              style={[
                                styles.messageActions,
                                {
                                  borderTopColor: user ? 'rgba(255, 255, 255, 0.24)' : palette.border,
                                },
                              ]}>
                              <Pressable
                                accessibilityRole="button"
                                accessibilityLabel={copiedItemId === item.id ? 'Copied' : 'Copy message'}
                                hitSlop={8}
                                onPress={() => {
                                  void handleCopy(item.id, body);
                                }}
                                style={({ pressed }) => [
                                  styles.iconButton,
                                  {
                                    backgroundColor: user ? 'rgba(255, 255, 255, 0.12)' : palette.surface,
                                    borderColor: user ? 'rgba(255, 255, 255, 0.24)' : palette.border,
                                    opacity: pressed ? 0.82 : 1,
                                  },
                                ]}>
                                <Feather
                                  name={copiedItemId === item.id ? 'check' : 'copy'}
                                  size={14}
                                  color={user ? palette.surface : palette.textSecondary}
                                />
                              </Pressable>
                              <Pressable
                                accessibilityRole="button"
                                accessibilityLabel="Open message menu"
                                hitSlop={8}
                                onPress={() => {
                                  openMessageMenu(item.id, body);
                                }}
                                style={({ pressed }) => [
                                  styles.iconButton,
                                  {
                                    backgroundColor: user ? 'rgba(255, 255, 255, 0.12)' : palette.surface,
                                    borderColor: user ? 'rgba(255, 255, 255, 0.24)' : palette.border,
                                    opacity: pressed ? 0.82 : 1,
                                  },
                                ]}>
                                <Feather
                                  name="more-horizontal"
                                  size={14}
                                  color={user ? palette.surface : palette.textSecondary}
                                />
                              </Pressable>
                            </View>
                          ) : null}
                        </View>
                      );
                    })}
                  </View>
                );
              })
            )}
          </ScrollView>
          {showJumpToLatest && thread.items.length > 0 ? (
            <Pressable
              accessibilityRole="button"
              accessibilityLabel="Jump to latest message"
              onPress={jumpToLatest}
              style={({ pressed }) => [
                styles.jumpButton,
                {
                  backgroundColor: palette.surface,
                  borderColor: palette.border,
                  opacity: pressed ? 0.86 : 1,
                },
              ]}>
              <Feather name="arrow-down" size={14} color={palette.textSecondary} />
              <Text style={[styles.jumpLabel, { color: palette.textSecondary }]}>Latest</Text>
            </Pressable>
          ) : null}
        </View>
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
  scrollArea: {
    flex: 1,
  },
  turnGroup: {
    gap: spacing.xs,
  },
  item: {
    borderRadius: radius.md,
    borderWidth: 1,
    maxWidth: '90%',
    padding: spacing.sm,
  },
  messagePressArea: {
    gap: spacing.xs,
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
  itemMeta: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'flex-start',
  },
  itemBody: {
    ...typography.body,
    fontSize: 14,
    fontWeight: '400',
  },
  messageActions: {
    alignItems: 'center',
    borderTopWidth: 1,
    flexDirection: 'row',
    gap: spacing.xs,
    marginTop: spacing.xs,
    paddingTop: spacing.xs,
  },
  iconButton: {
    alignItems: 'center',
    borderRadius: radius.sm,
    borderWidth: 1,
    height: 32,
    justifyContent: 'center',
    width: 32,
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
  jumpButton: {
    alignItems: 'center',
    borderRadius: radius.pill,
    borderWidth: 1,
    bottom: spacing.md,
    flexDirection: 'row',
    gap: spacing.xs,
    minHeight: 36,
    paddingHorizontal: spacing.sm,
    position: 'absolute',
    right: spacing.md,
  },
  jumpLabel: {
    ...typography.label,
    fontSize: 11,
    textTransform: 'uppercase',
  },
});
