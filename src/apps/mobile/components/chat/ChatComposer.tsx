import { Pressable, StyleSheet, Text, TextInput, View } from 'react-native';
import { useState } from 'react';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface ChatComposerProps {
  disabled?: boolean;
  sending?: boolean;
  onSend: (message: string) => Promise<void>;
}

export function ChatComposer({
  disabled = false,
  sending = false,
  onSend,
}: ChatComposerProps) {
  const { palette } = useAppTheme();
  const [value, setValue] = useState('');

  const trimmed = value.trim();
  const canSend = !disabled && !sending && trimmed.length > 0;

  const submit = async () => {
    if (!canSend) return;
    const draft = trimmed;
    setValue('');
    try {
      await onSend(draft);
    } catch {
      setValue(draft);
    }
  };

  return (
    <View style={[styles.container, { backgroundColor: palette.surface, borderColor: palette.border }]}>
      <TextInput
        value={value}
        onChangeText={setValue}
        editable={!disabled && !sending}
        placeholder="Message gateway chat"
        placeholderTextColor={palette.textSecondary}
        multiline
        style={[
          styles.input,
          {
            backgroundColor: palette.surfaceAlt,
            borderColor: palette.border,
            color: palette.text,
          },
        ]}
      />
      <Pressable
        accessibilityRole="button"
        onPress={submit}
        disabled={!canSend}
        style={({ pressed }) => [
          styles.sendButton,
          {
            backgroundColor: canSend ? palette.accent : palette.surfaceAlt,
            borderColor: canSend ? palette.accent : palette.border,
            opacity: pressed ? 0.85 : 1,
          },
        ]}>
        <Text style={[styles.sendLabel, { color: canSend ? palette.surface : palette.textSecondary }]}>
          {sending ? 'Sending...' : 'Send'}
        </Text>
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.sm,
    gap: spacing.sm,
  },
  input: {
    ...typography.body,
    minHeight: 72,
    maxHeight: 140,
    borderRadius: radius.md,
    borderWidth: 1,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
    textAlignVertical: 'top',
  },
  sendButton: {
    alignItems: 'center',
    borderRadius: radius.md,
    borderWidth: 1,
    justifyContent: 'center',
    minHeight: 44,
  },
  sendLabel: {
    ...typography.label,
    fontSize: 14,
  },
});
