// ABOUTME: Chat timeline with pinned-to-latest behavior, grouped tool activity, and inline approval handling.
// ABOUTME: Provides robust loading/empty/error states and touch-friendly message actions for mobile chat UX.

import {
  formatRelativeTime,
  groupChatItemsByTurn,
  type ChatItem,
  type ChatTurnGroup,
  type ConnectionStatus,
} from '@homie/shared';
import {
  AlertCircle,
  ArrowDown,
  Clock3,
  Link2,
  LoaderCircle,
  MessageCircle,
  WifiOff,
} from 'lucide-react-native';
import * as Clipboard from 'expo-clipboard';
import * as Haptics from 'expo-haptics';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  Alert,
  FlatList,
  Pressable,
  Text,
  View,
  type ListRenderItemInfo,
  type NativeScrollEvent,
  type NativeSyntheticEvent,
} from 'react-native';
import Animated, { FadeIn, FadeInUp, FadeOut } from 'react-native-reanimated';

import { useAppTheme } from '@/hooks/useAppTheme';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion } from '@/theme/motion';

import { ChatTurnActivity } from './ChatTurnActivity';
import { ChatTimelineMessageItem, triggerMessageHaptic } from './ChatTimelineMessageItem';
import { ChatTimelineStateCard } from './ChatTimelineStateCard';
import {
  colorsForTone,
  statusForConnection,
} from './chat-timeline-helpers';
import { styles } from './chat-timeline-styles';

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
  status?: ConnectionStatus;
  hasTarget?: boolean;
  error?: string | null;
  threadLastActivityAt?: number;
  onRetry?: () => void;
  onApprovalDecision?: (
    requestId: number | string,
    decision: 'accept' | 'decline' | 'accept_for_session',
  ) => Promise<void> | void;
}

function callsLabel(count: number): string {
  return count === 1 ? '1 message' : `${count} messages`;
}

function approvalStatusValue(item: ChatItem, localApprovalStatus: Record<string, string>): string {
  if (item.kind !== 'approval') return 'pending';
  return localApprovalStatus[item.id] ?? item.status ?? 'pending';
}

export function ChatTimeline({
  thread,
  loading,
  status = 'disconnected',
  hasTarget = true,
  error = null,
  threadLastActivityAt,
  onRetry,
  onApprovalDecision,
}: ChatTimelineProps) {
  const { palette } = useAppTheme();
  const reducedMotion = useReducedMotion();

  const [respondingItemId, setRespondingItemId] = useState<string | null>(null);
  const [localApprovalStatus, setLocalApprovalStatus] = useState<Record<string, string>>({});
  const [copiedItemId, setCopiedItemId] = useState<string | null>(null);
  const [activeActionItemId, setActiveActionItemId] = useState<string | null>(null);
  const [showJumpToLatest, setShowJumpToLatest] = useState(false);

  const flatListRef = useRef<FlatList<ChatTurnGroup> | null>(null);
  const copyResetTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const pinnedToLatestRef = useRef(true);
  const lastThreadIdRef = useRef<string | undefined>(undefined);

  const turnGroups = useMemo(() => (thread ? groupChatItemsByTurn(thread.items) : []), [thread?.items]);
  const reversedGroups = useMemo(() => [...turnGroups].reverse(), [turnGroups]);

  const pendingApprovalCount = useMemo(() => {
    if (!thread) return 0;
    return thread.items.reduce((count, item) => {
      if (item.kind !== 'approval') return count;
      return approvalStatusValue(item, localApprovalStatus) === 'pending' ? count + 1 : count;
    }, 0);
  }, [localApprovalStatus, thread]);

  const statusState = statusForConnection(status);
  const statusColors = colorsForTone(palette, statusState.tone);

  const threadSummary = useMemo(() => {
    if (!thread) return '';
    const countLabel = callsLabel(thread.items.length);
    const updated = threadLastActivityAt ? formatRelativeTime(threadLastActivityAt) : '';
    return updated ? `${countLabel} · updated ${updated}` : countLabel;
  }, [thread, threadLastActivityAt]);

  const scrollToLatest = useCallback((animated: boolean) => {
    flatListRef.current?.scrollToOffset({ offset: 0, animated });
  }, []);

  const scheduleScrollToLatest = useCallback(
    (animated: boolean) => {
      if (animationFrameRef.current !== null) cancelAnimationFrame(animationFrameRef.current);
      animationFrameRef.current = requestAnimationFrame(() => {
        scrollToLatest(animated);
        animationFrameRef.current = null;
      });
    },
    [scrollToLatest],
  );

  useEffect(() => {
    if (thread?.chatId === lastThreadIdRef.current) return;
    lastThreadIdRef.current = thread?.chatId;
    pinnedToLatestRef.current = true;
    setRespondingItemId(null);
    setLocalApprovalStatus({});
    setCopiedItemId(null);
    setActiveActionItemId(null);
    setShowJumpToLatest(false);
    if (copyResetTimeoutRef.current) {
      clearTimeout(copyResetTimeoutRef.current);
      copyResetTimeoutRef.current = null;
    }
    if (thread) scheduleScrollToLatest(false);
  }, [scheduleScrollToLatest, thread]);

  useEffect(() => {
    if (!thread || !pinnedToLatestRef.current) return;
    scheduleScrollToLatest(false);
  }, [scheduleScrollToLatest, thread?.items.length]);

  useEffect(
    () => () => {
      if (copyResetTimeoutRef.current) clearTimeout(copyResetTimeoutRef.current);
      if (animationFrameRef.current !== null) cancelAnimationFrame(animationFrameRef.current);
    },
    [],
  );

  const handleDecision = useCallback(
    async (item: ChatItem, decision: 'accept' | 'decline' | 'accept_for_session') => {
      if (!onApprovalDecision || item.requestId === undefined) return;
      setRespondingItemId(item.id);
      if (decision === 'decline') {
        if (Haptics.notificationAsync) {
          await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Error);
        }
      } else if (Haptics.notificationAsync) {
        await Haptics.notificationAsync(Haptics.NotificationFeedbackType.Success);
      }
      try {
        await onApprovalDecision(item.requestId, decision);
        setLocalApprovalStatus((current) => ({ ...current, [item.id]: decision }));
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
      return;
    }
  }, []);

  const openMessageMenu = useCallback(
    (itemId: string, value: string) => {
      triggerMessageHaptic();
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
    pinnedToLatestRef.current = true;
    setShowJumpToLatest(false);
    scrollToLatest(true);
  }, [scrollToLatest]);

  const handleScroll = useCallback((event: NativeSyntheticEvent<NativeScrollEvent>) => {
    const offsetY = event.nativeEvent.contentOffset.y;
    const isPinned = offsetY <= 32;
    pinnedToLatestRef.current = isPinned;
    setShowJumpToLatest(offsetY > 240);
  }, []);

  const handleContentSizeChange = useCallback(() => {
    if (!pinnedToLatestRef.current) return;
    scrollToLatest(false);
  }, [scrollToLatest]);

  const renderTurnGroup = useCallback(
    ({ item: turnGroup, index }: ListRenderItemInfo<ChatTurnGroup>) => {
      const toolItems = turnGroup.items.filter((entry) => entry.kind === 'tool');
      const firstToolIndex = turnGroup.items.findIndex((entry) => entry.kind === 'tool');
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

            return (
              <ChatTimelineMessageItem
                key={item.id}
                item={item}
                palette={palette}
                copiedItemId={copiedItemId}
                activeActionItemId={activeActionItemId}
                localApprovalStatus={localApprovalStatus}
                respondingItemId={respondingItemId}
                hasApprovalHandler={onApprovalDecision !== undefined}
                onToggleActions={(itemId) => {
                  setActiveActionItemId((current) => (current === itemId ? null : itemId));
                }}
                onOpenMenu={openMessageMenu}
                onCopy={(itemId, body) => {
                  void handleCopy(itemId, body);
                }}
                onApprovalDecision={(approvalItem, decision) => {
                  void handleDecision(approvalItem, decision);
                }}
              />
            );
          })}
          <View style={[styles.turnDivider, { backgroundColor: palette.border }]} />
        </Animated.View>
      );
    },
    [
      reducedMotion,
      palette,
      thread?.activeTurnId,
      thread?.running,
      copiedItemId,
      activeActionItemId,
      localApprovalStatus,
      respondingItemId,
      onApprovalDecision,
      openMessageMenu,
      handleCopy,
      handleDecision,
    ],
  );

  if (!thread) {
    if (!hasTarget) {
      return (
        <View style={[styles.emptyContainer, { backgroundColor: palette.background }]}> 
          <Animated.View entering={reducedMotion ? undefined : FadeIn.duration(motion.duration.fast)}>
            <ChatTimelineStateCard
              icon={Link2}
              title="Gateway target needed"
              body="Open Settings and add your gateway URL to start chatting remotely."
              palette={palette}
              tone="warning"
            />
          </Animated.View>
        </View>
      );
    }

    if (status === 'connecting' || status === 'handshaking') {
      return (
        <View style={[styles.emptyContainer, { backgroundColor: palette.background }]}> 
          <Animated.View entering={reducedMotion ? undefined : FadeIn.duration(motion.duration.fast)}>
            <ChatTimelineStateCard
              icon={LoaderCircle}
              title="Connecting"
              body="Re-establishing your gateway session..."
              palette={palette}
            />
          </Animated.View>
        </View>
      );
    }

    if (status !== 'connected') {
      const body = error?.trim()
        ? error
        : 'Connection unavailable. Retry from the menu once the gateway is reachable.';
      return (
        <View style={[styles.emptyContainer, { backgroundColor: palette.background }]}> 
          <Animated.View entering={reducedMotion ? undefined : FadeIn.duration(motion.duration.fast)}>
            <ChatTimelineStateCard
              icon={WifiOff}
              title="Gateway unavailable"
              body={body}
              palette={palette}
              tone="warning"
              actionLabel={onRetry ? 'Retry connection' : undefined}
              onAction={onRetry}
            />
          </Animated.View>
        </View>
      );
    }

    return (
      <View style={[styles.emptyContainer, { backgroundColor: palette.background }]}> 
        <Animated.View entering={reducedMotion ? undefined : FadeIn.duration(motion.duration.fast)}>
          <ChatTimelineStateCard
            icon={MessageCircle}
            title="Start a conversation"
            body="Create a chat from the menu, then send a message to begin."
            palette={palette}
          />
        </Animated.View>
      </View>
    );
  }

  return (
    <View style={[styles.container, { backgroundColor: palette.background }]}> 
      <View style={[styles.header, { borderBottomColor: palette.border, backgroundColor: palette.surface0 }]}> 
        <View style={styles.headerMain}> 
          <Text numberOfLines={1} style={[styles.headerTitle, { color: palette.text }]}>
            {thread.title}
          </Text>
          <Text numberOfLines={1} style={[styles.headerMeta, { color: palette.textSecondary }]}>
            {threadSummary}
          </Text>
        </View>

        <View style={styles.headerPills}>
          {pendingApprovalCount > 0 ? (
            <View
              style={[
                styles.headerPill,
                {
                  backgroundColor: palette.warningDim,
                  borderColor: palette.warning,
                },
              ]}>
              <Text style={[styles.headerPillLabel, { color: palette.warning }]}> 
                {pendingApprovalCount === 1 ? '1 approval' : `${pendingApprovalCount} approvals`}
              </Text>
            </View>
          ) : null}

          <View
            style={[
              styles.headerPill,
              {
                backgroundColor: thread.running ? palette.successDim : statusColors.background,
                borderColor: thread.running ? palette.success : statusColors.foreground,
              },
            ]}>
            <Text
              style={[
                styles.headerPillLabel,
                {
                  color: thread.running ? palette.success : statusColors.foreground,
                },
              ]}>
              {thread.running ? 'Running' : statusState.label}
            </Text>
          </View>
        </View>
      </View>

      {error ? (
        <Animated.View
          entering={reducedMotion ? undefined : FadeIn.duration(motion.duration.micro)}
          style={[
            styles.errorBanner,
            {
              backgroundColor: palette.dangerDim,
              borderBottomColor: palette.border,
            },
          ]}>
          <AlertCircle size={14} color={palette.danger} />
          <Text numberOfLines={2} style={[styles.errorText, { color: palette.danger }]}>{error}</Text>
        </Animated.View>
      ) : null}

      {loading && thread.items.length === 0 ? (
        <View style={styles.loadingWrap}>
          <ChatTimelineStateCard
            icon={Clock3}
            title="Loading messages"
            body="Syncing thread history from the gateway..."
            palette={palette}
          />
        </View>
      ) : (
        <View style={styles.listArea}>
          <FlatList
            ref={flatListRef}
            data={reversedGroups}
            renderItem={renderTurnGroup}
            keyExtractor={(group) => group.id}
            inverted
            accessibilityLabel="Chat timeline"
            keyboardDismissMode="interactive"
            keyboardShouldPersistTaps="handled"
            showsVerticalScrollIndicator={false}
            onScroll={handleScroll}
            onContentSizeChange={handleContentSizeChange}
            maintainVisibleContentPosition={{
              minIndexForVisible: 0,
              autoscrollToTopThreshold: 20,
            }}
            scrollEventThrottle={16}
            contentContainerStyle={styles.listContent}
            ListEmptyComponent={
              <View style={styles.emptyListWrap}>
                <Text style={[styles.emptyListLabel, { color: palette.textSecondary }]}>No messages yet.</Text>
              </View>
            }
          />

          {showJumpToLatest && thread.items.length > 0 ? (
            <Animated.View
              entering={reducedMotion ? undefined : FadeIn.duration(motion.duration.fast)}
              exiting={reducedMotion ? undefined : FadeOut.duration(motion.duration.micro)}
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
                <ArrowDown size={16} color={palette.textSecondary} />
              </Pressable>
            </Animated.View>
          ) : null}
        </View>
      )}
    </View>
  );
}
