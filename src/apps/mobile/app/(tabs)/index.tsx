import FontAwesome from '@expo/vector-icons/FontAwesome';
import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import {
  PanResponder,
  Pressable,
  StyleSheet,
  Text,
  View,
  useWindowDimensions,
} from 'react-native';
import Animated, {
  interpolate,
  useAnimatedStyle,
  useSharedValue,
  withTiming,
} from 'react-native-reanimated';
import { KeyboardStickyView } from 'react-native-keyboard-controller';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { ChatComposer } from '@/components/chat/ChatComposer';
import { ChatTimeline } from '@/components/chat/ChatTimeline';
import { ThreadList } from '@/components/chat/ThreadList';
import { GatewayTargetForm } from '@/components/gateway/GatewayTargetForm';
import {
  type MobileSection,
  PrimarySectionMenu,
} from '@/components/shell/PrimarySectionMenu';
import { TerminalSessionList } from '@/components/shell/TerminalSessionList';
import { ThreadActionSheet } from '@/components/shell/ThreadActionSheet';
import { ScreenSurface } from '@/components/ui/ScreenSurface';
import { StatusPill } from '@/components/ui/StatusPill';
import { useAppTheme } from '@/hooks/useAppTheme';
import { useGatewayChat } from '@/hooks/useGatewayChat';
import { useGatewayTarget } from '@/hooks/useGatewayTarget';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion } from '@/theme/motion';
import { radius, spacing, typography } from '@/theme/tokens';

type SettingRowProps = {
  label: string;
  value: string;
};

function SettingRow({ label, value }: SettingRowProps) {
  const { palette } = useAppTheme();

  return (
    <View style={[styles.settingRow, { borderColor: palette.border }]}> 
      <Text style={[styles.settingLabel, { color: palette.textSecondary }]}>{label}</Text>
      <Text style={[styles.settingValue, { color: palette.text }]}>{value}</Text>
    </View>
  );
}

export default function ChatTabScreen() {
  const { palette, mode } = useAppTheme();
  const reducedMotion = useReducedMotion();
  const drawerProgress = useSharedValue(0);
  const { width } = useWindowDimensions();
  const insets = useSafeAreaInsets();
  const isTablet = width >= 600;
  const drawerWidth = isTablet ? 340 : Math.min(360, Math.round(width * 0.86));
  const edgeGestureWidth = 24;

  const [savingTarget, setSavingTarget] = useState(false);
  const [drawerOpen, setDrawerOpen] = useState(false);
  const [section, setSection] = useState<MobileSection>('chat');
  const [activeTerminalSessionId, setActiveTerminalSessionId] = useState<string | null>(null);
  const [actioningThreadId, setActioningThreadId] = useState<string | null>(null);
  const [busyThreadAction, setBusyThreadAction] = useState(false);

  const {
    loading: loadingTarget,
    targetUrl,
    hasTarget,
    targetHint,
    error: targetError,
    saveTarget,
    clearTarget,
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
    loadingTerminals,
    terminalSessions,
    models,
    selectedModel,
    selectedEffort,
    setSelectedModel,
    setSelectedEffort,
    selectThread,
    refreshThreads,
    refreshTerminals,
    createChat,
    sendMessage,
    renameThread,
    archiveThread,
    respondApproval,
  } = useGatewayChat(targetUrl ?? '');

  const actioningThread = useMemo(
    () => threads.find((thread) => thread.chatId === actioningThreadId) ?? null,
    [actioningThreadId, threads],
  );

  const activeTerminalSession = useMemo(
    () => terminalSessions.find((session) => session.session_id === activeTerminalSessionId) ?? null,
    [activeTerminalSessionId, terminalSessions],
  );

  const canCreateChat = hasTarget && status === 'connected' && !creatingChat;
  const canRefreshThreads = hasTarget && status === 'connected' && !loadingThreads;
  const canRefreshTerminals = hasTarget && status === 'connected' && !loadingTerminals;
  const dragProgressRef = useRef(0);

  const clampProgress = useCallback((value: number) => {
    if (value <= 0) return 0;
    if (value >= 1) return 1;
    return value;
  }, []);

  const setDrawerProgress = useCallback((value: number) => {
    const next = clampProgress(value);
    dragProgressRef.current = next;
    drawerProgress.value = next;
  }, [clampProgress, drawerProgress]);

  const closeDrawer = useCallback(() => {
    if (!isTablet) setDrawerOpen(false);
  }, [isTablet]);

  const handleSaveTarget = async (value: string) => {
    setSavingTarget(true);
    try {
      await saveTarget(value);
    } finally {
      setSavingTarget(false);
    }
  };

  const handleClearTarget = async () => {
    setSavingTarget(true);
    try {
      await clearTarget();
      setSection('chat');
      setDrawerOpen(false);
    } finally {
      setSavingTarget(false);
    }
  };

  useEffect(() => {
    if (!hasTarget) {
      setDrawerOpen(false);
      setActiveTerminalSessionId(null);
      setActioningThreadId(null);
      setSection('settings');
      return;
    }
    if (isTablet) {
      setDrawerOpen(true);
    }
  }, [hasTarget, isTablet]);

  useEffect(() => {
    if (!hasTarget || section !== 'terminals' || status !== 'connected') return;
    void refreshTerminals();
  }, [hasTarget, refreshTerminals, section, status]);

  useEffect(() => {
    if (section !== 'terminals') {
      setActiveTerminalSessionId(null);
      return;
    }
    if (!activeTerminalSessionId && terminalSessions[0]) {
      setActiveTerminalSessionId(terminalSessions[0].session_id);
    }
  }, [activeTerminalSessionId, section, terminalSessions]);

  useEffect(() => {
    const target = isTablet || drawerOpen ? 1 : 0;
    dragProgressRef.current = target;
    drawerProgress.value = withTiming(target, {
      duration: reducedMotion ? 0 : motion.duration.regular,
      easing: motion.easing.enterExit,
    });
  }, [drawerOpen, drawerProgress, isTablet, reducedMotion]);

  const edgeSwipeResponder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: (_, gesture) => {
          if (isTablet || drawerOpen) return false;
          return gesture.x0 <= edgeGestureWidth;
        },
        onMoveShouldSetPanResponder: (_, gesture) => {
          if (isTablet || drawerOpen) return false;
          const horizontal = Math.abs(gesture.dx) > Math.abs(gesture.dy);
          return horizontal && gesture.x0 <= edgeGestureWidth && Math.abs(gesture.dx) > 8;
        },
        onPanResponderGrant: () => {
          setDrawerProgress(0);
        },
        onPanResponderMove: (_, gesture) => {
          setDrawerProgress(gesture.dx / drawerWidth);
        },
        onPanResponderRelease: (_, gesture) => {
          const open = gesture.vx > 0.12 || dragProgressRef.current > 0.35;
          setDrawerOpen(open);
        },
        onPanResponderTerminate: () => {
          setDrawerOpen(false);
        },
      }),
    [drawerOpen, drawerWidth, edgeGestureWidth, isTablet, setDrawerProgress],
  );

  const panelSwipeResponder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: () => false,
        onMoveShouldSetPanResponder: (_, gesture) => {
          if (isTablet || !drawerOpen) return false;
          const horizontal = Math.abs(gesture.dx) > Math.abs(gesture.dy);
          return horizontal && Math.abs(gesture.dx) > 8;
        },
        onPanResponderGrant: () => {
          setDrawerProgress(1);
        },
        onPanResponderMove: (_, gesture) => {
          setDrawerProgress(1 + gesture.dx / drawerWidth);
        },
        onPanResponderRelease: (_, gesture) => {
          const stayOpen = gesture.vx > -0.12 && dragProgressRef.current > 0.65;
          setDrawerOpen(stayOpen);
        },
        onPanResponderTerminate: () => {
          setDrawerOpen(true);
        },
      }),
    [drawerOpen, drawerWidth, isTablet, setDrawerProgress],
  );

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

  const sectionTitle =
    section === 'chat' ? 'Chat' : section === 'terminals' ? 'Terminals' : 'Settings';

  const renderSectionContent = () => {
    if (section === 'chat') {
      return (
        <View style={styles.chatPane}>
          <ChatTimeline
            thread={hasTarget ? activeThread : null}
            loading={loadingMessages && hasTarget}
            onApprovalDecision={respondApproval}
          />
          <KeyboardStickyView offset={{ closed: 0, opened: -insets.bottom }}>
            <ChatComposer
              disabled={status !== 'connected' || !activeThread || !hasTarget}
              sending={sendingMessage}
              bottomInset={insets.bottom}
              models={models}
              selectedModel={selectedModel}
              selectedEffort={selectedEffort}
              onSelectModel={setSelectedModel}
              onSelectEffort={setSelectedEffort}
              onSend={sendMessage}
            />
          </KeyboardStickyView>
        </View>
      );
    }

    if (section === 'terminals') {
      return (
        <View style={[styles.sectionCard, { backgroundColor: palette.surface, borderColor: palette.border }]}> 
          <Text style={[styles.sectionTitle, { color: palette.text }]}>Terminal Session</Text>
          {activeTerminalSession ? (
            <>
              <SettingRow label="Name" value={activeTerminalSession.name || activeTerminalSession.shell} />
              <SettingRow
                label="Resolution"
                value={`${activeTerminalSession.cols} x ${activeTerminalSession.rows}`}
              />
              <SettingRow label="Status" value={activeTerminalSession.status} />
              <Text style={[styles.meta, { color: palette.textSecondary }]}>Terminal rendering ships in the next milestone.</Text>
            </>
          ) : (
            <Text style={[styles.meta, { color: palette.textSecondary }]}>Pick a terminal from the left list.</Text>
          )}
        </View>
      );
    }

    return (
      <View style={[styles.sectionCard, { backgroundColor: palette.surface, borderColor: palette.border }]}> 
        <Text style={[styles.sectionTitle, { color: palette.text }]}>Gateway Settings</Text>
        <SettingRow label="Target" value={targetUrl ?? 'Not set'} />
        <SettingRow label="Theme" value={mode} />
        <GatewayTargetForm
          initialValue={targetUrl ?? targetHint}
          hintValue={targetHint}
          saving={savingTarget || loadingTarget}
          saveLabel={hasTarget ? 'Update Target' : 'Save Target'}
          onSave={handleSaveTarget}
          onClear={hasTarget ? handleClearTarget : undefined}
        />
        {targetError ? <Text style={[styles.meta, { color: palette.textSecondary }]}>{targetError}</Text> : null}
      </View>
    );
  };

  return (
    <ScreenSurface>
      <View style={[styles.container, { backgroundColor: palette.background, paddingTop: insets.top + spacing.sm }]}> 
        <View style={styles.headerRow}>
          <View>
            <Text style={[styles.eyebrow, { color: palette.textSecondary }]}>Gateway</Text>
            <Text style={[styles.title, { color: palette.text }]}>{sectionTitle}</Text>
          </View>
          <View style={styles.headerActions}>
            {!isTablet ? (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Toggle app menu"
                onPress={() => {
                  setDrawerOpen((current) => !current);
                }}
                style={({ pressed }) => [
                  styles.drawerToggle,
                  {
                    backgroundColor: palette.surface,
                    borderColor: palette.border,
                    opacity: pressed ? 0.86 : 1,
                  },
                ]}>
                <FontAwesome
                  name={drawerOpen ? 'times' : 'bars'}
                  size={14}
                  color={palette.text}
                />
                <Text style={[styles.drawerToggleLabel, { color: palette.text }]}>Menu</Text>
              </Pressable>
            ) : null}
            <StatusPill
              label={hasTarget ? statusBadge.label : 'Setup'}
              tone={hasTarget ? statusBadge.tone : 'warning'}
            />
          </View>
        </View>

        {loadingTarget ? (
          <View style={[styles.setupCard, { backgroundColor: palette.surface, borderColor: palette.border }]}> 
            <Text style={[styles.setupTitle, { color: palette.text }]}>Loading target</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>Checking saved gateway configuration...</Text>
          </View>
        ) : null}

        {!loadingTarget && !hasTarget && section !== 'settings' ? (
          <View style={[styles.setupCard, { backgroundColor: palette.surface, borderColor: palette.border }]}> 
            <Text style={[styles.setupTitle, { color: palette.text }]}>Connect your gateway</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>Open Settings from the left menu to configure target URL.</Text>
          </View>
        ) : null}

        {error ? (
          <View style={[styles.errorCard, { backgroundColor: palette.surface, borderColor: palette.border }]}> 
            <Text style={[styles.errorText, { color: palette.danger }]}>{error}</Text>
          </View>
        ) : null}

        <View style={styles.contentRow}>
          <View style={styles.mainContent}>{renderSectionContent()}</View>
        </View>

        {!isTablet && !drawerOpen ? (
          <View
            pointerEvents="box-only"
            style={styles.edgeSwipeArea}
            {...edgeSwipeResponder.panHandlers}
          />
        ) : null}

        <View pointerEvents={!isTablet && drawerOpen ? 'auto' : isTablet ? 'auto' : 'none'} style={styles.drawerLayer}>
          {!isTablet ? (
            <Animated.View
              style={[styles.drawerBackdrop, { backgroundColor: 'rgba(8, 12, 18, 0.38)' }, backdropStyle]}>
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Close menu"
                onPress={closeDrawer}
                style={styles.backdropHitbox}
              />
            </Animated.View>
          ) : null}

          <Animated.View
            {...(!isTablet ? panelSwipeResponder.panHandlers : {})}
            style={[
              styles.drawerPanel,
              isTablet ? styles.tabletDrawer : null,
              {
                backgroundColor: palette.surface,
                borderColor: palette.border,
                width: isTablet ? 340 : '86%',
              },
              isTablet ? undefined : panelStyle,
            ]}>
            <View style={[styles.drawerHeader, { borderBottomColor: palette.border }]}> 
              <Text style={[styles.drawerTitle, { color: palette.text }]}>Homie</Text>
              {!isTablet ? (
                <Pressable
                  accessibilityRole="button"
                  accessibilityLabel="Close menu"
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
              ) : null}
            </View>

            <PrimarySectionMenu
              activeSection={section}
              onSelect={(nextSection) => {
                setSection(nextSection);
                closeDrawer();
              }}
            />

            <View style={[styles.detailHeader, { borderTopColor: palette.border }]}> 
              <Text style={[styles.detailLabel, { color: palette.textSecondary }]}>Section Items</Text>
            </View>

            <View style={styles.drawerActions}>
              {section === 'chat' ? (
                <>
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
                </>
              ) : section === 'terminals' ? (
                <Pressable
                  accessibilityRole="button"
                  disabled={!canRefreshTerminals}
                  onPress={() => {
                    void refreshTerminals();
                  }}
                  style={({ pressed }) => [
                    styles.actionButton,
                    {
                      backgroundColor: palette.surfaceAlt,
                      borderColor: palette.border,
                      opacity: pressed ? 0.86 : canRefreshTerminals ? 1 : 0.58,
                    },
                  ]}>
                  <Text style={[styles.actionLabel, { color: palette.text }]}> 
                    {loadingTerminals ? 'Refreshing...' : 'Refresh Sessions'}
                  </Text>
                </Pressable>
              ) : null}
            </View>

            <View style={styles.drawerContent}>
              {section === 'chat' ? (
                <ThreadList
                  threads={threads}
                  activeChatId={activeChatId}
                  loading={loadingThreads}
                  onSelect={(chatId) => {
                    selectThread(chatId);
                    closeDrawer();
                  }}
                  onLongPressThread={(thread) => {
                    setActioningThreadId(thread.chatId);
                  }}
                />
              ) : section === 'terminals' ? (
                <TerminalSessionList
                  sessions={terminalSessions}
                  loading={loadingTerminals}
                  activeSessionId={activeTerminalSessionId}
                  onSelect={(sessionId) => {
                    setActiveTerminalSessionId(sessionId);
                    closeDrawer();
                  }}
                />
              ) : (
                <View style={[styles.emptySection, { borderColor: palette.border, backgroundColor: palette.surfaceAlt }]}> 
                  <Text style={[styles.emptySectionText, { color: palette.textSecondary }]}>No nested items for Settings.</Text>
                </View>
              )}
            </View>
          </Animated.View>
        </View>

        <ThreadActionSheet
          thread={actioningThread}
          busy={busyThreadAction}
          onClose={() => {
            if (busyThreadAction) return;
            setActioningThreadId(null);
          }}
          onRename={async (title) => {
            if (!actioningThread) return;
            setBusyThreadAction(true);
            try {
              await renameThread(actioningThread.chatId, title);
              setActioningThreadId(null);
            } finally {
              setBusyThreadAction(false);
            }
          }}
          onArchive={async () => {
            if (!actioningThread) return;
            setBusyThreadAction(true);
            try {
              await archiveThread(actioningThread.chatId);
              setActioningThreadId(null);
            } finally {
              setBusyThreadAction(false);
            }
          }}
        />
      </View>
    </ScreenSurface>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
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
  contentRow: {
    flex: 1,
    minHeight: 0,
  },
  mainContent: {
    flex: 1,
    minHeight: 0,
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
  },
  drawerLayer: {
    ...StyleSheet.absoluteFillObject,
    zIndex: 20,
  },
  edgeSwipeArea: {
    bottom: 0,
    left: 0,
    position: 'absolute',
    top: 0,
    width: 28,
    zIndex: 22,
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
    shadowColor: '#000000',
    shadowOpacity: 0.16,
    shadowRadius: 16,
    shadowOffset: {
      width: 2,
      height: 0,
    },
    elevation: 18,
  },
  tabletDrawer: {
    position: 'relative',
    shadowOpacity: 0,
    elevation: 0,
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
  detailHeader: {
    borderTopWidth: 1,
    marginTop: spacing.sm,
    paddingHorizontal: spacing.md,
    paddingTop: spacing.sm,
  },
  detailLabel: {
    ...typography.label,
    fontSize: 11,
    textTransform: 'uppercase',
  },
  drawerActions: {
    flexDirection: 'row',
    gap: spacing.sm,
    paddingHorizontal: spacing.md,
    paddingTop: spacing.sm,
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
  drawerContent: {
    flex: 1,
    minHeight: 0,
    paddingTop: spacing.sm,
    paddingBottom: spacing.lg,
  },
  emptySection: {
    borderRadius: radius.md,
    borderWidth: 1,
    marginHorizontal: spacing.md,
    marginTop: spacing.sm,
    minHeight: 88,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: spacing.md,
  },
  emptySectionText: {
    ...typography.body,
    fontSize: 13,
    fontWeight: '400',
    textAlign: 'center',
  },
  sectionCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.md,
  },
  sectionTitle: {
    ...typography.title,
  },
  settingRow: {
    borderBottomWidth: 1,
    paddingVertical: spacing.sm,
    gap: spacing.xs,
  },
  settingLabel: {
    ...typography.label,
    textTransform: 'uppercase',
  },
  settingValue: {
    ...typography.data,
  },
  meta: {
    ...typography.body,
    fontWeight: '400',
    fontSize: 13,
  },
});
