// ABOUTME: Chat message timeline using an inverted FlatList for virtualized, auto-scrolling display.
// ABOUTME: Renders messages in a flat, Slack/Linear-like layout with avatars, timestamps, and animated entry.

import { groupChatItemsByTurn, type ChatItem, type ChatTurnGroup } from '@homie/shared';
import { Feather } from '@expo/vector-icons';
import * as Clipboard from 'expo-clipboard';
import * as Haptics from 'expo-haptics';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Alert,
  FlatList,
  Platform,
  Pressable,
  StyleSheet,
  Text,
  View,
  type ListRenderItemInfo,
} from 'react-native';
import Animated, {
  FadeIn,
  FadeInUp,
  FadeOut,
} from 'react-native-reanimated';

import { useAppTheme } from '@/hooks/useAppTheme';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion } from '@/theme/motion';
import { elevation, palettes, radius, spacing, typography } from '@/theme/tokens';
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
  if (item.kind === 'assistant') return 'Gateway';
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

function avatarInitial(item: ChatItem): string {
  if (item.kind === 'user') return 'Y';
  return 'G';
}

function formatTimestamp(): string {
  const now = new Date();
  const h = now.getHours();
  const m = now.getMinutes();
  return `${h}:${m.toString().padStart(2, '0')}`;
}

export function ChatTimeline({ thread, loading, onApprovalDecision }: ChatTimelineProps) {
  const { palette } = useAppTheme();
  const reducedMotion = useReducedMotion();
  const [respondingItemId, setRespondingItemId] = useState<string | null>(null);
  const [localApprovalStatus, setLocalApprovalStatus] = useState<Record<string, string>>({});
  const [copiedItemId, setCopiedItemId] = useState<string | null>(null);
  const [activeActionItemId, setActiveActionItemId] = useState<string | null>(null);
  const [showJumpToLatest, setShowJumpToLatest] = useState(false);
  const copyResetTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const flatListRef = useRef<FlatList<ChatTurnGroup> | null>(null);
  const lastThreadIdRef = useRef<string | undefined>(undefined);

  const turnGroups = useMemo(
    () => (thread ? groupChatItemsByTurn(thread.items) : []),
    [thread?.items],
  );

  // For inverted FlatList, reverse data so newest items render at top (visual bottom)
  const reversedGroups = useMemo(() => [...turnGroups].reverse(), [turnGroups]);

  useEffect(() => {
    if (thread?.chatId !== lastThreadIdRef.current) {
      lastThreadIdRef.current = thread?.chatId;
      setRespondingItemId(null);
      setLocalApprovalStatus({});
      setCopiedItemId(null);
      setActiveActionItemId(null);
      setShowJumpToLatest(false);
      if (copyResetTimeoutRef.current) {
        clearTimeout(copyResetTimeoutRef.current);
        copyResetTimeoutRef.current = null;
      }
    }
  }, [thread?.chatId]);

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
      if (Platform.OS !== 'web') {
        Haptics.notificationAsync(
          decision === 'decline'
            ? Haptics.NotificationFeedbackType.Error
            : Haptics.NotificationFeedbackType.Success,
        );
      }
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
      if (Platform.OS !== 'web') {
        Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Medium);
      }
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

  const jumpToLatest = useCallback(() => {
    setShowJumpToLatest(false);
    flatListRef.current?.scrollToOffset({ offset: 0, animated: true });
  }, []);

  const handleScroll = useCallback(
    (event: { nativeEvent: { contentOffset: { y: number } } }) => {
      const offsetY = event.nativeEvent.contentOffset.y;
      // In an inverted list, offset 0 is the bottom (latest). Show jump when scrolled up.
      setShowJumpToLatest(offsetY > 400);
    },
    [],
  );

  const renderMessageItem = useCallback(
    (item: ChatItem) => {
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
                backgroundColor: palette.surface1,
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
              <Text style={[styles.messageBody, { color: palette.text }]}>{item.reason}</Text>
            ) : null}
            {item.command ? (
              <View
                style={[
                  styles.commandCard,
                  {
                    backgroundColor: palette.surface0,
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
                  accessibilityLabel="Accept"
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
                  <Text style={[styles.approvalLabel, { color: palettes.light.surface0 }]}>
                    {responding ? '...' : 'Accept'}
                  </Text>
                </Pressable>
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel="Always accept for session"
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
                  <Text style={[styles.approvalLabel, { color: palettes.light.surface0 }]}>
                    Always
                  </Text>
                </Pressable>
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel="Decline"
                  disabled={!canRespond}
                  onPress={() => {
                    void handleDecision(item, 'decline');
                  }}
                  style={({ pressed }) => [
                    styles.approvalButton,
                    {
                      backgroundColor: palette.surface0,
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
      const isUser = item.kind === 'user';
      const showActions = activeActionItemId === item.id;

      return (
        <Pressable
          key={item.id}
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
            styles.messageRow,
            {
              backgroundColor: pressed ? palette.surface1 : 'transparent',
            },
          ]}>
          {/* Avatar */}
          <View
            style={[
              styles.avatar,
              {
                backgroundColor: isUser ? palette.accent : palette.surface1,
              },
            ]}>
            {isUser ? (
              <Text style={styles.avatarText}>{avatarInitial(item)}</Text>
            ) : (
              <Feather name="cpu" size={14} color={palette.textSecondary} />
            )}
          </View>

          {/* Content */}
          <View style={styles.messageContent}>
            <View style={styles.messageMeta}>
              <Text style={[styles.senderName, { color: palette.text }]}>
                {labelForItem(item)}
              </Text>
              <Text style={[styles.timestamp, { color: palette.textSecondary }]}>
                {formatTimestamp()}
              </Text>
            </View>
            {isUser ? (
              <Text style={[styles.messageBody, { color: palette.text }]}>{body}</Text>
            ) : (
              <ChatMarkdown content={body} itemKind={item.kind} palette={palette} />
            )}
            {showActions ? (
              <View style={[styles.messageActions, { borderTopColor: palette.border }]}>
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
                      backgroundColor: palette.surface1,
                      borderColor: palette.border,
                      opacity: pressed ? 0.82 : 1,
                    },
                  ]}>
                  <Feather
                    name={copiedItemId === item.id ? 'check' : 'copy'}
                    size={13}
                    color={palette.textSecondary}
                  />
                </Pressable>
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel="More actions"
                  hitSlop={8}
                  onPress={() => {
                    openMessageMenu(item.id, body);
                  }}
                  style={({ pressed }) => [
                    styles.iconButton,
                    {
                      backgroundColor: palette.surface1,
                      borderColor: palette.border,
                      opacity: pressed ? 0.82 : 1,
                    },
                  ]}>
                  <Feather
                    name="more-horizontal"
                    size={13}
                    color={palette.textSecondary}
                  />
                </Pressable>
              </View>
            ) : null}
          </View>
        </Pressable>
      );
    },
    [
      activeActionItemId,
      copiedItemId,
      handleCopy,
      handleDecision,
      localApprovalStatus,
      onApprovalDecision,
      openMessageMenu,
      palette,
      respondingItemId,
    ],
  );

  const renderTurnGroup = useCallback(
    ({ item: turnGroup, index }: ListRenderItemInfo<ChatTurnGroup>) => {
      const toolItems = turnGroup.items.filter((entry) => entry.kind === 'tool');
      const firstToolIndex = turnGroup.items.findIndex((entry) => entry.kind === 'tool');

      // Stagger animation: cap at 10 items, then flat delay.
      const staggerDelay = reducedMotion
        ? 0
        : index < 10
          ? index * motion.stagger.tight
          : 10 * motion.stagger.tight;
      const entering = reducedMotion
        ? undefined
        : FadeInUp.delay(staggerDelay).duration(motion.duration.fast);

      return (
        <Animated.View entering={entering} style={styles.turnGroup}>
          {turnGroup.items.map((item, itemIndex) => {
            if (item.kind === 'tool') {
              if (itemIndex !== firstToolIndex || toolItems.length === 0) return null;
              return (
                <ChatTurnActivity
                  key={`activity-${turnGroup.id}`}
                  activeTurnId={thread?.activeTurnId}
                  palette={palette}
                  running={thread?.running ?? false}
                  toolItems={toolItems}
                  turnId={turnGroup.turnId}
                />
              );
            }
            return renderMessageItem(item);
          })}
          {/* Turn group divider (thin line between groups) */}
          <View style={[styles.turnDivider, { backgroundColor: palette.border }]} />
        </Animated.View>
      );
    },
    [palette, reducedMotion, renderMessageItem, thread?.activeTurnId, thread?.running],
  );

  if (!thread) {
    return (
      <View style={[styles.emptyContainer, { backgroundColor: palette.background }]}>
        <Animated.View
          entering={reducedMotion ? undefined : FadeIn.duration(200)}
          style={styles.emptyContent}>
          <Feather name="message-circle" size={36} color={palette.textSecondary} />
          <Text style={[styles.emptyTitle, { color: palette.text }]}>Start a conversation</Text>
          <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>
            Type a message to begin chatting with the gateway.
          </Text>
        </Animated.View>
      </View>
    );
  }

  return (
    <View style={[styles.container, { backgroundColor: palette.background }]}>
      {/* Thread header bar */}
      <View style={[styles.header, { borderBottomColor: palette.border, backgroundColor: palette.surface0 }]}>
        <Text numberOfLines={1} style={[styles.headerTitle, { color: palette.text }]}>
          {thread.title}
        </Text>
        {thread.running ? (
          <View style={[styles.runningPill, { borderColor: palette.success }]}>
            <View style={[styles.runningDot, { backgroundColor: palette.success }]} />
            <Text style={[styles.runningLabel, { color: palette.success }]}>Running</Text>
          </View>
        ) : null}
      </View>

      {loading ? (
        <View style={styles.loadingWrap}>
          <Text style={[styles.loadingLabel, { color: palette.textSecondary }]}>Loading messages…</Text>
        </View>
      ) : (
        <View style={styles.listArea}>
          <FlatList
            ref={flatListRef}
            data={reversedGroups}
            renderItem={renderTurnGroup}
            keyExtractor={(group) => group.id}
            inverted
            keyboardDismissMode="interactive"
            keyboardShouldPersistTaps="handled"
            showsVerticalScrollIndicator={false}
            onScroll={handleScroll}
            scrollEventThrottle={16}
            contentContainerStyle={styles.listContent}
            ListEmptyComponent={
              <View style={styles.emptyListWrap}>
                <Text style={[styles.emptyBody, { color: palette.textSecondary }]}>No messages yet.</Text>
              </View>
            }
          />
          {showJumpToLatest && thread.items.length > 0 ? (
            <Animated.View
              entering={reducedMotion ? undefined : FadeIn.duration(150)}
              exiting={reducedMotion ? undefined : FadeOut.duration(100)}
              style={styles.jumpButtonWrap}>
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Jump to latest message"
                onPress={jumpToLatest}
                style={({ pressed }) => [
                  styles.jumpButton,
                  {
                    backgroundColor: palette.surface0,
                    borderColor: palette.border,
                    opacity: pressed ? 0.86 : 1,
                  },
                ]}>
                <Feather name="arrow-down" size={14} color={palette.textSecondary} />
              </Pressable>
            </Animated.View>
          ) : null}
        </View>
      )}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
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
  headerTitle: {
    ...typography.label,
    flex: 1,
    fontSize: 14,
  },
  runningPill: {
    alignItems: 'center',
    borderRadius: radius.pill,
    borderWidth: 1,
    flexDirection: 'row',
    gap: spacing.xs,
    paddingHorizontal: spacing.sm,
    paddingVertical: 2,
  },
  runningDot: {
    borderRadius: 3,
    height: 6,
    width: 6,
  },
  runningLabel: {
    ...typography.label,
    fontSize: 10,
    textTransform: 'uppercase',
  },
  listArea: {
    flex: 1,
  },
  listContent: {
    paddingHorizontal: spacing.xs,
    paddingVertical: spacing.sm,
  },
  turnGroup: {
    gap: 0,
  },
  turnDivider: {
    height: 1,
    marginVertical: spacing.sm,
    marginHorizontal: spacing.md,
    opacity: 0.5,
  },
  // Flat, Slack/Linear-like message row
  messageRow: {
    flexDirection: 'row',
    alignItems: 'flex-start',
    gap: spacing.sm,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.xs,
    borderRadius: radius.sm,
  },
  avatar: {
    width: 28,
    height: 28,
    borderRadius: 14,
    alignItems: 'center',
    justifyContent: 'center',
    marginTop: 2,
  },
  avatarText: {
    fontSize: 12,
    fontWeight: '600',
    color: palettes.light.surface0,
  },
  messageContent: {
    flex: 1,
    gap: 2,
  },
  messageMeta: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacing.sm,
  },
  senderName: {
    ...typography.label,
    fontSize: 13,
  },
  timestamp: {
    ...typography.data,
    fontSize: 11,
  },
  messageBody: {
    ...typography.body,
    fontSize: 14,
    fontWeight: '400',
  },
  messageActions: {
    flexDirection: 'row',
    gap: spacing.xs,
    marginTop: spacing.xs,
    paddingTop: spacing.xs,
    borderTopWidth: 1,
  },
  iconButton: {
    alignItems: 'center',
    borderRadius: radius.sm,
    borderWidth: 1,
    height: 28,
    justifyContent: 'center',
    width: 28,
  },
  approvalCard: {
    borderRadius: radius.sm,
    borderWidth: 1,
    gap: spacing.sm,
    marginLeft: 36,
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
    borderRadius: radius.sm,
    borderWidth: 1,
    flex: 1,
    justifyContent: 'center',
    minHeight: 36,
    paddingHorizontal: spacing.sm,
  },
  approvalLabel: {
    ...typography.label,
    fontSize: 12,
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
  emptyContainer: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    padding: spacing.xl,
  },
  emptyContent: {
    alignItems: 'center',
    gap: spacing.sm,
  },
  emptyTitle: {
    ...typography.title,
    fontSize: 18,
  },
  emptyBody: {
    ...typography.body,
    fontWeight: '400',
    textAlign: 'center',
  },
  emptyListWrap: {
    padding: spacing.lg,
    alignItems: 'center',
  },
  jumpButtonWrap: {
    position: 'absolute',
    bottom: spacing.md,
    right: spacing.md,
  },
  jumpButton: {
    alignItems: 'center',
    justifyContent: 'center',
    width: 36,
    height: 36,
    borderRadius: 18,
    borderWidth: 1,
    ...elevation.fab,
  },
});
