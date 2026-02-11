// ABOUTME: Chat message composer with embedded send button and animated focus states.
// ABOUTME: Unified floating card design with model/effort pills above a multiline input, anchored above keyboard via KeyboardStickyView in parent.

import { Feather } from '@expo/vector-icons';
import type { ChatEffort, ModelOption, ReasoningEffortOption } from '@homie/shared';
import * as Haptics from 'expo-haptics';
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  Platform,
  Pressable,
  StyleSheet,
  Text,
  TextInput,
  View,
} from 'react-native';
import Animated, {
  useAnimatedStyle,
  useSharedValue,
  withSpring,
  withTiming,
} from 'react-native-reanimated';

import { EffortPickerSheet } from '@/components/chat/EffortPickerSheet';
import { ModelPickerSheet } from '@/components/chat/ModelPickerSheet';
import { useAppTheme } from '@/hooks/useAppTheme';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { radius, spacing, typography } from '@/theme/tokens';

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

/** Spring config for the send button scale animation */
const SEND_SPRING = { damping: 14, stiffness: 200, mass: 0.6 } as const;

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
  const { palette, mode } = useAppTheme();
  const reducedMotion = useReducedMotion();
  const [value, setValue] = useState('');
  const [focused, setFocused] = useState(false);
  const [pickerVisible, setPickerVisible] = useState(false);
  const [effortPickerVisible, setEffortPickerVisible] = useState(false);
  const inputRef = useRef<TextInput>(null);

  const trimmed = value.trim();
  const canSend = !disabled && !sending && trimmed.length > 0;

  /* ── Animated values ─────────────────────────────────────── */
  const focusProgress = useSharedValue(0);
  const sendScale = useSharedValue(0);
  const sendOpacity = useSharedValue(0);

  useEffect(() => {
    const dur = reducedMotion ? 0 : 180;
    focusProgress.value = withTiming(focused ? 1 : 0, { duration: dur });
  }, [focused, focusProgress, reducedMotion]);

  useEffect(() => {
    if (canSend) {
      sendScale.value = reducedMotion ? 1 : withSpring(1, SEND_SPRING);
      sendOpacity.value = reducedMotion ? 1 : withTiming(1, { duration: 120 });
    } else {
      sendScale.value = reducedMotion ? 0.6 : withSpring(0.6, SEND_SPRING);
      sendOpacity.value = reducedMotion ? 0.35 : withTiming(0.35, { duration: 120 });
    }
  }, [canSend, sendScale, sendOpacity, reducedMotion]);

  const sendButtonStyle = useAnimatedStyle(() => ({
    transform: [{ scale: sendScale.value }],
    opacity: sendOpacity.value,
  }));

  /* ── Model / effort state ────────────────────────────────── */
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

  const submit = useCallback(async () => {
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
  }, [canSend, trimmed, onSend]);

  const safeBottomPadding = Math.max(bottomInset, spacing.sm);

  /* ── Derived palette values ──────────────────────────────── */
  const containerBg = mode === 'dark'
    ? 'rgba(18, 28, 39, 0.96)'
    : 'rgba(255, 255, 255, 0.96)';
  const inputBg = palette.surfaceAlt;
  const focusedBorderColor = palette.accent;
  const restBorderColor = mode === 'dark'
    ? 'rgba(233, 239, 247, 0.10)'
    : 'rgba(16, 26, 39, 0.08)';

  return (
    <View
      style={[
        styles.outerContainer,
        { paddingBottom: safeBottomPadding },
      ]}>
      <View
        style={[
          styles.container,
          {
            backgroundColor: containerBg,
            borderColor: focused ? focusedBorderColor : restBorderColor,
            borderWidth: 1,
          },
        ]}>
        {/* ── Options row: model + effort pills ───────────── */}
        {showModelPill ? (
          <View style={styles.pillRow}>
            <Pressable
              accessibilityRole="button"
              accessibilityLabel={`Model: ${modelLabel ?? 'Default'}. Tap to change.`}
              onPress={() => setPickerVisible(true)}
              disabled={disabled}
              style={({ pressed }) => [
                styles.pill,
                {
                  backgroundColor: mode === 'dark'
                    ? 'rgba(233, 239, 247, 0.07)'
                    : 'rgba(16, 26, 39, 0.05)',
                  opacity: pressed ? 0.7 : disabled ? 0.45 : 1,
                },
              ]}>
              <Feather name="cpu" size={11} color={palette.textSecondary} />
              <Text
                style={[styles.pillLabel, { color: palette.textSecondary }]}
                numberOfLines={1}>
                {modelLabel ?? 'Default'}
              </Text>
              <Feather name="chevron-down" size={10} color={palette.textSecondary} style={{ opacity: 0.6 }} />
            </Pressable>
            {showEffortPill ? (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel={`Effort: ${effortLabel}. Tap to change.`}
                onPress={() => setEffortPickerVisible(true)}
                disabled={disabled}
                style={({ pressed }) => [
                  styles.pill,
                  {
                    backgroundColor: mode === 'dark'
                      ? 'rgba(233, 239, 247, 0.07)'
                      : 'rgba(16, 26, 39, 0.05)',
                    opacity: pressed ? 0.7 : disabled ? 0.45 : 1,
                  },
                ]}>
                <Feather name="activity" size={11} color={palette.textSecondary} />
                <Text
                  style={[styles.pillLabel, { color: palette.textSecondary }]}
                  numberOfLines={1}>
                  {effortLabel}
                </Text>
                <Feather name="chevron-down" size={10} color={palette.textSecondary} style={{ opacity: 0.6 }} />
              </Pressable>
            ) : null}
          </View>
        ) : null}

        {/* ── Input area with embedded send button ────────── */}
        <View style={styles.inputRow}>
          <TextInput
            ref={inputRef}
            value={value}
            onChangeText={setValue}
            onFocus={() => setFocused(true)}
            onBlur={() => setFocused(false)}
            editable={!disabled && !sending}
            placeholder="Message…"
            placeholderTextColor={palette.textSecondary}
            multiline
            style={[
              styles.input,
              { color: palette.text },
            ]}
          />
          <AnimatedPressable
            accessibilityRole="button"
            accessibilityLabel={sending ? 'Sending message' : 'Send message'}
            onPress={submit}
            disabled={!canSend}
            style={[
              styles.sendButton,
              sendButtonStyle,
              {
                backgroundColor: canSend ? palette.accent : (
                  mode === 'dark'
                    ? 'rgba(233, 239, 247, 0.08)'
                    : 'rgba(16, 26, 39, 0.06)'
                ),
              },
            ]}>
            <Feather
              name="arrow-up"
              size={16}
              color={canSend ? '#FFFFFF' : palette.textSecondary}
              style={canSend ? undefined : { opacity: 0.5 }}
            />
          </AnimatedPressable>
        </View>
      </View>

      {/* ── Bottom sheets (rendered outside container) ──── */}
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
  outerContainer: {
    paddingHorizontal: spacing.sm,
    paddingTop: spacing.xs,
  },
  container: {
    borderRadius: radius.lg + 4,
    overflow: 'hidden',
    paddingHorizontal: spacing.sm,
    paddingTop: spacing.sm,
    paddingBottom: spacing.sm,
  },
  pillRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: spacing.xs,
    paddingBottom: spacing.xs + 2,
    gap: spacing.xs + 2,
  },
  pill: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: 3,
    paddingHorizontal: spacing.sm,
    paddingVertical: 3,
    borderRadius: radius.pill,
    maxWidth: 180,
  },
  pillLabel: {
    fontSize: 12,
    lineHeight: 16,
    fontWeight: '500',
    letterSpacing: 0.1,
  },
  inputRow: {
    flexDirection: 'row',
    alignItems: 'flex-end',
    gap: spacing.xs,
  },
  input: {
    ...typography.body,
    flex: 1,
    fontSize: 16,
    minHeight: 36,
    maxHeight: 120,
    paddingHorizontal: spacing.sm,
    paddingTop: Platform.OS === 'ios' ? spacing.sm : spacing.xs,
    paddingBottom: Platform.OS === 'ios' ? spacing.sm : spacing.xs,
    textAlignVertical: 'top',
  },
  sendButton: {
    alignItems: 'center',
    justifyContent: 'center',
    width: 32,
    height: 32,
    borderRadius: 16,
    marginBottom: Platform.OS === 'ios' ? 2 : 0,
  },
});
