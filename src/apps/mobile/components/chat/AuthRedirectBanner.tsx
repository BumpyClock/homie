// ABOUTME: Inline banner for chat surfaces when provider authorization is required.
// ABOUTME: Redirects users to Settings; uses warning tones, accessible touch targets, and Reanimated for enter/exit.

import { AlertTriangle, ChevronRight } from 'lucide-react-native';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import Animated, { FadeIn, FadeOut, useReducedMotion } from 'react-native-reanimated';
import { useRouter } from 'expo-router';
import { AUTH_COPY } from '@homie/shared';

import { useAppTheme } from '@/hooks/useAppTheme';
import { motion } from '@/theme/motion';
import { radius, spacing, touchTarget, typography } from '@/theme/tokens';

interface AuthRedirectBannerProps {
  /** Controls visibility/animation */
  visible: boolean;
  /** Primary message text */
  message: string;
  /** Button text (default: "Open Settings") */
  actionLabel?: string;
  /** Optional override for navigation; defaults to Settings > Providers */
  onAction?: () => void;
}

/**
 * Inline banner for chat surfaces that appears when a provider is unauthorized.
 * Redirects users to Settings for provider authentication.
 *
 * Accessibility:
 * - accessibilityRole="alert" + accessibilityLiveRegion="polite"
 * - 44px minimum touch target on action button
 * - Keyboard navigable (VoiceOver / TalkBack)
 * - No layout shift: uses conditional rendering with Reanimated fade
 */
export function AuthRedirectBanner({
  visible,
  message,
  actionLabel = AUTH_COPY.bannerActionMobile,
  onAction,
}: AuthRedirectBannerProps) {
  const { palette } = useAppTheme();
  const reducedMotion = useReducedMotion();
  const router = useRouter();

  if (!visible) return null;

  const handleAction = () => {
    if (onAction) {
      onAction();
    } else {
      // Navigate to Settings tab with Providers section active
      router.push('/settings');
    }
  };

  const entering = reducedMotion
    ? undefined
    : FadeIn.duration(motion.duration.fast);
  const exiting = reducedMotion
    ? undefined
    : FadeOut.duration(motion.duration.micro);

  return (
    <Animated.View
      entering={entering}
      exiting={exiting}
      accessible
      accessibilityRole="alert"
      accessibilityLiveRegion="polite"
      accessibilityLabel={message}
      style={[
        styles.container,
        {
          backgroundColor: palette.warningDim,
          borderColor: palette.warning + '4D', // 30% opacity
        },
      ]}
    >
      <AlertTriangle
        size={16}
        color={palette.warning}
        accessibilityElementsHidden
        importantForAccessibility="no"
      />
      <Text
        style={[styles.message, { color: palette.text }]}
        numberOfLines={2}
      >
        {message}
      </Text>
      <Pressable
        onPress={handleAction}
        accessibilityRole="button"
        accessibilityLabel={`${actionLabel} to sign in to provider`}
        accessibilityHint="Opens the Settings screen where you can authorize providers"
        style={({ pressed }) => [
          styles.actionButton,
          pressed && { opacity: 0.9 },
        ]}
      >
        <Text style={[styles.actionLabel, { color: palette.warning }]}>
          {actionLabel}
        </Text>
        <ChevronRight
          size={14}
          color={palette.warning}
          accessibilityElementsHidden
          importantForAccessibility="no"
        />
      </Pressable>
    </Animated.View>
  );
}

const styles = StyleSheet.create({
  container: {
    alignItems: 'center',
    borderRadius: radius.sm,
    borderWidth: 1,
    flexDirection: 'row',
    gap: spacing.md,
    marginHorizontal: spacing.xs,
    marginBottom: spacing.sm,
    paddingHorizontal: spacing.lg,
    paddingVertical: spacing.md,
  },
  message: {
    ...typography.caption,
    flex: 1,
  },
  actionButton: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.xs,
    minHeight: touchTarget.min,
    justifyContent: 'center',
    paddingHorizontal: spacing.sm,
  },
  actionLabel: {
    ...typography.label,
    fontSize: 12,
  },
});
