import { useCallback, useEffect } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  useReducedMotion,
  withTiming,
} from 'react-native-reanimated';

import { AUTH_COPY } from '@homie/shared';
import type { ChatAccountProviderStatus } from '@homie/shared';
import type { ProviderAuthState } from '@homie/shared';

import { DeviceCodeCard } from '@/components/settings/DeviceCodeCard';
import { StatusPill } from '@/components/ui/StatusPill';
import { useAppTheme } from '@/hooks/useAppTheme';
import { motion, triggerMobileHaptic } from '@/theme/motion';
import { radius, spacing, touchTarget, typography } from '@/theme/tokens';

interface ProviderRowProps {
  provider: ChatAccountProviderStatus;
  providerLabel: string;
  authState: ProviderAuthState | undefined;
  onConnect: (providerId: string) => void;
  onCancel: (providerId: string) => void;
}

const EXPANDED_HEIGHT = 280;
const COLLAPSED_HEIGHT = 0;

export function ProviderRow({
  provider,
  providerLabel,
  authState,
  onConnect,
  onCancel,
}: ProviderRowProps) {
  const { palette } = useAppTheme();
  const reduceMotion = useReducedMotion();
  const status = authState?.status ?? (provider.loggedIn ? 'authorized' : 'idle');
  const isPolling = status === 'polling';
  const isError = status === 'error' || status === 'denied' || status === 'expired';

  const expandHeight = useSharedValue(isPolling ? EXPANDED_HEIGHT : COLLAPSED_HEIGHT);

  useEffect(() => {
    if (reduceMotion) {
      expandHeight.value = isPolling ? EXPANDED_HEIGHT : COLLAPSED_HEIGHT;
      return;
    }
    expandHeight.value = withTiming(isPolling ? EXPANDED_HEIGHT : COLLAPSED_HEIGHT, {
      duration: motion.duration.standard,
      easing: motion.easing.move,
    });
  }, [isPolling, expandHeight, reduceMotion]);

  useEffect(() => {
    if (status === 'authorized') {
      triggerMobileHaptic(motion.haptics.providerAuthorized);
    } else if (status === 'denied' || status === 'expired') {
      triggerMobileHaptic(motion.haptics.providerDenied);
    }
  }, [status]);

  const expandStyle = useAnimatedStyle(() => ({
    height: expandHeight.value,
    overflow: 'hidden' as const,
    opacity: expandHeight.value > 10 ? 1 : 0,
  }));

  const handleAction = useCallback(() => {
    if (isPolling) {
      onCancel(provider.id);
      return;
    }
    triggerMobileHaptic(motion.haptics.providerConnect);
    onConnect(provider.id);
  }, [isPolling, onCancel, onConnect, provider.id]);

  const errorMessage = authState?.error;
  const scopes = provider.scopes?.join(' \u00B7 ');
  const expiresAt = provider.expiresAt;

  return (
    <View
      style={[styles.row, { backgroundColor: palette.surface0, borderColor: palette.border }]}
      accessible
      accessibilityLabel={`${providerLabel}, ${statusAccessibilityLabel(status)}`}
    >
      <View style={styles.header}>
        <View style={styles.nameColumn}>
          <Text style={[styles.providerName, { color: palette.text }]}>{providerLabel}</Text>
          {status === 'authorized' && scopes ? (
            <Text style={[styles.meta, { color: palette.textSecondary }]}>Scopes: {scopes}</Text>
          ) : null}
          {status === 'authorized' && expiresAt ? (
            <Text style={[styles.meta, { color: palette.textSecondary }]}>
              Expires: {expiresAt.slice(0, 10)}
            </Text>
          ) : null}
          {isError && errorMessage ? (
            <Text style={[styles.errorText, { color: palette.danger }]}>{errorMessage}</Text>
          ) : null}
          {status === 'idle' && !provider.loggedIn ? (
            <Text style={[styles.meta, { color: palette.textSecondary }]}>{AUTH_COPY.statusNotConnected}</Text>
          ) : null}
          {status === 'starting' ? (
            <Text style={[styles.meta, { color: palette.textSecondary }]}>
              Starting device code flow...
            </Text>
          ) : null}
        </View>

        <View style={styles.actionColumn}>
          {status === 'authorized' ? (
            <StatusPill compact label={AUTH_COPY.statusConnected} tone="success" />
          ) : (
            <Pressable
              onPress={handleAction}
              accessibilityRole="button"
              accessibilityLabel={actionAccessibilityLabel(status, providerLabel)}
              disabled={status === 'starting'}
              style={({ pressed }) => [
                styles.actionButton,
                {
                  backgroundColor: isError ? palette.dangerDim : palette.accentDim,
                  borderColor: isError ? palette.danger : palette.accent,
                  opacity: status === 'starting' || pressed ? 0.7 : 1,
                },
              ]}
            >
              <Text
                style={[
                  styles.actionLabel,
                  { color: isError ? palette.danger : palette.accent },
                ]}
              >
                {actionButtonLabel(status)}
              </Text>
            </Pressable>
          )}
        </View>
      </View>

      <Animated.View style={expandStyle}>
        {isPolling && authState?.session ? (
          <View style={styles.expandContent}>
            <DeviceCodeCard
              verificationUrl={authState.session.verificationUrl}
              userCode={authState.session.userCode}
            />
          </View>
        ) : null}
      </Animated.View>
    </View>
  );
}

function actionButtonLabel(status: string): string {
  switch (status) {
    case 'starting':
      return 'Connecting\u2026';
    case 'polling':
      return 'Cancel';
    case 'error':
    case 'denied':
    case 'expired':
      return 'Try Again';
    default:
      return 'Connect';
  }
}

function statusAccessibilityLabel(status: string): string {
  switch (status) {
    case 'authorized':
      return AUTH_COPY.statusConnected.toLowerCase();
    case 'starting':
      return 'connecting';
    case 'polling':
      return 'waiting for authorization';
    case 'error':
    case 'denied':
    case 'expired':
      return 'connection failed';
    default:
      return AUTH_COPY.statusNotConnected.toLowerCase();
  }
}

function actionAccessibilityLabel(status: string, providerLabel: string): string {
  switch (status) {
    case 'starting':
      return `Connecting to ${providerLabel}`;
    case 'polling':
      return `Cancel connection to ${providerLabel}`;
    case 'error':
    case 'denied':
    case 'expired':
      return `Retry connection to ${providerLabel}`;
    default:
      return `Connect to ${providerLabel}`;
  }
}

const styles = StyleSheet.create({
  row: {
    borderRadius: radius.lg,
    borderWidth: 1,
    overflow: 'hidden',
  },
  header: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
    padding: spacing.xl,
  },
  nameColumn: {
    flex: 1,
    gap: spacing.xs,
  },
  providerName: {
    ...typography.bodyMedium,
    fontSize: 15,
  },
  meta: {
    ...typography.caption,
    fontSize: 13,
  },
  errorText: {
    ...typography.caption,
    fontSize: 13,
    fontWeight: '600',
  },
  actionColumn: {
    alignItems: 'flex-end',
    justifyContent: 'center',
    marginLeft: spacing.md,
  },
  actionButton: {
    alignItems: 'center',
    borderRadius: radius.sm,
    borderWidth: 1,
    justifyContent: 'center',
    minHeight: touchTarget.min,
    paddingHorizontal: spacing.xl,
    paddingVertical: spacing.sm,
  },
  actionLabel: {
    ...typography.label,
    fontSize: 12,
    textTransform: 'uppercase',
  },
  expandContent: {
    paddingBottom: spacing.xl,
    paddingHorizontal: spacing.xl,
  },
});
