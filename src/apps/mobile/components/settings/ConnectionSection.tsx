import { Server } from 'lucide-react-native';
import { ScrollView, StyleSheet, Text, View } from 'react-native';

import { GatewayTargetForm } from '@/components/gateway/GatewayTargetForm';
import { LabeledValueRow } from '@/components/ui/LabeledValueRow';
import { StatusPill } from '@/components/ui/StatusPill';
import { runtimeConfig } from '@/config/runtime';
import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface ConnectionSectionProps {
  status: string;
  loadingTarget: boolean;
  targetUrl: string | null;
  targetHint: string;
  hasTarget: boolean;
  targetError: string | null;
  savingTarget: boolean;
  saveGatewayTarget: (value: string) => Promise<void>;
  clearGatewayTarget: () => Promise<void>;
  statusBadge: { label: string; tone: 'accent' | 'success' | 'warning' };
  error: string | null;
}

function statusDetails(status: string, hasTarget: boolean, loadingTarget: boolean): string {
  if (loadingTarget) return 'Loading saved gateway target from device storage.';
  if (!hasTarget) return 'No gateway target set yet. Add one below to start syncing.';
  if (status === 'connected') return 'Live connection is healthy. Chat and terminals are ready.';
  if (status === 'connecting' || status === 'handshaking')
    return 'Connecting to gateway now. This should settle in a moment.';
  if (status === 'rejected') return 'Gateway rejected the session. Confirm auth and URL path.';
  if (status === 'error') return 'Connection error from gateway. Check URL, network, and certificates.';
  return 'Disconnected from gateway. Reconnect by confirming target URL.';
}

export function ConnectionSection({
  status,
  loadingTarget,
  targetUrl,
  targetHint,
  hasTarget,
  targetError,
  savingTarget,
  saveGatewayTarget,
  clearGatewayTarget,
  statusBadge,
}: ConnectionSectionProps) {
  const { palette } = useAppTheme();
  const activeTarget = targetUrl ?? targetHint ?? 'Not set';

  return (
    <ScrollView
      style={styles.scroll}
      contentContainerStyle={styles.scrollContent}
      showsVerticalScrollIndicator={false}
    >
      {/* Hero card — gateway status */}
      <View style={[styles.heroCard, { backgroundColor: palette.surface1, borderColor: palette.border }]}>
        <View style={styles.heroHeader}>
          <Text style={[styles.heroEyebrow, { color: palette.textSecondary }]}>Connection</Text>
          <StatusPill compact label={statusBadge.label} tone={statusBadge.tone} />
        </View>
        <Text style={[styles.heroTitle, { color: palette.text }]}>Gateway Status</Text>
        <Text style={[styles.heroBody, { color: palette.textSecondary }]}>
          {statusDetails(status, hasTarget, loadingTarget)}
        </Text>
        <LabeledValueRow label="Current target" value={activeTarget} mono />
        <LabeledValueRow label="Transport state" value={status} mono last />
        {targetError ? (
          <View
            style={[styles.inlineAlert, { backgroundColor: palette.dangerDim, borderColor: palette.danger }]}
          >
            <Text style={[styles.inlineAlertText, { color: palette.danger }]}>{targetError}</Text>
          </View>
        ) : null}
      </View>

      {/* Target card — gateway form */}
      <View style={[styles.card, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
        <View style={styles.cardHeader}>
          <Server size={14} color={palette.accent} />
          <Text style={[styles.cardTitle, { color: palette.text }]}>Gateway Target</Text>
        </View>
        <Text style={[styles.cardBody, { color: palette.textSecondary }]}>
          Set the URL used by mobile chat + terminal sessions.
        </Text>
        <GatewayTargetForm
          initialValue={targetUrl ?? targetHint}
          hintValue={targetHint}
          saving={savingTarget || loadingTarget}
          saveLabel={hasTarget ? 'Update Target' : 'Save Target'}
          onSave={saveGatewayTarget}
          onClear={hasTarget ? clearGatewayTarget : undefined}
        />
        {!targetHint ? (
          <Text style={[styles.meta, { color: palette.textSecondary }]}>No env fallback. Set target manually.</Text>
        ) : (
          <Text style={[styles.meta, { color: palette.textSecondary }]}>
            Env fallback available: {runtimeConfig.gatewayUrl}
          </Text>
        )}
      </View>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  scroll: {
    flex: 1,
  },
  scrollContent: {
    gap: spacing.md,
    paddingBottom: spacing.xxxl,
  },
  heroCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    gap: spacing.sm,
    padding: spacing.lg,
  },
  heroHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
    justifyContent: 'space-between',
  },
  heroEyebrow: {
    ...typography.overline,
    textTransform: 'uppercase',
  },
  heroTitle: {
    ...typography.heading,
    fontSize: 20,
  },
  heroBody: {
    ...typography.body,
    fontSize: 14,
  },
  card: {
    borderRadius: radius.lg,
    borderWidth: 1,
    gap: spacing.md,
    padding: spacing.lg,
  },
  cardHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
  },
  cardTitle: {
    ...typography.title,
    fontSize: 18,
  },
  cardBody: {
    ...typography.body,
    fontSize: 14,
  },
  inlineAlert: {
    borderRadius: radius.md,
    borderWidth: 1,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
  },
  inlineAlertText: {
    ...typography.caption,
    fontWeight: '600',
  },
  meta: {
    ...typography.monoSmall,
    fontSize: 12,
  },
});
