import * as Clipboard from 'expo-clipboard';
import { useCallback, useEffect, useRef, useState } from 'react';
import { Linking, Pressable, StyleSheet, Text, View } from 'react-native';
import Animated, {
  FadeIn,
  useReducedMotion,
} from 'react-native-reanimated';

import { useAppTheme } from '@/hooks/useAppTheme';
import { motion } from '@/theme/motion';
import { radius, spacing, touchTarget, typography } from '@/theme/tokens';

interface DeviceCodeCardProps {
  verificationUrl: string;
  userCode: string;
}

const COPIED_RESET_MS = 2_000;

export function DeviceCodeCard({ verificationUrl, userCode }: DeviceCodeCardProps) {
  const { palette } = useAppTheme();
  const reduceMotion = useReducedMotion();
  const [copied, setCopied] = useState(false);
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    return () => {
      if (timerRef.current) clearTimeout(timerRef.current);
    };
  }, []);

  const handleCopy = useCallback(async () => {
    await Clipboard.setStringAsync(userCode);
    setCopied(true);
    if (timerRef.current) clearTimeout(timerRef.current);
    timerRef.current = setTimeout(() => setCopied(false), COPIED_RESET_MS);
  }, [userCode]);

  const handleOpenUrl = useCallback(() => {
    void Linking.openURL(verificationUrl);
  }, [verificationUrl]);

  const enterAnimation = reduceMotion ? undefined : FadeIn.duration(motion.duration.fast);

  return (
    <Animated.View
      entering={enterAnimation}
      style={[styles.card, { backgroundColor: palette.surface1, borderColor: palette.border }]}
    >
      <Text style={[styles.heading, { color: palette.text }]}>Verify your account</Text>

      <View style={styles.stepRow}>
        <Text style={[styles.stepLabel, { color: palette.textSecondary }]}>1.</Text>
        <Text style={[styles.stepText, { color: palette.textSecondary }]}>
          Open{' '}
          <Text
            style={[styles.link, { color: palette.accent }]}
            accessibilityRole="link"
            onPress={handleOpenUrl}
          >
            {verificationUrl}
          </Text>
        </Text>
      </View>

      <View style={styles.stepRow}>
        <Text style={[styles.stepLabel, { color: palette.textSecondary }]}>2.</Text>
        <Text style={[styles.stepText, { color: palette.textSecondary }]}>Enter code:</Text>
      </View>

      <View style={styles.codeRow}>
        <Text
          style={[styles.codeText, { color: palette.text }]}
          accessibilityLiveRegion="assertive"
          accessibilityLabel={`Verification code: ${userCode}`}
          selectable
        >
          {userCode}
        </Text>
        <Pressable
          onPress={handleCopy}
          accessibilityRole="button"
          accessibilityLabel={copied ? 'Copied' : 'Copy verification code'}
          style={({ pressed }) => [
            styles.copyButton,
            {
              backgroundColor: copied ? palette.successDim : palette.accentDim,
              borderColor: copied ? palette.success : palette.accent,
              opacity: pressed ? 0.7 : 1,
            },
          ]}
        >
          <Text style={[styles.copyLabel, { color: copied ? palette.success : palette.accent }]}>
            {copied ? 'Copied!' : 'Copy'}
          </Text>
        </Pressable>
      </View>

      <View style={styles.waitingRow}>
        <Text style={[styles.waitingDots, { color: palette.accent }]}>
          {'\u25CF \u25CF \u25CF'}
        </Text>
        <Text style={[styles.waitingText, { color: palette.textSecondary }]}>
          Waiting for authorization...
        </Text>
      </View>
    </Animated.View>
  );
}

const styles = StyleSheet.create({
  card: {
    borderRadius: radius.md,
    borderWidth: 1,
    gap: spacing.md,
    padding: spacing.xl,
  },
  heading: {
    ...typography.title,
    fontSize: 15,
  },
  stepRow: {
    flexDirection: 'row',
    gap: spacing.sm,
  },
  stepLabel: {
    ...typography.body,
    fontSize: 14,
  },
  stepText: {
    ...typography.body,
    flex: 1,
    fontSize: 14,
  },
  link: {
    textDecorationLine: 'underline',
  },
  codeRow: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.lg,
    justifyContent: 'space-between',
    paddingVertical: spacing.sm,
  },
  codeText: {
    fontFamily: 'SpaceMono',
    fontSize: 20,
    fontWeight: '600',
    letterSpacing: 1,
    lineHeight: 28,
  },
  copyButton: {
    borderRadius: radius.sm,
    borderWidth: 1,
    minHeight: touchTarget.min,
    minWidth: touchTarget.min,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: spacing.lg,
    paddingVertical: spacing.sm,
  },
  copyLabel: {
    ...typography.label,
    fontSize: 12,
    textTransform: 'uppercase',
  },
  waitingRow: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
  },
  waitingDots: {
    fontSize: 8,
    letterSpacing: 2,
  },
  waitingText: {
    ...typography.caption,
    fontSize: 13,
  },
});
