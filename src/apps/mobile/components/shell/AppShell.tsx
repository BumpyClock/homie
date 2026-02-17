import { useRouter } from 'expo-router';
import { Menu, X } from 'lucide-react-native';
import { useCallback, useEffect, useMemo, useRef, useState, type PropsWithChildren, type ReactNode } from 'react';
import {
  PanResponder,
  Pressable,
  StyleSheet,
  Text,
  View,
  useWindowDimensions,
} from 'react-native';
import Animated, {
  FadeIn,
  interpolate,
  useAnimatedStyle,
  useSharedValue,
  withSpring,
  withTiming,
} from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { ScreenSurface } from '@/components/ui/ScreenSurface';
import { StatusPill } from '@/components/ui/StatusPill';
import { useAppTheme } from '@/hooks/useAppTheme';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion, triggerMobileHaptic } from '@/theme/motion';
import { elevation, radius, spacing, typography } from '@/theme/tokens';

type MobileSection = 'chat' | 'terminals' | 'settings';

interface DrawerRenderHelpers {
  closeDrawer: () => void;
}

interface AppShellProps extends PropsWithChildren {
  section: MobileSection;
  topBarTitle?: string;
  hasTarget: boolean;
  loadingTarget: boolean;
  error: string | null;
  statusBadge: {
    label: string;
    tone: 'accent' | 'success' | 'warning';
  };
  renderDrawerContent: (helpers: DrawerRenderHelpers) => ReactNode;
  renderDrawerActions?: (helpers: DrawerRenderHelpers) => ReactNode;
}

const DRAWER_DETAIL_LABELS: Record<MobileSection, string> = {
  chat: 'Conversations',
  terminals: 'Terminal Sessions',
  settings: 'Configuration',
};

const DRAWER_DETAIL_HINTS: Record<MobileSection, string> = {
  chat: 'Pick a thread or create a chat.',
  terminals: 'Switch active terminal session.',
  settings: 'Gateway target and app defaults.',
};

const SECTION_TITLES: Record<MobileSection, string> = {
  chat: 'Chat',
  terminals: 'Terminals',
  settings: 'Settings',
};

const PERSISTENT_DRAWER_MIN_SHORTEST_SIDE = 600;
const LARGE_SCREEN_MIN_WIDTH = 1100;
// TODO(remotely-8di.3.10): keep fixed pane in v1; future pass adds user-resizable width
// with bounded min/max, persisted preference, and reduced-motion/a11y-compliant drag affordance.

export function AppShell({
  section,
  topBarTitle,
  hasTarget,
  loadingTarget,
  error,
  statusBadge,
  renderDrawerContent,
  renderDrawerActions,
  children,
}: AppShellProps) {
  const { palette } = useAppTheme();
  const reducedMotion = useReducedMotion();
  const drawerProgress = useSharedValue(0);
  const { width, height } = useWindowDimensions();
  const insets = useSafeAreaInsets();
  const router = useRouter();

  const shortestSide = Math.min(width, height);
  const persistentDrawer = shortestSide >= PERSISTENT_DRAWER_MIN_SHORTEST_SIDE;
  const largeScreen = width >= LARGE_SCREEN_MIN_WIDTH;
  const horizontalPadding = largeScreen ? spacing.xxl : spacing.lg;
  const compactDrawerWidth = Math.min(380, Math.round(width * 0.86));
  const persistentDrawerWidth = Math.max(300, Math.min(400, Math.round(width * 0.32)));
  const drawerWidth = persistentDrawer ? persistentDrawerWidth : compactDrawerWidth;
  const edgeGestureWidth = 24;

  const [drawerOpen, setDrawerOpen] = useState(false);
  const dragProgressRef = useRef(0);
  const drawerVelocityRef = useRef(0);

  const clampProgress = useCallback((value: number) => {
    if (value <= 0) return 0;
    if (value >= 1) return 1;
    return value;
  }, []);

  const setDrawerProgress = useCallback(
    (value: number) => {
      const next = clampProgress(value);
      dragProgressRef.current = next;
      drawerProgress.value = next;
    },
    [clampProgress, drawerProgress],
  );

  const animateDrawer = useCallback(
    (target: 0 | 1, velocity = 0) => {
      if (reducedMotion) {
        drawerProgress.value = withTiming(target, { duration: 0 });
        return;
      }
      drawerProgress.value = withSpring(target, {
        ...motion.spring.drawer,
        velocity,
      });
    },
    [drawerProgress, reducedMotion],
  );

  const commitDrawer = useCallback((open: boolean, velocity = 0) => {
    drawerVelocityRef.current = velocity;
    setDrawerOpen(open);
  }, []);

  const closeDrawer = useCallback(() => {
    if (persistentDrawer) return;
    triggerMobileHaptic(motion.haptics.navSelect);
    commitDrawer(false);
  }, [commitDrawer, persistentDrawer]);

  const toggleDrawer = useCallback(() => {
    if (persistentDrawer) return;
    triggerMobileHaptic(motion.haptics.drawerToggle);
    setDrawerOpen((current) => !current);
  }, [persistentDrawer]);

  useEffect(() => {
    if (!hasTarget) {
      setDrawerOpen(false);
      return;
    }
    if (persistentDrawer) {
      setDrawerOpen(true);
    }
  }, [hasTarget, persistentDrawer]);

  useEffect(() => {
    if (hasTarget || section === 'settings') return;
    router.replace('/(tabs)/settings');
  }, [hasTarget, router, section]);

  useEffect(() => {
    if (persistentDrawer) {
      dragProgressRef.current = 1;
      drawerProgress.value = withTiming(1, { duration: 0 });
      return;
    }
    const target = drawerOpen ? 1 : 0;
    dragProgressRef.current = target;
    const velocity = drawerVelocityRef.current;
    drawerVelocityRef.current = 0;
    animateDrawer(target ? 1 : 0, velocity);
  }, [animateDrawer, drawerOpen, drawerProgress, persistentDrawer]);

  const edgeSwipeResponder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: (_, gesture) => {
          if (persistentDrawer || drawerOpen) return false;
          return gesture.x0 <= edgeGestureWidth;
        },
        onMoveShouldSetPanResponder: (_, gesture) => {
          if (persistentDrawer || drawerOpen) return false;
          const horizontal = Math.abs(gesture.dx) > Math.abs(gesture.dy) * 1.5;
          return horizontal && gesture.x0 <= edgeGestureWidth && Math.abs(gesture.dx) > 8;
        },
        onPanResponderGrant: () => {
          setDrawerProgress(0);
        },
        onPanResponderMove: (_, gesture) => {
          setDrawerProgress(gesture.dx / drawerWidth);
        },
        onPanResponderRelease: (_, gesture) => {
          const open = gesture.vx > 0.3 || dragProgressRef.current > 0.35;
          const velocity = (gesture.vx * 1000) / drawerWidth;
          if (open !== drawerOpen) {
            triggerMobileHaptic(motion.haptics.drawerSnap);
          }
          commitDrawer(open, velocity);
        },
        onPanResponderTerminate: () => {
          commitDrawer(false);
        },
      }),
    [commitDrawer, drawerOpen, drawerWidth, edgeGestureWidth, persistentDrawer, setDrawerProgress],
  );

  const panelSwipeResponder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: () => false,
        onMoveShouldSetPanResponder: (_, gesture) => {
          if (persistentDrawer || !drawerOpen) return false;
          const horizontal = Math.abs(gesture.dx) > Math.abs(gesture.dy) * 1.5;
          return horizontal && Math.abs(gesture.dx) > 8;
        },
        onPanResponderGrant: () => {
          setDrawerProgress(1);
        },
        onPanResponderMove: (_, gesture) => {
          setDrawerProgress(1 + gesture.dx / drawerWidth);
        },
        onPanResponderRelease: (_, gesture) => {
          const stayOpen = gesture.vx > -0.3 && dragProgressRef.current > 0.65;
          const velocity = (gesture.vx * 1000) / drawerWidth;
          if (stayOpen !== drawerOpen) {
            triggerMobileHaptic(motion.haptics.drawerSnap);
          }
          commitDrawer(stayOpen, velocity);
        },
        onPanResponderTerminate: () => {
          commitDrawer(true);
        },
      }),
    [commitDrawer, drawerOpen, drawerWidth, persistentDrawer, setDrawerProgress],
  );

  const backdropStyle = useAnimatedStyle(() => ({
    opacity: interpolate(drawerProgress.value, [0, 1], [0, 0.45]),
  }));

  const panelStyle = useAnimatedStyle(() => ({
    transform: [
      {
        translateX: interpolate(drawerProgress.value, [0, 1], [-drawerWidth, 0]),
      },
    ],
  }));

  const drawerHelpers = useMemo(() => ({ closeDrawer }), [closeDrawer]);
  const sectionTitle = topBarTitle?.trim() || SECTION_TITLES[section];
  const drawerActions = renderDrawerActions?.(drawerHelpers);
  const statusLabel = hasTarget ? statusBadge.label : 'Setup';
  const statusTone = hasTarget ? statusBadge.tone : 'warning';
  const drawerHeaderTopPadding = persistentDrawer ? spacing.xl : spacing.xxl;

  const drawerPanelContent = (
    <>
      <View style={[styles.drawerHeader, { borderBottomColor: palette.border, paddingTop: drawerHeaderTopPadding }]}> 
        <Text style={[styles.drawerTitle, { color: palette.text }]}>Homie</Text>
        <StatusPill compact label={statusLabel} tone={statusTone} />
        {!persistentDrawer ? (
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Close side panel"
            onPress={closeDrawer}
            style={({ pressed }) => [
              styles.drawerClose,
              {
                borderColor: palette.border,
                backgroundColor: palette.surface1,
                opacity: pressed ? 0.86 : 1,
              },
            ]}>
            <X size={14} color={palette.text} />
          </Pressable>
        ) : null}
      </View>

      <View style={[styles.detailHeader, { borderTopColor: palette.border }]}> 
        <Text style={[styles.detailLabel, { color: palette.textSecondary }]}> {DRAWER_DETAIL_LABELS[section]} </Text>
        <Text style={[styles.detailHint, { color: palette.textSecondary }]}> {DRAWER_DETAIL_HINTS[section]} </Text>
      </View>

      {drawerActions ? <View style={styles.drawerActions}>{drawerActions}</View> : null}

      <Animated.View
        key={section}
        entering={reducedMotion ? undefined : FadeIn.duration(motion.duration.fast)}
        style={styles.drawerContent}
      >
        {renderDrawerContent(drawerHelpers)}
      </Animated.View>
    </>
  );

  return (
    <ScreenSurface>
      <View
        style={[
          styles.container,
          {
            backgroundColor: palette.background,
            paddingTop: insets.top + spacing.sm,
            paddingHorizontal: horizontalPadding,
          },
        ]}>
        {!persistentDrawer ? (
          <View style={styles.topBar}>
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={drawerOpen ? 'Close side panel' : 'Open side panel'}
              accessibilityState={{ expanded: drawerOpen }}
              accessibilityHint="Toggles the navigation drawer"
              onPress={toggleDrawer}
              style={({ pressed }) => [
                styles.menuButton,
                {
                  backgroundColor: palette.surface0,
                  borderColor: palette.border,
                  opacity: pressed ? 0.86 : 1,
                },
              ]}>
              <Menu size={16} color={palette.text} />
            </Pressable>
            <Text numberOfLines={1} style={[styles.topBarTitle, { color: palette.text }]}>{sectionTitle}</Text>
          </View>
        ) : null}

        {loadingTarget ? (
          <View style={[styles.setupCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}> 
            <Text style={[styles.setupTitle, { color: palette.text }]}>Loading target</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>Checking saved gateway configuration...</Text>
          </View>
        ) : null}

        {!loadingTarget && !hasTarget && section !== 'settings' ? (
          <View style={[styles.setupCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}> 
            <Text style={[styles.setupTitle, { color: palette.text }]}>Connect your gateway</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>Open Settings from the menu to configure target URL.</Text>
          </View>
        ) : null}

        {error ? (
          <View style={[styles.errorCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}> 
            <Text style={[styles.errorText, { color: palette.danger }]}>{error}</Text>
          </View>
        ) : null}

        <View style={[styles.contentRow, persistentDrawer ? styles.contentRowPersistent : null]}>
          {persistentDrawer ? (
            <View style={[styles.persistentDrawerSlot, { width: drawerWidth }]}> 
              <View
                style={[
                  styles.persistentDrawerPanel,
                  {
                    backgroundColor: palette.surface0,
                    borderColor: palette.border,
                  },
                ]}>
                {drawerPanelContent}
              </View>
            </View>
          ) : null}

          <View style={[styles.mainContent, persistentDrawer ? styles.mainContentPersistent : null]}>{children}</View>
        </View>

        {!persistentDrawer && !drawerOpen ? (
          <View pointerEvents="box-only" style={styles.edgeSwipeArea} {...edgeSwipeResponder.panHandlers} />
        ) : null}

        {!persistentDrawer ? (
          <View pointerEvents={drawerOpen ? 'auto' : 'none'} style={styles.drawerLayer}>
            <Animated.View accessibilityRole="none" style={[styles.drawerBackdrop, { backgroundColor: palette.overlay }, backdropStyle]}>
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Close side panel"
                onPress={closeDrawer}
                style={styles.backdropHitbox}
              />
            </Animated.View>

            <Animated.View
              accessibilityViewIsModal={true}
              {...panelSwipeResponder.panHandlers}
              style={[
                styles.overlayDrawerPanel,
                {
                  backgroundColor: palette.surface0,
                  borderColor: palette.border,
                  width: drawerWidth,
                },
                panelStyle,
              ]}>
              {drawerPanelContent}
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
    gap: spacing.lg,
  },
  topBar: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'flex-start',
    gap: spacing.sm,
  },
  menuButton: {
    alignItems: 'center',
    borderRadius: radius.pill,
    borderWidth: 1,
    height: 40,
    justifyContent: 'center',
    width: 40,
  },
  topBarTitle: {
    ...typography.label,
    fontSize: 14,
    flex: 1,
    textAlign: 'left',
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
  contentRowPersistent: {
    flexDirection: 'row',
    gap: spacing.lg,
  },
  persistentDrawerSlot: {
    flexShrink: 0,
    minHeight: 0,
  },
  mainContent: {
    flex: 1,
    minHeight: 0,
  },
  mainContentPersistent: {
    minWidth: 0,
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
  overlayDrawerPanel: {
    borderRightWidth: 1,
    bottom: 0,
    left: 0,
    maxWidth: 420,
    position: 'absolute',
    top: 0,
    ...elevation.drawer,
  },
  persistentDrawerPanel: {
    flex: 1,
    minHeight: 0,
    width: '100%',
    borderRadius: radius.lg,
    borderWidth: 1,
    borderRightWidth: 1,
    maxWidth: 420,
    shadowOpacity: 0,
    elevation: 0,
    overflow: 'hidden',
  },
  drawerHeader: {
    alignItems: 'center',
    borderBottomWidth: 1,
    flexDirection: 'row',
    gap: spacing.xs,
    justifyContent: 'space-between',
    paddingHorizontal: spacing.md,
    paddingBottom: spacing.sm,
  },
  drawerTitle: {
    ...typography.title,
    fontSize: 19,
    flex: 1,
  },
  drawerClose: {
    alignItems: 'center',
    borderRadius: radius.md,
    borderWidth: 1,
    justifyContent: 'center',
    minHeight: 40,
    minWidth: 40,
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
  detailHint: {
    ...typography.data,
    fontSize: 12,
    marginTop: 2,
  },
  drawerActions: {
    flexDirection: 'row',
    gap: spacing.sm,
    paddingHorizontal: spacing.md,
    paddingTop: spacing.sm,
  },
  drawerContent: {
    flex: 1,
    minHeight: 0,
    paddingTop: spacing.sm,
    paddingBottom: spacing.lg,
  },
});
