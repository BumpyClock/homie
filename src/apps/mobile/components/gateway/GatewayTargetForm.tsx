import { useEffect, useState } from 'react';
import { Pressable, StyleSheet, Text, TextInput, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { palettes, radius, spacing, typography } from '@/theme/tokens';

interface GatewayTargetFormProps {
  initialValue?: string;
  hintValue?: string;
  saveLabel?: string;
  saving?: boolean;
  disabled?: boolean;
  onSave: (value: string) => Promise<void>;
  onClear?: () => Promise<void>;
}

export function GatewayTargetForm({
  initialValue = '',
  hintValue = '',
  saveLabel = 'Save Target',
  saving = false,
  disabled = false,
  onSave,
  onClear,
}: GatewayTargetFormProps) {
  const { palette } = useAppTheme();
  const [value, setValue] = useState(initialValue);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setValue(initialValue);
  }, [initialValue]);

  const saveDisabled = disabled || saving || !value.trim();

  const handleSave = async () => {
    if (saveDisabled) return;
    setError(null);
    try {
      await onSave(value);
    } catch (nextError) {
      const message = nextError instanceof Error ? nextError.message : 'Failed to save target';
      setError(message);
    }
  };

  const handleClear = async () => {
    if (!onClear || disabled || saving) return;
    setError(null);
    try {
      await onClear();
      setValue('');
    } catch (nextError) {
      const message = nextError instanceof Error ? nextError.message : 'Failed to clear target';
      setError(message);
    }
  };

  return (
    <View style={styles.container}>
      <TextInput
        value={value}
        onChangeText={setValue}
        editable={!disabled && !saving}
        autoCapitalize="none"
        autoCorrect={false}
        keyboardType="url"
        placeholder={hintValue || 'wss://gateway.example.com/ws'}
        placeholderTextColor={palette.textSecondary}
        style={[
          styles.input,
          {
            backgroundColor: palette.surface1,
            borderColor: palette.border,
            color: palette.text,
          },
        ]}
      />
      {hintValue ? (
        <Text style={[styles.hint, { color: palette.textSecondary }]}>
          Env hint: {hintValue}
        </Text>
      ) : null}
      {error ? <Text style={[styles.error, { color: palette.danger }]}>{error}</Text> : null}
      <View style={styles.actions}>
        <Pressable
          accessibilityRole="button"
          disabled={saveDisabled}
          onPress={() => {
            void handleSave();
          }}
          style={({ pressed }) => [
            styles.actionButton,
            styles.primaryAction,
            {
              backgroundColor: saveDisabled ? palette.surface1 : palette.accent,
              borderColor: saveDisabled ? palette.border : palette.accent,
              opacity: pressed ? 0.86 : 1,
            },
          ]}>
          <Text
            style={[
              styles.actionLabel,
              { color: saveDisabled ? palette.textSecondary : palettes.light.surface0 },
            ]}>
            {saving ? 'Saving...' : saveLabel}
          </Text>
        </Pressable>
        {onClear ? (
          <Pressable
            accessibilityRole="button"
            disabled={disabled || saving}
            onPress={() => {
              void handleClear();
            }}
            style={({ pressed }) => [
              styles.actionButton,
              {
                backgroundColor: palette.surface0,
                borderColor: palette.border,
                opacity: pressed ? 0.86 : 1,
              },
            ]}>
            <Text style={[styles.actionLabel, { color: palette.text }]}>Clear</Text>
          </Pressable>
        ) : null}
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    gap: spacing.sm,
  },
  input: {
    ...typography.body,
    borderRadius: radius.md,
    borderWidth: 1,
    minHeight: 48,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
  },
  hint: {
    ...typography.data,
    fontSize: 12,
  },
  error: {
    ...typography.body,
    fontSize: 13,
    fontWeight: '500',
  },
  actions: {
    flexDirection: 'row',
    gap: spacing.sm,
  },
  actionButton: {
    borderRadius: radius.md,
    borderWidth: 1,
    minHeight: 44,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: spacing.md,
  },
  primaryAction: {
    flex: 1,
  },
  actionLabel: {
    ...typography.label,
    fontSize: 13,
  },
});
