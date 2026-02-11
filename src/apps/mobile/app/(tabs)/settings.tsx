import { StyleSheet, Text, View } from 'react-native';

import { AppShell } from '@/components/shell/AppShell';
import { useMobileShellData } from '@/components/shell/MobileShellDataContext';
import { GatewayTargetForm } from '@/components/gateway/GatewayTargetForm';
import { runtimeConfig } from '@/config/runtime';
import { useAppTheme } from '@/hooks/useAppTheme';
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

export default function SettingsTabScreen() {
  const { palette, mode } = useAppTheme();
  const {
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

  return (
    <AppShell
      section="settings"
      hasTarget={hasTarget}
      loadingTarget={loadingTarget}
      error={error}
      statusBadge={statusBadge}
      renderDrawerContent={() => (
        <View style={[styles.emptySection, { borderColor: palette.border, backgroundColor: palette.surface1 }]}> 
          <Text style={[styles.emptySectionText, { color: palette.textSecondary }]}>No nested items for Settings.</Text>
        </View>
      )}>
      <View style={[styles.card, { backgroundColor: palette.surface0, borderColor: palette.border }]}> 
        <Text style={[styles.cardTitle, { color: palette.text }]}>Gateway Settings</Text>
        <SettingRow label="Target" value={targetUrl ?? 'Not set'} />
        <SettingRow label="Theme" value={mode} />
        <SettingRow label="Provider" value="OpenAI Codex" />
        <SettingRow label="Model" value="gpt-5.2-codex" />
        <GatewayTargetForm
          initialValue={targetUrl ?? targetHint}
          hintValue={targetHint}
          saving={savingTarget || loadingTarget}
          saveLabel={hasTarget ? 'Update Target' : 'Save Target'}
          onSave={saveGatewayTarget}
          onClear={hasTarget ? clearGatewayTarget : undefined}
        />
        {targetError ? <Text style={[styles.meta, { color: palette.textSecondary }]}>{targetError}</Text> : null}
        {!targetHint ? (
          <Text style={[styles.meta, { color: palette.textSecondary }]}>No env fallback. Set target manually.</Text>
        ) : (
          <Text style={[styles.meta, { color: palette.textSecondary }]}>Env fallback available: {runtimeConfig.gatewayUrl}</Text>
        )}
      </View>
    </AppShell>
  );
}

const styles = StyleSheet.create({
  card: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.xl,
    gap: spacing.md,
  },
  cardTitle: {
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
    ...typography.data,
    fontSize: 12,
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
});
