import { StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { palettes, radius, spacing, typography } from '@/theme/tokens';

type StatusPillProps = {
  label: string;
  tone?: 'accent' | 'success' | 'warning';
};

export function StatusPill({ label, tone = 'accent' }: StatusPillProps) {
  const { palette } = useAppTheme();

  const background = tone === 'success' ? palette.success : tone === 'warning' ? palette.warning : palette.accent;

  return (
    <View style={[styles.container, { backgroundColor: background }]}> 
      <Text style={[styles.label, { color: palettes.light.surface0 }]}>{label}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.xs,
    borderRadius: radius.pill,
    minHeight: 28,
    justifyContent: 'center',
  },
  label: {
    ...typography.label,
    fontSize: 12,
    textTransform: 'uppercase',
  },
});
