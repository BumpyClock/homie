import { StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

type StatusPillProps = {
  label: string;
  tone?: 'accent' | 'success' | 'warning';
  compact?: boolean;
};

export function StatusPill({ label, tone = 'accent', compact = false }: StatusPillProps) {
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
        styles.base,
        compact ? styles.compact : styles.regular,
        {
          backgroundColor: background,
          borderColor: foreground,
        },
      ]}>
      <Text style={[compact ? styles.compactLabel : styles.regularLabel, { color: foreground }]}>{label}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  base: {
    alignItems: 'center',
    borderWidth: 1,
    borderRadius: radius.pill,
    justifyContent: 'center',
  },
  regular: {
    minHeight: 28,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.xs,
  },
  compact: {
    minHeight: 20,
    paddingHorizontal: spacing.sm,
    paddingVertical: 3,
  },
  regularLabel: {
    ...typography.label,
    fontSize: 12,
    textTransform: 'uppercase',
  },
  compactLabel: {
    ...typography.label,
    fontSize: 10,
    textTransform: 'uppercase',
  },
});
