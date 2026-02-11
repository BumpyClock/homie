// ABOUTME: Chat message composer with inline send button.
// ABOUTME: Uses a horizontal layout with expanding TextInput and circular send icon, anchored above keyboard via KeyboardStickyView in parent.

import { Feather } from '@expo/vector-icons';
import type { ChatEffort, ModelOption, ReasoningEffortOption } from '@homie/shared';
import * as Haptics from 'expo-haptics';
import { useRef, useState } from 'react';
import {
  Platform,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';

import { EffortPickerSheet } from '@/components/chat/EffortPickerSheet';
import { ModelPickerSheet } from '@/components/chat/ModelPickerSheet';
import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface ChatComposerProps {
  disabled?: boolean;
  sending?: boolean;
  bottomInset?: number;
  models?: ModelOption[];
  selectedModel?: string | null;
  selectedEffort?: ChatEffort;
  onSelectModel?: (modelId: string) => void;
  onSelectEffort?: (effort: ChatEffort) => void;
  onSend: (message: string) => Promise<void>;
}

export function ChatComposer({
  disabled = false,
  sending = false,
  bottomInset = 0,
  models = [],
  selectedModel = null,
  selectedEffort = 'auto',
  onSelectModel,
  onSelectEffort,
  onSend,
}: ChatComposerProps) {
  const { palette } = useAppTheme();
  const [value, setValue] = useState('');
  const [pickerVisible, setPickerVisible] = useState(false);
  const [effortPickerVisible, setEffortPickerVisible] = useState(false);
  const inputRef = useRef<TextInput>(null);

  const trimmed = value.trim();
  const canSend = !disabled && !sending && trimmed.length > 0;

  const activeModel =
    models.find((m) => m.model === selectedModel || m.id === selectedModel) ??
    models.find((m) => m.isDefault) ??
    null;
  const modelLabel = activeModel?.displayName ?? activeModel?.model ?? null;
  const showModelPill = models.length > 0 && onSelectModel;

  const supportedEfforts: ReasoningEffortOption[] = activeModel?.supportedReasoningEfforts ?? [];
  const defaultEffort = activeModel?.defaultReasoningEffort ?? null;
  const effortLabel = selectedEffort === 'auto'
    ? 'Auto'
    : selectedEffort.charAt(0).toUpperCase() + selectedEffort.slice(1);
  const showEffortPill = onSelectEffort != null;

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
      {showModelPill ? (
        <View style={styles.pillRow}>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel={`Model: ${modelLabel ?? 'Default'}. Tap to change.`}
            onPress={() => setPickerVisible(true)}
            disabled={disabled}
            style={({ pressed }) => [
              styles.modelPill,
              {
                backgroundColor: palette.surfaceAlt,
                borderColor: palette.border,
                opacity: pressed ? 0.78 : disabled ? 0.55 : 1,
              },
            ]}>
            <Feather name="cpu" size={12} color={palette.textSecondary} />
            <Text
              style={[styles.modelPillLabel, { color: palette.textSecondary }]}
              numberOfLines={1}>
              {modelLabel ?? 'Default'}
            </Text>
            <Feather name="chevron-down" size={12} color={palette.textSecondary} />
          </Pressable>
          {showEffortPill ? (
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={`Effort: ${effortLabel}. Tap to change.`}
              onPress={() => setEffortPickerVisible(true)}
              disabled={disabled}
              style={({ pressed }) => [
                styles.modelPill,
                {
                  backgroundColor: palette.surfaceAlt,
                  borderColor: palette.border,
                  opacity: pressed ? 0.78 : disabled ? 0.55 : 1,
                },
              ]}>
              <Feather name="activity" size={12} color={palette.textSecondary} />
              <Text
                style={[styles.modelPillLabel, { color: palette.textSecondary }]}
                numberOfLines={1}>
                {effortLabel}
              </Text>
              <Feather name="chevron-down" size={12} color={palette.textSecondary} />
            </Pressable>
          ) : null}
        </View>
      ) : null}
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
      {showModelPill ? (
        <ModelPickerSheet
          visible={pickerVisible}
          models={models}
          selectedModelId={selectedModel}
          onSelect={onSelectModel}
          onClose={() => setPickerVisible(false)}
        />
      ) : null}
      {showEffortPill ? (
        <EffortPickerSheet
          visible={effortPickerVisible}
          supportedEfforts={supportedEfforts}
          defaultEffort={defaultEffort}
          selectedEffort={selectedEffort}
          onSelect={onSelectEffort}
          onClose={() => setEffortPickerVisible(false)}
        />
      ) : null}
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
  pillRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingBottom: spacing.xs,
    gap: spacing.sm,
  },
  modelPill: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 4,
    paddingHorizontal: spacing.sm,
    paddingVertical: 4,
    borderRadius: radius.pill,
    borderWidth: 1,
    maxWidth: 200,
  },
  modelPillLabel: {
    ...typography.label,
    fontWeight: '500',
  },
});
