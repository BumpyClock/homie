// ABOUTME: Chat message composer with inline send button.
// ABOUTME: Uses a horizontal layout with expanding TextInput and circular send icon, anchored above keyboard via KeyboardStickyView in parent.

import { Feather } from '@expo/vector-icons';
import * as Haptics from 'expo-haptics';
import { useState } from 'react';
import { Platform, Pressable, StyleSheet, TextInput, View } from 'react-native';

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
    if (Platform.OS !== 'web') {
      Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
    }
    try {
      await onSend(draft);
    } catch {
      setValue(draft);
    }
  };

  return (
    <View style={[styles.container, { backgroundColor: palette.surface, borderTopColor: palette.border }]}>
      <TextInput
        value={value}
        onChangeText={setValue}
        editable={!disabled && !sending}
        placeholder="Message…"
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
        accessibilityLabel={sending ? 'Sending message' : 'Send message'}
        onPress={submit}
        disabled={!canSend}
        style={({ pressed }) => [
          styles.sendButton,
          {
            backgroundColor: canSend ? palette.accent : palette.surfaceAlt,
            opacity: pressed && canSend ? 0.8 : 1,
          },
        ]}>
        <Feather
          name="arrow-up"
          size={18}
          color={canSend ? '#FFFFFF' : palette.textSecondary}
        />
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    gap: spacing.sm,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
    borderTopWidth: 1,
  },
  input: {
    ...typography.body,
    flex: 1,
    fontSize: 16,
    minHeight: 40,
    maxHeight: 120,
    borderRadius: radius.md,
    borderWidth: 1,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
    textAlignVertical: 'top',
  },
  sendButton: {
    alignItems: 'center',
    justifyContent: 'center',
    width: 40,
    height: 40,
    borderRadius: 20,
  },
});
