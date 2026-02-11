import { Pressable, StyleSheet, Text } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

type ActionButtonVariant = 'primary' | 'secondary';

interface ActionButtonProps {
  label: string;
  onPress: () => void;
  disabled?: boolean;
  variant?: ActionButtonVariant;
  flex?: boolean;
}

export function ActionButton({
  label,
  onPress,
  disabled = false,
  variant = 'secondary',
  flex = false,
}: ActionButtonProps) {
  const { palette } = useAppTheme();
  const primary = variant === 'primary';

  return (
    <Pressable
      accessibilityRole="button"
      disabled={disabled}
      onPress={onPress}
      style={({ pressed }) => [
        styles.button,
        flex ? styles.flex : null,
        {
          backgroundColor: primary ? palette.accent : palette.surface1,
          borderColor: primary ? palette.accent : palette.border,
          opacity: pressed ? 0.86 : disabled ? 0.58 : 1,
        },
      ]}>
      <Text style={[styles.label, { color: primary ? palette.surface0 : palette.text }]}>{label}</Text>
    </Pressable>
  );
}

const styles = StyleSheet.create({
  button: {
    alignItems: 'center',
    borderRadius: radius.md,
    borderWidth: 1,
    justifyContent: 'center',
    minHeight: 44,
    paddingHorizontal: spacing.md,
  },
  flex: {
    flex: 1,
  },
  label: {
    ...typography.label,
    fontSize: 13,
  },
});
