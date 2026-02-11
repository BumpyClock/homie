import { StyleSheet, Text, View } from 'react-native';
import { useState } from 'react';

import { GatewayTargetForm } from '@/components/gateway/GatewayTargetForm';
import { ScreenSurface } from '@/components/ui/ScreenSurface';
import { runtimeConfig } from '@/config/runtime';
import { StatusPill } from '@/components/ui/StatusPill';
import { useAppTheme } from '@/hooks/useAppTheme';
import { useGatewayTarget } from '@/hooks/useGatewayTarget';
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
  const [saving, setSaving] = useState(false);
  const {
    loading,
    targetUrl,
    targetHint,
    hasTarget,
    error,
    saveTarget,
    clearTarget,
  } = useGatewayTarget();

  const handleSaveTarget = async (value: string) => {
    setSaving(true);
    try {
      await saveTarget(value);
    } finally {
      setSaving(false);
    }
  };

  const handleClearTarget = async () => {
    setSaving(true);
    try {
      await clearTarget();
    } finally {
      setSaving(false);
    }
  };

  return (
    <ScreenSurface>
      <View style={[styles.container, { backgroundColor: palette.background }]}>
        <View style={styles.headerRow}>
          <Text style={[styles.title, { color: palette.text }]}>Settings</Text>
          <StatusPill label={mode} />
        </View>

        <View style={[styles.card, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
          <Text style={[styles.cardTitle, { color: palette.text }]}>Gateway</Text>
          <SettingRow label="Target" value={targetUrl ?? 'Not set'} />
          <SettingRow label="Provider" value="OpenAI Codex" />
          <SettingRow label="Model" value="gpt-5.2-codex" />
          <GatewayTargetForm
            initialValue={targetUrl ?? targetHint}
            hintValue={targetHint}
            saving={saving || loading}
            saveLabel={hasTarget ? 'Update Target' : 'Save Target'}
            onSave={handleSaveTarget}
            onClear={hasTarget ? handleClearTarget : undefined}
          />
          {error ? <Text style={[styles.meta, { color: palette.textSecondary }]}>{error}</Text> : null}
          {!targetHint ? (
            <Text style={[styles.meta, { color: palette.textSecondary }]}>
              No env fallback. Set target manually.
            </Text>
          ) : (
            <Text style={[styles.meta, { color: palette.textSecondary }]}>
              Env fallback available: {runtimeConfig.gatewayUrl}
            </Text>
          )}
        </View>
      </View>
    </ScreenSurface>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    paddingTop: spacing.xxl,
    paddingHorizontal: spacing.lg,
    gap: spacing.lg,
  },
  headerRow: {
    alignItems: 'center',
    flexDirection: 'row',
    justifyContent: 'space-between',
  },
  title: {
    ...typography.display,
  },
  card: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.xl,
    gap: spacing.md,
  },
  cardTitle: {
    ...typography.title,
    marginBottom: spacing.xs,
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
});
