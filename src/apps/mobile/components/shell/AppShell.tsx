import { useRouter } from 'expo-router';
import {
  PanelLeft,
  X,
  ChevronUp,
  Circle,
} from 'lucide-react-native';
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
  Easing,
  interpolate,
  type SharedValue,
  useAnimatedStyle,
  useSharedValue,
  withSpring,
  withTiming,
} from 'react-native-reanimated';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { ScreenSurface } from '@/components/ui/ScreenSurface';
import { useAppTheme } from '@/hooks/useAppTheme';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion, triggerMobileHaptic } from '@/theme/motion';
import { elevation, radius, spacing, typography } from '@/theme/tokens';
import {
  MOBILE_SECTION_ITEMS,
  MOBILE_SECTION_ROUTES,
  MOBILE_SECTION_TITLES,
  type MobileSection,
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

interface SectionOptionProps {
  expandedProgress: SharedValue<number>;
  index: number;
  selected: boolean;
  label: string;
  subtitle: string;
  Icon: (props: { size?: number; color?: string }) => ReactNode;
  palette: ReturnType<typeof useAppTheme>['palette'];
  onPress: () => void;
}

function SectionOption({
  expandedProgress,
  index,
  selected,
  label,
  subtitle,
  Icon,
  palette,
  onPress,
}: SectionOptionProps) {
  const optionStyle = useAnimatedStyle(() => ({
    opacity: expandedProgress.value,
    transform: [
      {
        translateY: interpolate(expandedProgress.value, [0, 1], [12 + index * 6, 0]),
      },
      {
        scale: interpolate(expandedProgress.value, [0, 1], [0.95, 1]),
      },
    ],
  }));

  return (
    <Animated.View style={optionStyle}>
      <Pressable
        accessibilityRole="button"
        accessibilityState={{ selected }}
        accessibilityLabel={`${label} section`}
        onPress={onPress}
        style={({ pressed }) => [
          styles.sectionOption,
          {
            backgroundColor: selected ? palette.surface1 : palette.surface0,
            borderColor: selected ? palette.accent : palette.border,
            opacity: pressed ? 0.86 : 1,
          },
        ]}>
        <Icon size={16} color={selected ? palette.accent : palette.textSecondary} />
        <View style={styles.sectionOptionText}>
          <Text style={[styles.sectionOptionLabel, { color: palette.text }]}>{label}</Text>
          <Text style={[styles.sectionOptionSubtitle, { color: palette.textSecondary }]}>{subtitle}</Text>
        </View>
      </Pressable>
    </Animated.View>
  );
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
  const fabProgress = useSharedValue(0);
  const { width } = useWindowDimensions();
  const insets = useSafeAreaInsets();
  const router = useRouter();
  const isTablet = width >= 600;
  const drawerWidth = isTablet ? 340 : Math.min(360, Math.round(width * 0.86));
  const edgeGestureWidth = 24;

  const [drawerOpen, setDrawerOpen] = useState(false);
  const [fabOpen, setFabOpen] = useState(false);
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

  const animateFab = useCallback(
    (expanded: boolean) => {
      const target = expanded ? 1 : 0;
      if (reducedMotion) {
        fabProgress.value = withTiming(target, { duration: 0 });
        return;
      }
      fabProgress.value = withTiming(target, {
        duration: expanded ? 220 : 180,
        easing: expanded ? Easing.out(Easing.cubic) : Easing.inOut(Easing.quad),
      });
    },
    [fabProgress, reducedMotion],
  );

  const commitDrawer = useCallback((open: boolean, velocity = 0) => {
    drawerVelocityRef.current = velocity;
    setDrawerOpen(open);
  }, []);

  const closeDrawer = useCallback(() => {
    if (isTablet) return;
    triggerMobileHaptic(motion.haptics.navSelect);
    commitDrawer(false);
  }, [commitDrawer, isTablet]);

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
    if (hasTarget || section === 'settings') return;
    router.replace(MOBILE_SECTION_ROUTES.settings);
  }, [hasTarget, router, section]);

  useEffect(() => {
    const target = isTablet || drawerOpen ? 1 : 0;
    dragProgressRef.current = target;
    const velocity = drawerVelocityRef.current;
    drawerVelocityRef.current = 0;
    animateDrawer(target ? 1 : 0, velocity);
  }, [animateDrawer, drawerOpen, isTablet]);

  useEffect(() => {
    animateFab(fabOpen);
  }, [animateFab, fabOpen]);

  useEffect(() => {
    setFabOpen(false);
  }, [section]);

  const edgeSwipeResponder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: (_, gesture) => {
          if (isTablet || drawerOpen) return false;
          return gesture.x0 <= edgeGestureWidth;
        },
        onMoveShouldSetPanResponder: (_, gesture) => {
          if (isTablet || drawerOpen) return false;
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
    [commitDrawer, drawerOpen, drawerWidth, edgeGestureWidth, isTablet, setDrawerProgress],
  );

  const panelSwipeResponder = useMemo(
    () =>
      PanResponder.create({
        onStartShouldSetPanResponder: () => false,
        onMoveShouldSetPanResponder: (_, gesture) => {
          if (isTablet || !drawerOpen) return false;
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
    [commitDrawer, drawerOpen, drawerWidth, isTablet, setDrawerProgress],
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

  const fabOptionsStyle = useAnimatedStyle(() => ({
    opacity: fabProgress.value,
    transform: [
      {
        translateY: interpolate(fabProgress.value, [0, 1], [18, 0]),
      },
    ],
  }));

  const drawerHelpers = useMemo(() => ({ closeDrawer }), [closeDrawer]);
  const sectionTitle = MOBILE_SECTION_TITLES[section];
  const activeSectionItem = useMemo(
    () => MOBILE_SECTION_ITEMS.find((item) => item.id === section) ?? MOBILE_SECTION_ITEMS[0],
    [section],
  );
  const drawerActions = renderDrawerActions?.(drawerHelpers);
  const statusLabel = hasTarget ? statusBadge.label : 'Setup';
  const statusTone = hasTarget ? statusBadge.tone : 'warning';

  const statusColor =
    statusTone === 'success' ? palette.success : statusTone === 'warning' ? palette.warning : palette.accent;

  const statusDim =
    statusTone === 'success'
      ? palette.successDim
      : statusTone === 'warning'
        ? palette.warningDim
        : palette.accentDim;

  const handleSectionSelect = useCallback(
    (nextSection: MobileSection) => {
      const route = MOBILE_SECTION_ROUTES[nextSection];
      triggerMobileHaptic(motion.haptics.navSelect);
      setFabOpen(false);
      router.replace(route);
    },
    [router],
  );

  return (
    <ScreenSurface>
      <View
        style={[
          styles.container,
          {
            backgroundColor: palette.background,
            paddingTop: insets.top + spacing.sm,
          },
        ]}>
        {loadingTarget ? (
          <View
            style={[
              styles.setupCard,
              { backgroundColor: palette.surface0, borderColor: palette.border },
            ]}>
            <Text style={[styles.setupTitle, { color: palette.text }]}>Loading target</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>Checking saved gateway configuration...</Text>
          </View>
        ) : null}

        {!loadingTarget && !hasTarget && section !== 'settings' ? (
          <View
            style={[
              styles.setupCard,
              { backgroundColor: palette.surface0, borderColor: palette.border },
            ]}>
            <Text style={[styles.setupTitle, { color: palette.text }]}>Connect your gateway</Text>
            <Text style={[styles.setupBody, { color: palette.textSecondary }]}>Open Settings from section switcher to configure target URL.</Text>
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
          <View pointerEvents="box-only" style={styles.edgeSwipeArea} {...edgeSwipeResponder.panHandlers} />
        ) : null}

        <View
          pointerEvents={!isTablet && drawerOpen ? 'auto' : isTablet ? 'auto' : 'none'}
          style={styles.drawerLayer}>
          {!isTablet ? (
            <Animated.View style={[styles.drawerBackdrop, { backgroundColor: palette.overlay }, backdropStyle]}>
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Close side panel"
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
              <Text style={[styles.detailLabel, { color: palette.textSecondary }]}>
                {DRAWER_DETAIL_LABELS[section]}
              </Text>
              <Text style={[styles.detailHint, { color: palette.textSecondary }]}>
                {DRAWER_DETAIL_HINTS[section]}
              </Text>
            </View>

            {drawerActions ? <View style={styles.drawerActions}>{drawerActions}</View> : null}

            <View style={styles.drawerContent}>{renderDrawerContent(drawerHelpers)}</View>
          </Animated.View>
        </View>

        {!isTablet ? (
          <View
            pointerEvents="box-none"
            style={[
              styles.fabLayer,
              {
                paddingBottom: Math.max(insets.bottom, spacing.sm),
              },
            ]}>
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={drawerOpen ? 'Close side panel' : 'Open side panel'}
              onPress={() => {
                triggerMobileHaptic(motion.haptics.drawerToggle);
                setDrawerOpen((current) => !current);
              }}
              style={({ pressed }) => [
                styles.panelButton,
                {
                  backgroundColor: palette.surface0,
                  borderColor: palette.border,
                  opacity: pressed ? 0.86 : 1,
                },
              ]}>
              <PanelLeft size={16} color={palette.text} />
            </Pressable>

            <Animated.View
              pointerEvents={fabOpen ? 'auto' : 'none'}
              style={[styles.sectionOptionsContainer, fabOptionsStyle]}>
              {MOBILE_SECTION_ITEMS.map((item, index) => {
                const Icon = item.icon;
                const selected = item.id === section;
                return (
                  <SectionOption
                    key={item.id}
                    expandedProgress={fabProgress}
                    index={index}
                    selected={selected}
                    label={item.label}
                    subtitle={item.subtitle}
                    Icon={Icon}
                    palette={palette}
                    onPress={() => {
                      handleSectionSelect(item.id);
                    }}
                  />
                );
              })}
            </Animated.View>

            <Pressable
              accessibilityRole="button"
              accessibilityLabel={fabOpen ? 'Collapse section menu' : 'Expand section menu'}
              onPress={() => {
                triggerMobileHaptic(motion.haptics.drawerToggle);
                setFabOpen((current) => !current);
              }}
              style={({ pressed }) => [
                styles.fabButton,
                {
                  backgroundColor: palette.surface0,
                  borderColor: palette.border,
                  opacity: pressed ? 0.86 : 1,
                },
              ]}>
              <activeSectionItem.icon size={16} color={palette.text} />
              <View style={styles.fabTextColumn}>
                <Text style={[styles.fabTitle, { color: palette.text }]}>{sectionTitle}</Text>
                <View style={styles.fabStatusRow}>
                  <Circle size={8} fill={statusColor} color={statusColor} />
                  <Text style={[styles.fabStatus, { color: statusColor }]}>{statusLabel}</Text>
                </View>
              </View>
              <View style={[styles.fabChevronWrap, { backgroundColor: statusDim, borderColor: statusColor }]}> 
                <ChevronUp
                  size={14}
                  color={statusColor}
                  style={{ transform: [{ rotate: fabOpen ? '180deg' : '0deg' }] }}
                />
              </View>
            </Pressable>
          </View>
        ) : null}
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
  fabLayer: {
    bottom: 0,
    left: 0,
    paddingHorizontal: spacing.lg,
    position: 'absolute',
    right: 0,
    zIndex: 30,
  },
  panelButton: {
    alignItems: 'center',
    borderRadius: radius.pill,
    borderWidth: 1,
    height: 42,
    justifyContent: 'center',
    marginBottom: spacing.sm,
    width: 42,
  },
  sectionOptionsContainer: {
    alignSelf: 'flex-end',
    gap: spacing.sm,
    marginBottom: spacing.sm,
    width: 220,
  },
  sectionOption: {
    alignItems: 'center',
    borderRadius: radius.md,
    borderWidth: 1,
    flexDirection: 'row',
    gap: spacing.sm,
    minHeight: 50,
    paddingHorizontal: spacing.md,
  },
  sectionOptionText: {
    flex: 1,
    gap: 2,
  },
  sectionOptionLabel: {
    ...typography.label,
    fontSize: 13,
  },
  sectionOptionSubtitle: {
    ...typography.data,
    fontSize: 11,
  },
  fabButton: {
    alignItems: 'center',
    alignSelf: 'flex-end',
    borderRadius: radius.pill,
    borderWidth: 1,
    flexDirection: 'row',
    gap: spacing.sm,
    minHeight: 56,
    paddingHorizontal: spacing.md,
    ...elevation.fab,
  },
  fabTextColumn: {
    gap: 2,
    minWidth: 108,
  },
  fabTitle: {
    ...typography.label,
    fontSize: 13,
  },
  fabStatusRow: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.xs,
  },
  fabStatus: {
    ...typography.data,
    fontSize: 11,
  },
  fabChevronWrap: {
    alignItems: 'center',
    borderRadius: radius.pill,
    borderWidth: 1,
    height: 26,
    justifyContent: 'center',
    width: 26,
  },
});
