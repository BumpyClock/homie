import { Pressable, StyleSheet, Text, View } from 'react-native';

import { ScreenSurface } from '@/components/ui/ScreenSurface';
import { runtimeConfig } from '@/config/runtime';
import { StatusPill } from '@/components/ui/StatusPill';
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

  return (
    <ScreenSurface>
      <View style={[styles.container, { backgroundColor: palette.background }]}> 
        <View style={styles.headerRow}>
          <Text style={[styles.title, { color: palette.text }]}>Settings</Text>
          <StatusPill label={mode} />
        </View>

        <View style={[styles.card, { backgroundColor: palette.surface, borderColor: palette.border }]}>
          <Text style={[styles.cardTitle, { color: palette.text }]}>Gateway</Text>
          <SettingRow label="Target" value={runtimeConfig.gatewayUrl} />
          <SettingRow label="Provider" value="OpenAI Codex" />
          <SettingRow label="Model" value="gpt-5.2-codex" />
        </View>

        <Pressable
          accessibilityRole="button"
          style={({ pressed }) => [
            styles.button,
            {
              backgroundColor: palette.surfaceAlt,
              borderColor: palette.border,
              opacity: pressed ? 0.86 : 1,
            },
          ]}>
          <Text style={[styles.buttonLabel, { color: palette.text }]}>Manage Targets</Text>
        </Pressable>
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
  button: {
    minHeight: 48,
    borderRadius: radius.md,
    borderWidth: 1,
    alignItems: 'center',
    justifyContent: 'center',
  },
  buttonLabel: {
    ...typography.label,
  },
});
