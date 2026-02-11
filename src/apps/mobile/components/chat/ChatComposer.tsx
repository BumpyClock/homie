// ABOUTME: Chat message composer with inline send button.
// ABOUTME: Uses a horizontal layout with expanding TextInput and circular send icon, anchored above keyboard via KeyboardStickyView in parent.

import { Feather } from '@expo/vector-icons';
import * as Haptics from 'expo-haptics';
import { useRef, useState } from 'react';
import {
  Platform,
  Pressable,
  StyleSheet,
  TextInput,
  View,
} from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface ChatComposerProps {
  disabled?: boolean;
  sending?: boolean;
  bottomInset?: number;
  onSend: (message: string) => Promise<void>;
}

export function ChatComposer({
  disabled = false,
  sending = false,
  bottomInset = 0,
  onSend,
}: ChatComposerProps) {
  const { palette } = useAppTheme();
  const [value, setValue] = useState('');
  const inputRef = useRef<TextInput>(null);

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

  // Ensure the composer clears the home indicator / nav bar
  const safeBottomPadding = Math.max(bottomInset, spacing.sm);

  return (
    <View
      style={[
        styles.container,
        {
          backgroundColor: palette.surface,
          borderTopColor: palette.border,
          paddingBottom: safeBottomPadding,
        },
      ]}>
      <View style={styles.inputRow}>
        <View
          style={[
            styles.inputWrap,
            {
              backgroundColor: palette.surfaceAlt,
              borderColor: palette.border,
            },
          ]}>
          <TextInput
            ref={inputRef}
            value={value}
            onChangeText={setValue}
            editable={!disabled && !sending}
            placeholder="Message…"
            placeholderTextColor={palette.textSecondary}
            multiline
            style={[styles.input, { color: palette.text }]}
          />
        </View>
        <Pressable
          accessibilityRole="button"
          accessibilityLabel={sending ? 'Sending message' : 'Send message'}
          onPress={submit}
          disabled={!canSend}
          style={({ pressed }) => [
            styles.sendButton,
            {
              backgroundColor: canSend ? palette.accent : palette.surfaceAlt,
              opacity: pressed && canSend ? 0.82 : 1,
              transform: [{ scale: pressed && canSend ? 0.92 : 1 }],
            },
          ]}>
          <Feather
            name="arrow-up"
            size={18}
            color={canSend ? '#FFFFFF' : palette.textSecondary}
          />
        </Pressable>
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    borderTopWidth: 1,
    paddingTop: spacing.sm,
    paddingHorizontal: spacing.md,
  },
  inputRow: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    gap: spacing.sm,
  },
  inputWrap: {
    flex: 1,
    borderRadius: radius.lg,
    borderWidth: 1,
    overflow: 'hidden',
  },
  input: {
    ...typography.body,
    fontSize: 16,
    minHeight: 40,
    maxHeight: 120,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
    textAlignVertical: 'top',
  },
  sendButton: {
    alignItems: 'center',
    justifyContent: 'center',
    width: 36,
    height: 36,
    borderRadius: 18,
    marginBottom: 2,
  },
});
