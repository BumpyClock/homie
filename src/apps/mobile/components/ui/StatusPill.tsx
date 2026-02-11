import { StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

type StatusPillProps = {
  label: string;
  tone?: 'accent' | 'success' | 'warning';
};

export function StatusPill({ label, tone = 'accent' }: StatusPillProps) {
  const { palette } = useAppTheme();

  const foreground = tone === 'success'
    ? palette.success
    : tone === 'warning'
      ? palette.warning
      : palette.accent;
  const background = tone === 'success'
    ? palette.successDim
    : tone === 'warning'
      ? palette.warningDim
      : palette.accentDim;

  return (
    <View
      accessible
      accessibilityRole="text"
      accessibilityLabel={`Status ${label}`}
      style={[
        styles.container,
        {
          backgroundColor: background,
          borderColor: foreground,
        },
      ]}>
      <Text style={[styles.label, { color: foreground }]}>{label}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    alignItems: 'center',
    borderWidth: 1,
    justifyContent: 'center',
    minHeight: 28,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.xs,
    borderRadius: radius.pill,
  },
  label: {
    ...typography.label,
    fontSize: 12,
    textTransform: 'uppercase',
  },
});
