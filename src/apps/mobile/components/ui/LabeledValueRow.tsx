import { StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { typography } from '@/theme/tokens';

interface LabeledValueRowProps {
  label: string;
  value: string;
  mono?: boolean;
  last?: boolean;
}

export function LabeledValueRow({ label, value, mono = false, last = false }: LabeledValueRowProps) {
  const { palette } = useAppTheme();

  return (
    <View
      style={[
        styles.row,
        {
          borderBottomColor: palette.border,
          borderBottomWidth: last ? 0 : 1,
        },
      ]}>
      <Text style={[styles.label, { color: palette.textSecondary }]}>{label}</Text>
      <Text style={[mono ? styles.valueMono : styles.value, { color: palette.text }]}>{value}</Text>
    </View>
  );
}

const styles = StyleSheet.create({
  row: {
    gap: 2,
    paddingVertical: 6,
  },
  label: {
    ...typography.label,
    textTransform: 'uppercase',
  },
  value: {
    ...typography.bodyMedium,
    fontSize: 14,
  },
  valueMono: {
    ...typography.mono,
    fontSize: 13,
  },
});
