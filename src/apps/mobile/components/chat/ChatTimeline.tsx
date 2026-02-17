// ABOUTME: Chat timeline with pinned-to-latest state machine, grouped tool activity, and inline approval handling.
// ABOUTME: Provides robust loading/empty/error states and touch-friendly message actions for mobile chat UX.

import {
  AUTH_COPY,
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
  Platform,
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

import { AuthRedirectBanner } from './AuthRedirectBanner';
import { ChatTurnActivity } from './ChatTurnActivity';
import { ChatTimelineMessageItem, triggerMessageHaptic } from './ChatTimelineMessageItem';
import { ChatTimelineStateCard } from './ChatTimelineStateCard';
import { styles } from './chat-timeline-styles';

/* ── constants ─────────────────────────────────────────── */

/** Offset (in px) within which the list counts as "at the bottom". */
const PIN_THRESHOLD = 32;

/** Offset beyond which the jump-to-latest FAB appears. */
const FAB_THRESHOLD = 240;

/** Estimated average height (px) of a turn group, used by getItemLayout. */
const ESTIMATED_ITEM_HEIGHT = 120;

/* ── types ─────────────────────────────────────────────── */

type ScrollState = 'pinned' | 'unpinned';

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
  /** True if all enabled providers are logged in; false triggers auth banner */
  providerAuthOk?: boolean;
  onRetry?: () => void;
  onApprovalDecision?: (
    requestId: number | string,
    decision: 'accept' | 'decline' | 'accept_for_session',
  ) => Promise<void> | void;
}

function approvalStatusValue(item: ChatItem, localApprovalStatus: Record<string, string>): string {
  if (item.kind !== 'approval') return 'pending';
  return localApprovalStatus[item.id] ?? item.status ?? 'pending';
}

export function ChatTimeline({
  thread,
  loading,
  status = 'disconnected',
  providerAuthOk = true,
  hasTarget = true,
  error = null,
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
  const [unreadCount, setUnreadCount] = useState(0);

  const flatListRef = useRef<FlatList<ChatTurnGroup> | null>(null);
  const copyResetTimeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const animationFrameRef = useRef<number | null>(null);
  const lastThreadIdRef = useRef<string | undefined>(undefined);

  /* ── scroll state machine refs ───────────────────────── */
  const scrollStateRef = useRef<ScrollState>('pinned');
  const isUserDraggingRef = useRef(false);
  const prevItemCountRef = useRef(0);

  const turnGroups = useMemo(() => (thread ? groupChatItemsByTurn(thread.items) : []), [thread?.items]);
  const reversedGroups = useMemo(() => [...turnGroups].reverse(), [turnGroups]);

  const getItemLayout = useCallback(
    (_data: ArrayLike<ChatTurnGroup> | null | undefined, index: number) => ({
      length: ESTIMATED_ITEM_HEIGHT,
      offset: ESTIMATED_ITEM_HEIGHT * index,
      index,
    }),
    [],
  );

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

  /* ── thread switch: reset everything ─────────────────── */
  useEffect(() => {
    if (thread?.chatId === lastThreadIdRef.current) return;
    lastThreadIdRef.current = thread?.chatId;
    scrollStateRef.current = 'pinned';
    prevItemCountRef.current = thread?.items.length ?? 0;
    setRespondingItemId(null);
    setLocalApprovalStatus({});
    setCopiedItemId(null);
    setActiveActionItemId(null);
    setShowJumpToLatest(false);
    setUnreadCount(0);
    if (copyResetTimeoutRef.current) {
      clearTimeout(copyResetTimeoutRef.current);
      copyResetTimeoutRef.current = null;
    }
    if (thread) scheduleScrollToLatest(false);
  }, [scheduleScrollToLatest, thread]);

  /* ── auto-scroll when pinned; track unread when unpinned */
  useEffect(() => {
    if (!thread) return;
    const currentCount = thread.items.length;
    const delta = currentCount - prevItemCountRef.current;
    prevItemCountRef.current = currentCount;

    if (scrollStateRef.current === 'pinned') {
      scheduleScrollToLatest(false);
    } else if (delta > 0) {
      // Accumulate unread while user is scrolled away
      setUnreadCount((prev) => prev + delta);
    }
  }, [scheduleScrollToLatest, thread?.items.length]);

  /* ── cleanup timers on unmount ───────────────────────── */
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

  /* ── jump-to-latest: force pin + clear unread ────────── */
  const jumpToLatest = useCallback(() => {
    scrollStateRef.current = 'pinned';
    setShowJumpToLatest(false);
    setUnreadCount(0);
    scrollToLatest(true);
  }, [scrollToLatest]);

  /* ── scroll state machine handlers ───────────────────── */

  const handleScrollBeginDrag = useCallback(() => {
    isUserDraggingRef.current = true;
  }, []);

  const handleScroll = useCallback((event: NativeSyntheticEvent<NativeScrollEvent>) => {
    const offsetY = event.nativeEvent.contentOffset.y;
    const nearBottom = offsetY <= PIN_THRESHOLD;

    if (scrollStateRef.current === 'pinned') {
      // Only unpin on user-initiated scroll away from bottom
      if (!nearBottom && isUserDraggingRef.current) {
        scrollStateRef.current = 'unpinned';
      }
    } else {
      // Re-pin when user scrolls back to bottom
      if (nearBottom) {
        scrollStateRef.current = 'pinned';
        setUnreadCount(0);
      }
    }

    setShowJumpToLatest(offsetY > FAB_THRESHOLD);
  }, []);

  const handleMomentumScrollEnd = useCallback((event: NativeSyntheticEvent<NativeScrollEvent>) => {
    isUserDraggingRef.current = false;
    // Check final resting position
    const offsetY = event.nativeEvent.contentOffset.y;
    if (offsetY <= PIN_THRESHOLD) {
      scrollStateRef.current = 'pinned';
      setUnreadCount(0);
    }
  }, []);

  const handleScrollEndDrag = useCallback((event: NativeSyntheticEvent<NativeScrollEvent>) => {
    // If no momentum (user lifted finger without flick), mark drag done
    const velocity = event.nativeEvent.velocity;
    if (!velocity || (Math.abs(velocity.y) < 0.1)) {
      isUserDraggingRef.current = false;
      const offsetY = event.nativeEvent.contentOffset.y;
      if (offsetY <= PIN_THRESHOLD) {
        scrollStateRef.current = 'pinned';
        setUnreadCount(0);
      }
    }
  }, []);

  const handleContentSizeChange = useCallback(() => {
    if (scrollStateRef.current !== 'pinned') return;
    scrollToLatest(false);
  }, [scrollToLatest]);

  const handleToggleActions = useCallback((itemId: string) => {
    setActiveActionItemId((current) => (current === itemId ? null : itemId));
  }, []);

  const handleCopyAction = useCallback((itemId: string, body: string) => {
    void handleCopy(itemId, body);
  }, [handleCopy]);

  const handleApprovalAction = useCallback(
    (approvalItem: ChatItem, decision: 'accept' | 'decline' | 'accept_for_session') => {
      void handleDecision(approvalItem, decision);
    },
    [handleDecision],
  );

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

            const itemIsStreaming =
              (thread?.running ?? false) &&
              item.turnId !== undefined &&
              item.turnId === thread?.activeTurnId &&
              item.kind === 'assistant';

            return (
              <ChatTimelineMessageItem
                key={item.id}
                item={item}
                palette={palette}
                isCopied={copiedItemId === item.id}
                showActions={activeActionItemId === item.id}
                isStreaming={itemIsStreaming}
                approvalStatusForItem={
                  item.kind === 'approval' ? approvalStatusValue(item, localApprovalStatus) : undefined
                }
                approvalResponding={item.kind === 'approval' && respondingItemId === item.id}
                hasApprovalHandler={onApprovalDecision !== undefined}
                onToggleActions={handleToggleActions}
                onOpenMenu={openMessageMenu}
                onCopy={handleCopyAction}
                onApprovalDecision={handleApprovalAction}
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
      handleCopyAction,
      handleToggleActions,
      handleApprovalAction,
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

  const badgeLabel = unreadCount > 99 ? '99+' : String(unreadCount);

  return (
    <View style={[styles.container, { backgroundColor: palette.background }]}>
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

      <AuthRedirectBanner
        visible={!providerAuthOk}
        message={AUTH_COPY.bannerMessage}
      />

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
            getItemLayout={getItemLayout}
            inverted
            importantForAccessibility="yes"
            accessibilityLabel="Chat timeline"
            accessibilityHint="Newest messages are near the composer. Swipe up for older messages."
            keyboardDismissMode="interactive"
            keyboardShouldPersistTaps="handled"
            showsVerticalScrollIndicator={false}
            onScrollBeginDrag={handleScrollBeginDrag}
            onScroll={handleScroll}
            onScrollEndDrag={handleScrollEndDrag}
            onMomentumScrollEnd={handleMomentumScrollEnd}
            onContentSizeChange={handleContentSizeChange}
            maintainVisibleContentPosition={{
              minIndexForVisible: 0,
              autoscrollToTopThreshold: 20,
            }}
            scrollEventThrottle={16}
            initialNumToRender={8}
            maxToRenderPerBatch={6}
            updateCellsBatchingPeriod={32}
            windowSize={7}
            removeClippedSubviews={Platform.OS === 'android'}
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
                accessibilityLabel={
                  unreadCount > 0
                    ? `Jump to latest message, ${unreadCount} unread`
                    : 'Jump to latest message'
                }
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
                {unreadCount > 0 ? (
                  <View style={[styles.jumpBadge, { backgroundColor: palette.accent }]}>
                    <Text style={[styles.jumpBadgeLabel, { color: palette.surface0 }]}>
                      {badgeLabel}
                    </Text>
                  </View>
                ) : null}
              </Pressable>
            </Animated.View>
          ) : null}
        </View>
      )}
    </View>
  );
}
