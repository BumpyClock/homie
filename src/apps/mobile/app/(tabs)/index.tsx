import FontAwesome from '@expo/vector-icons/FontAwesome';
import { useCallback, useEffect, useState } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import Animated, {
  interpolate,
  useAnimatedStyle,
  useSharedValue,
  withTiming,
} from 'react-native-reanimated';

import { ChatComposer } from '@/components/chat/ChatComposer';
import { ChatTimeline } from '@/components/chat/ChatTimeline';
import { ThreadList } from '@/components/chat/ThreadList';
import { GatewayTargetForm } from '@/components/gateway/GatewayTargetForm';
import { ScreenSurface } from '@/components/ui/ScreenSurface';
import { StatusPill } from '@/components/ui/StatusPill';
import { useAppTheme } from '@/hooks/useAppTheme';
import { useGatewayChat } from '@/hooks/useGatewayChat';
import { useGatewayTarget } from '@/hooks/useGatewayTarget';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion } from '@/theme/motion';
import { radius, spacing, typography } from '@/theme/tokens';

export default function ChatTabScreen() {
  const { palette } = useAppTheme();
  const [savingTarget, setSavingTarget] = useState(false);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const reducedMotion = useReducedMotion();
  const drawerProgress = useSharedValue(0);
  const {
    loading: loadingTarget,
    targetUrl,
    hasTarget,
    targetHint,
    error: targetError,
    saveTarget,
  } = useGatewayTarget();
  const {
    status,
    statusBadge,
    threads,
    activeChatId,
    activeThread,
    error,
    loadingThreads,
    loadingMessages,
    creatingChat,
    sendingMessage,
    selectThread,
    refreshThreads,
    createChat,
    sendMessage,
    respondApproval,
  } = useGatewayChat(targetUrl ?? '');
  const canCreateChat = hasTarget && status === 'connected' && !creatingChat;
  const canRefreshThreads = hasTarget && status === 'connected' && !loadingThreads;

  const handleSaveTarget = async (value: string) => {
    setSavingTarget(true);
    try {
      await saveTarget(value);
    } finally {
      setSavingTarget(false);
    }
  };

  const closeDrawer = useCallback(() => {
    setDrawerOpen(false);
  }, []);

  useEffect(() => {
    if (!hasTarget) {
      setDrawerOpen(false);
    }
  }, [hasTarget]);

  useEffect(() => {
    drawerProgress.value = withTiming(drawerOpen ? 1 : 0, {
      duration: reducedMotion ? 0 : motion.duration.regular,
      easing: motion.easing.enterExit,
    });
  }, [drawerOpen, drawerProgress, reducedMotion]);

  const backdropStyle = useAnimatedStyle(() => ({
    opacity: interpolate(drawerProgress.value, [0, 1], [0, 1]),
  }));

  const panelStyle = useAnimatedStyle(() => ({
    transform: [
      {
        translateX: interpolate(drawerProgress.value, [0, 1], [-420, 0]),
      },
    ],
  }));

  return (
    <ScreenSurface>
      <View style={[styles.container, { backgroundColor: palette.background }]}>
        <View style={styles.headerRow}>
          <View>
            <Text style={[styles.eyebrow, { color: palette.textSecondary }]}>Gateway</Text>
            <Text style={[styles.title, { color: palette.text }]}>Chat</Text>
          </View>
          <View style={styles.headerActions}>
            <Pressable
              accessibilityRole="button"
              accessibilityLabel="Toggle chats panel"
              disabled={!hasTarget}
              onPress={() => {
                setDrawerOpen((current) => !current);
              }}
              style={({ pressed }) => [
                styles.drawerToggle,
                {
                  backgroundColor: palette.surface,
                  borderColor: palette.border,
                  opacity: pressed ? 0.86 : hasTarget ? 1 : 0.55,
                },
              ]}>
              <FontAwesome
                name={drawerOpen ? 'times' : 'bars'}
                size={14}
                color={hasTarget ? palette.text : palette.textSecondary}
              />
              <Text
                style={[
                  styles.drawerToggleLabel,
                  { color: hasTarget ? palette.text : palette.textSecondary },
                ]}>
                Chats
              </Text>
            </Pressable>
            <StatusPill
              label={hasTarget ? statusBadge.label : 'Setup'}
              tone={hasTarget ? statusBadge.tone : 'warning'}
            />
          </View>
        </View>

        {loadingTarget ? (
          <View style={[styles.setupCard, { backgroundColor: palette.surface, borderColor: palette.border }]}>
            <Text style={[styles.setupTitle, { color: palette.text }]}>Loading target</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>
              Checking saved gateway configuration...
            </Text>
          </View>
        ) : null}

        {!loadingTarget && !hasTarget ? (
          <View style={[styles.setupCard, { backgroundColor: palette.surface, borderColor: palette.border }]}>
            <Text style={[styles.setupTitle, { color: palette.text }]}>Connect your gateway</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>
              Add a gateway URL to enable chat on this device.
            </Text>
            <GatewayTargetForm
              initialValue={targetHint}
              hintValue={targetHint}
              saving={savingTarget}
              saveLabel="Save and Connect"
              onSave={handleSaveTarget}
            />
            {targetError ? (
              <Text style={[styles.setupHint, { color: palette.textSecondary }]}>{targetError}</Text>
            ) : null}
          </View>
        ) : null}

        {hasTarget ? (
          <View style={[styles.targetRow, { backgroundColor: palette.surface, borderColor: palette.border }]}>
            <Text numberOfLines={1} style={[styles.targetValue, { color: palette.textSecondary }]}>
              {targetUrl}
            </Text>
          </View>
        ) : null}

        {error ? (
          <View style={[styles.errorCard, { backgroundColor: palette.surface, borderColor: palette.border }]}>
            <Text style={[styles.errorText, { color: palette.danger }]}>{error}</Text>
          </View>
        ) : null}

        <View style={styles.chatPane}>
          <ChatTimeline
            thread={hasTarget ? activeThread : null}
            loading={loadingMessages && hasTarget}
            onApprovalDecision={respondApproval}
          />
          <ChatComposer
            disabled={status !== 'connected' || !activeThread || !hasTarget}
            sending={sendingMessage}
            onSend={sendMessage}
          />
        </View>

        {hasTarget ? (
          <View pointerEvents={drawerOpen ? 'auto' : 'none'} style={styles.drawerLayer}>
            <Animated.View
              style={[styles.drawerBackdrop, { backgroundColor: 'rgba(8, 12, 18, 0.38)' }, backdropStyle]}>
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Close chats panel"
                onPress={closeDrawer}
                style={styles.backdropHitbox}
              />
            </Animated.View>
            <Animated.View
              style={[
                styles.drawerPanel,
                {
                  backgroundColor: palette.surface,
                  borderColor: palette.border,
                },
                panelStyle,
              ]}>
              <View style={[styles.drawerHeader, { borderBottomColor: palette.border }]}>
                <Text style={[styles.drawerTitle, { color: palette.text }]}>Conversations</Text>
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel="Close chats panel"
                  onPress={closeDrawer}
                  style={({ pressed }) => [
                    styles.drawerClose,
                    {
                      borderColor: palette.border,
                      backgroundColor: palette.surfaceAlt,
                      opacity: pressed ? 0.86 : 1,
                    },
                  ]}>
                  <FontAwesome name="times" size={14} color={palette.text} />
                </Pressable>
              </View>
              <View style={styles.drawerActions}>
                <Pressable
                  accessibilityRole="button"
                  disabled={!canCreateChat}
                  onPress={() => {
                    void createChat().then(closeDrawer);
                  }}
                  style={({ pressed }) => [
                    styles.actionButton,
                    styles.primaryAction,
                    {
                      backgroundColor: palette.accent,
                      borderColor: palette.accent,
                      opacity: pressed ? 0.86 : canCreateChat ? 1 : 0.58,
                    },
                  ]}>
                  <Text style={[styles.actionLabel, { color: palette.surface }]}>
                    {creatingChat ? 'Creating...' : 'New Chat'}
                  </Text>
                </Pressable>
                <Pressable
                  accessibilityRole="button"
                  disabled={!canRefreshThreads}
                  onPress={() => {
                    void refreshThreads();
                  }}
                  style={({ pressed }) => [
                    styles.actionButton,
                    {
                      backgroundColor: palette.surfaceAlt,
                      borderColor: palette.border,
                      opacity: pressed ? 0.86 : canRefreshThreads ? 1 : 0.58,
                    },
                  ]}>
                  <Text style={[styles.actionLabel, { color: palette.text }]}>
                    {loadingThreads ? 'Refreshing...' : 'Refresh'}
                  </Text>
                </Pressable>
              </View>
              <View style={styles.drawerContent}>
                <ThreadList
                  threads={threads}
                  activeChatId={activeChatId}
                  loading={loadingThreads}
                  onSelect={(chatId) => {
                    selectThread(chatId);
                    closeDrawer();
                  }}
                />
              </View>
            </Animated.View>
          </View>
        ) : null}
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
    gap: spacing.sm,
  },
  headerActions: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
  },
  eyebrow: {
    ...typography.label,
    textTransform: 'uppercase',
  },
  title: {
    ...typography.display,
  },
  drawerToggle: {
    alignItems: 'center',
    borderRadius: radius.md,
    borderWidth: 1,
    flexDirection: 'row',
    gap: spacing.xs,
    justifyContent: 'center',
    minHeight: 44,
    paddingHorizontal: spacing.md,
  },
  drawerToggleLabel: {
    ...typography.label,
    fontSize: 13,
  },
  setupCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.sm,
  },
  setupTitle: {
    ...typography.title,
    fontSize: 20,
  },
  setupBody: {
    ...typography.body,
    fontWeight: '400',
  },
  setupHint: {
    ...typography.data,
    fontSize: 12,
  },
  targetRow: {
    borderRadius: radius.md,
    borderWidth: 1,
    paddingHorizontal: spacing.sm,
    paddingVertical: spacing.xs,
  },
  targetValue: {
    ...typography.data,
  },
  actionButton: {
    borderRadius: radius.md,
    borderWidth: 1,
    minHeight: 44,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: spacing.md,
  },
  primaryAction: {
    flex: 1,
  },
  actionLabel: {
    ...typography.label,
    fontSize: 13,
  },
  errorCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    padding: spacing.sm,
  },
  errorText: {
    ...typography.body,
    fontWeight: '500',
    fontSize: 13,
  },
  chatPane: {
    flex: 1,
    minHeight: 0,
    gap: spacing.md,
  },
  drawerLayer: {
    ...StyleSheet.absoluteFillObject,
    zIndex: 20,
  },
  drawerBackdrop: {
    ...StyleSheet.absoluteFillObject,
  },
  backdropHitbox: {
    flex: 1,
  },
  drawerPanel: {
    borderRightWidth: 1,
    bottom: 0,
    left: 0,
    maxWidth: 360,
    position: 'absolute',
    top: 0,
    width: '86%',
    shadowColor: '#000000',
    shadowOpacity: 0.16,
    shadowRadius: 16,
    shadowOffset: {
      width: 2,
      height: 0,
    },
    elevation: 18,
  },
  drawerHeader: {
    alignItems: 'center',
    borderBottomWidth: 1,
    flexDirection: 'row',
    justifyContent: 'space-between',
    paddingHorizontal: spacing.md,
    paddingTop: spacing.xxl,
    paddingBottom: spacing.sm,
  },
  drawerTitle: {
    ...typography.title,
    fontSize: 19,
  },
  drawerClose: {
    alignItems: 'center',
    borderRadius: radius.md,
    borderWidth: 1,
    justifyContent: 'center',
    minHeight: 44,
    minWidth: 44,
  },
  drawerActions: {
    flexDirection: 'row',
    gap: spacing.sm,
    paddingHorizontal: spacing.md,
    paddingTop: spacing.md,
  },
  drawerContent: {
    flex: 1,
    minHeight: 0,
    paddingTop: spacing.sm,
    paddingBottom: spacing.lg,
  },
});
