import { useRef } from 'react';
import { Pressable, ScrollView, StyleSheet, Text, View } from 'react-native';
import Animated, {
  useAnimatedStyle,
  useReducedMotion,
  useSharedValue,
  withSpring,
} from 'react-native-reanimated';

import { useAppTheme } from '@/hooks/useAppTheme';
import { motion, triggerMobileHaptic } from '@/theme/motion';
import { spacing, touchTarget, typography } from '@/theme/tokens';

export interface SettingsSection {
  key: string;
  label: string;
}

interface SettingsSegmentedControlProps {
  sections: SettingsSection[];
  activeSection: string;
  onSectionChange: (key: string) => void;
}

export function SettingsSegmentedControl({
  sections,
  activeSection,
  onSectionChange,
}: SettingsSegmentedControlProps) {
  const { palette } = useAppTheme();
  const reducedMotion = useReducedMotion();
  const tabWidths = useRef<Record<string, number>>({});
  const tabOffsets = useRef<Record<string, number>>({});
  const indicatorX = useSharedValue(0);
  const indicatorWidth = useSharedValue(0);

  const animatedIndicatorStyle = useAnimatedStyle(() => ({
    transform: [{ translateX: indicatorX.value }],
    width: indicatorWidth.value,
  }));

  const moveIndicator = (key: string) => {
    const offset = tabOffsets.current[key];
    const width = tabWidths.current[key];
    if (offset === undefined || width === undefined) return;
    if (reducedMotion) {
      indicatorX.value = offset;
      indicatorWidth.value = width;
    } else {
      indicatorX.value = withSpring(offset, motion.spring.responsive);
      indicatorWidth.value = withSpring(width, motion.spring.responsive);
    }
  };

  const handleTabPress = (key: string) => {
    if (key === activeSection) return;
    triggerMobileHaptic(motion.haptics.settingsTabSwitch);
    onSectionChange(key);
    moveIndicator(key);
  };

  const handleTabLayout = (key: string, x: number, width: number) => {
    tabOffsets.current[key] = x;
    tabWidths.current[key] = width;
    if (key === activeSection) {
      // Snap immediately on initial layout — no spring needed
      indicatorX.value = x;
      indicatorWidth.value = width;
    }
  };

  return (
    <View
      accessible
      accessibilityRole="tablist"
      accessibilityLabel="Settings sections"
      style={[styles.container, { backgroundColor: palette.surface0, borderColor: palette.border }]}
    >
      <ScrollView
        horizontal
        showsHorizontalScrollIndicator={false}
        contentContainerStyle={styles.scrollContent}
      >
        {sections.map((section) => {
          const isActive = section.key === activeSection;
          return (
            <Pressable
              key={section.key}
              accessibilityRole="tab"
              accessibilityState={{ selected: isActive }}
              accessibilityLabel={`${section.label} tab`}
              accessibilityHint={`Switch to ${section.label} section`}
              onPress={() => handleTabPress(section.key)}
              onLayout={(e) => {
                const { x, width } = e.nativeEvent.layout;
                handleTabLayout(section.key, x, width);
              }}
              style={({ pressed }) => [
                styles.tab,
                pressed && !isActive && { opacity: 0.7 },
              ]}
            >
              <Text
                style={[
                  styles.tabLabel,
                  {
                    color: isActive ? palette.accent : palette.textSecondary,
                    fontWeight: isActive ? '600' : '500',
                  },
                ]}
              >
                {section.label}
              </Text>
            </Pressable>
          );
        })}
        <Animated.View
          style={[
            styles.indicator,
            { backgroundColor: palette.accent },
            animatedIndicatorStyle,
          ]}
        />
      </ScrollView>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    borderBottomWidth: 1,
  },
  scrollContent: {
    flexDirection: 'row',
    paddingHorizontal: spacing.sm,
    position: 'relative',
  },
  tab: {
    alignItems: 'center',
    justifyContent: 'center',
    minHeight: touchTarget.min,
    minWidth: touchTarget.min,
    paddingHorizontal: spacing.xl,
  },
  tabLabel: {
    ...typography.caption,
  },
  indicator: {
    bottom: 0,
    height: 2,
    position: 'absolute',
  },
});
