import {
  CircleHelp,
  Link2,
  Server,
  SlidersHorizontal,
  Wifi,
} from 'lucide-react-native';
import { ScrollView, StyleSheet, Text, View, useWindowDimensions } from 'react-native';

import { AppShell } from '@/components/shell/AppShell';
import { useMobileShellData } from '@/components/shell/MobileShellDataContext';
import { GatewayTargetForm } from '@/components/gateway/GatewayTargetForm';
import { LabeledValueRow } from '@/components/ui/LabeledValueRow';
import { StatusPill } from '@/components/ui/StatusPill';
import { runtimeConfig } from '@/config/runtime';
import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

function statusDetails(status: string, hasTarget: boolean, loadingTarget: boolean): string {
  if (loadingTarget) return 'Loading saved gateway target from device storage.';
  if (!hasTarget) return 'No gateway target set yet. Add one below to start syncing.';
  if (status === 'connected') return 'Live connection is healthy. Chat and terminals are ready.';
  if (status === 'connecting' || status === 'handshaking') return 'Connecting to gateway now. This should settle in a moment.';
  if (status === 'rejected') return 'Gateway rejected the session. Confirm auth and URL path.';
  if (status === 'error') return 'Connection error from gateway. Check URL, network, and certificates.';
  return 'Disconnected from gateway. Reconnect by confirming target URL.';
}

export default function SettingsTabScreen() {
  const { width } = useWindowDimensions();
  const { palette, mode } = useAppTheme();
  const {
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
    error,
  } = useMobileShellData();
  const activeTarget = targetUrl ?? targetHint ?? 'Not set';
  const wideLayout = width >= 1080;

  const connectionCard = (
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
        <View style={[styles.inlineAlert, { backgroundColor: palette.dangerDim, borderColor: palette.danger }]}> 
          <Text style={[styles.inlineAlertText, { color: palette.danger }]}>{targetError}</Text>
        </View>
      ) : null}
    </View>
  );

  const targetCard = (
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
        <Text style={[styles.meta, { color: palette.textSecondary }]}>Env fallback available: {runtimeConfig.gatewayUrl}</Text>
      )}
    </View>
  );

  const defaultsCard = (
    <View style={[styles.card, { backgroundColor: palette.surface0, borderColor: palette.border }]}> 
      <View style={styles.cardHeader}>
        <SlidersHorizontal size={14} color={palette.accent} />
        <Text style={[styles.cardTitle, { color: palette.text }]}>App Defaults & Help</Text>
      </View>
      <LabeledValueRow label="Theme" value={mode} />
      <LabeledValueRow label="Provider" value="OpenAI Codex" />
      <LabeledValueRow label="Model" value="gpt-5.2-codex" />
      <LabeledValueRow label="Gateway path" value="/ws" mono last />
      <View style={[styles.helpCard, { backgroundColor: palette.surface1, borderColor: palette.border }]}> 
        <Text style={[styles.helpTitle, { color: palette.text }]}>Tips</Text>
        <Text style={[styles.helpItem, { color: palette.textSecondary }]}>- Prefer `wss://` for remote access.</Text>
        <Text style={[styles.helpItem, { color: palette.textSecondary }]}>- Include `/ws` at the end of your gateway URL.</Text>
        <Text style={[styles.helpItem, { color: palette.textSecondary }]}>- Update target here any time to switch machines.</Text>
      </View>
    </View>
  );

  return (
    <AppShell
      section="settings"
      hasTarget={hasTarget}
      loadingTarget={loadingTarget}
      error={error}
      statusBadge={statusBadge}
      renderDrawerContent={() => (
        <View style={[styles.drawerCard, { borderColor: palette.border, backgroundColor: palette.surface1 }]}> 
          <Text style={[styles.drawerEyebrow, { color: palette.textSecondary }]}>Quick Links</Text>
          <View style={styles.drawerItem}>
            <Wifi size={13} color={palette.accent} />
            <Text style={[styles.drawerItemLabel, { color: palette.text }]}>Connection Status</Text>
          </View>
          <View style={styles.drawerItem}>
            <Link2 size={13} color={palette.accent} />
            <Text style={[styles.drawerItemLabel, { color: palette.text }]}>Gateway Target</Text>
          </View>
          <View style={styles.drawerItem}>
            <CircleHelp size={13} color={palette.accent} />
            <Text style={[styles.drawerItemLabel, { color: palette.text }]}>Defaults & Help</Text>
          </View>
          <Text style={[styles.drawerNote, { color: palette.textSecondary }]}>Configure once, then switch targets anytime from this screen.</Text>
        </View>
      )}>
      <ScrollView
        style={styles.scroll}
        contentContainerStyle={styles.scrollContent}
        showsVerticalScrollIndicator={false}>
        {wideLayout ? (
          <View style={styles.columns}>
            <View style={styles.primaryColumn}>
              {connectionCard}
              {targetCard}
            </View>
            <View style={styles.secondaryColumn}>{defaultsCard}</View>
          </View>
        ) : (
          <>
            {connectionCard}
            {targetCard}
            {defaultsCard}
          </>
        )}
      </ScrollView>
    </AppShell>
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
  columns: {
    alignItems: 'flex-start',
    flexDirection: 'row',
    gap: spacing.lg,
  },
  primaryColumn: {
    flex: 1.4,
    gap: spacing.md,
    minWidth: 0,
  },
  secondaryColumn: {
    flex: 1,
    minWidth: 0,
  },
  heroCard: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.lg,
    gap: spacing.sm,
  },
  heroHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
    gap: spacing.sm,
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
    padding: spacing.lg,
    gap: spacing.md,
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
  helpCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    padding: spacing.md,
    gap: spacing.xs,
  },
  helpTitle: {
    ...typography.caption,
    textTransform: 'uppercase',
  },
  helpItem: {
    ...typography.caption,
    fontSize: 12,
    fontWeight: '500',
  },
  drawerCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    marginHorizontal: spacing.md,
    marginTop: spacing.sm,
    padding: spacing.md,
    gap: spacing.sm,
  },
  drawerEyebrow: {
    ...typography.label,
    textTransform: 'uppercase',
  },
  drawerItem: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
  },
  drawerItemLabel: {
    ...typography.bodyMedium,
    fontSize: 14,
  },
  drawerNote: {
    ...typography.caption,
    fontSize: 12,
  },
});
