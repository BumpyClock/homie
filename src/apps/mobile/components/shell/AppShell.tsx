import FontAwesome from '@expo/vector-icons/FontAwesome';
import { useRouter } from 'expo-router';
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
  interpolate,
  useAnimatedStyle,
  useSharedValue,
  withTiming,
} from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { ScreenSurface } from '@/components/ui/ScreenSurface';
import { StatusPill } from '@/components/ui/StatusPill';
import { useAppTheme } from '@/hooks/useAppTheme';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion } from '@/theme/motion';
import { elevation, radius, spacing, typography } from '@/theme/tokens';

import {
  type MobileSection,
  MOBILE_SECTION_ROUTES,
  MOBILE_SECTION_TITLES,
  PrimarySectionMenu,
} from './PrimarySectionMenu';

interface DrawerRenderHelpers {
  closeDrawer: () => void;
}

interface AppShellProps extends PropsWithChildren {
  section: MobileSection;
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

export function AppShell({
  section,
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
  const { width } = useWindowDimensions();
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const isTablet = width >= 600;
  const drawerWidth = isTablet ? 340 : Math.min(360, Math.round(width * 0.86));
  const edgeGestureWidth = 24;

  const [drawerOpen, setDrawerOpen] = useState(false);
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

  useEffect(() => {
    if (!hasTarget) {
      setDrawerOpen(false);
      return;
    }
    if (isTablet) {
      setDrawerOpen(true);
    }
  }, [hasTarget, isTablet]);

  useEffect(() => {
    if (!hasTarget || section === 'settings') return;
    router.replace(MOBILE_SECTION_ROUTES.settings);
  }, [hasTarget, router, section]);

  useEffect(() => {
    const target = isTablet || drawerOpen ? 1 : 0;
    dragProgressRef.current = target;
    drawerProgress.value = withTiming(target, {
      duration: reducedMotion ? 0 : motion.duration.standard,
      easing: motion.easing.enter,
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
        translateX: interpolate(drawerProgress.value, [0, 1], [-drawerWidth, 0]),
      },
    ],
  }));

  const drawerHelpers = useMemo(() => ({ closeDrawer }), [closeDrawer]);
  const sectionTitle = MOBILE_SECTION_TITLES[section];
  const drawerActions = renderDrawerActions?.(drawerHelpers);
  const statusLabel = hasTarget ? statusBadge.label : 'Setup';
  const statusTone = hasTarget ? statusBadge.tone : 'warning';

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
                    backgroundColor: palette.surface0,
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
            <StatusPill label={statusLabel} tone={statusTone} />
          </View>
        </View>

        {loadingTarget ? (
          <View style={[styles.setupCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
            <Text style={[styles.setupTitle, { color: palette.text }]}>Loading target</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>
              Checking saved gateway configuration...
            </Text>
          </View>
        ) : null}

        {!loadingTarget && !hasTarget && section !== 'settings' ? (
          <View style={[styles.setupCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
            <Text style={[styles.setupTitle, { color: palette.text }]}>Connect your gateway</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>
              Open Settings from the left menu to configure target URL.
            </Text>
          </View>
        ) : null}

        {error ? (
          <View style={[styles.errorCard, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
            <Text style={[styles.errorText, { color: palette.danger }]}>{error}</Text>
          </View>
        ) : null}

        <View style={styles.contentRow}>
          <View style={styles.mainContent}>{children}</View>
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
              style={[styles.drawerBackdrop, { backgroundColor: palette.overlay }, backdropStyle]}>
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
                backgroundColor: palette.surface0,
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
                      backgroundColor: palette.surface1,
                      opacity: pressed ? 0.86 : 1,
                    },
                  ]}>
                  <FontAwesome name="times" size={14} color={palette.text} />
                </Pressable>
              ) : null}
            </View>

            <PrimarySectionMenu activeSection={section} onNavigate={closeDrawer} />

            <View style={[styles.detailHeader, { borderTopColor: palette.border }]}>
              <Text style={[styles.detailLabel, { color: palette.textSecondary }]}>Section Items</Text>
            </View>

            {drawerActions ? (
              <View style={styles.drawerActions}>
                {drawerActions}
              </View>
            ) : null}

            <View style={styles.drawerContent}>
              {renderDrawerContent(drawerHelpers)}
            </View>
          </Animated.View>
        </View>
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
    ...elevation.drawer,
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
  drawerContent: {
    flex: 1,
    minHeight: 0,
    paddingTop: spacing.sm,
    paddingBottom: spacing.lg,
  },
});
