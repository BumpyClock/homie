// ABOUTME: Chat message composer with embedded send button and animated focus states.
// ABOUTME: Unified floating card design with model/effort pills above a multiline input, anchored above keyboard via KeyboardStickyView in parent.

import { Activity, ArrowUp, ChevronDown, Cpu, Sparkles } from 'lucide-react-native';
import type { ChatEffort, ModelOption, ReasoningEffortOption, SkillOption } from '@homie/shared';
import * as Haptics from 'expo-haptics';
import { useCallback, useEffect, useRef, useState } from 'react';
import {
  Platform,
  Pressable,
  ScrollView,
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
import { palettes, radius, spacing, typography } from '@/theme/tokens';

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

/** Spring config for the send button scale animation */
const SEND_SPRING = { damping: 14, stiffness: 200, mass: 0.6 } as const;
const SLASH_TRIGGER_REGEX = /(?:^|\s)\/([\w-]*)$/;

interface ChatComposerProps {
  disabled?: boolean;
  sending?: boolean;
  bottomInset?: number;
  models?: ModelOption[];
  skills?: SkillOption[];
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
  skills = [],
  selectedModel = null,
  selectedEffort = 'auto',
  onSelectModel,
  onSelectEffort,
  onSend,
}: ChatComposerProps) {
  const { palette } = useAppTheme();
  const reducedMotion = useReducedMotion();
  const [value, setValue] = useState('');
  const [focused, setFocused] = useState(false);
  const [pickerVisible, setPickerVisible] = useState(false);
  const [effortPickerVisible, setEffortPickerVisible] = useState(false);
  const [selection, setSelection] = useState({ start: 0, end: 0 });
  const [slashTrigger, setSlashTrigger] = useState<{
    start: number;
    cursor: number;
    query: string;
  } | null>(null);
  const inputRef = useRef<TextInput>(null);

  const trimmed = value.trim();
  const canSend = !disabled && !sending && trimmed.length > 0;

  /* ── Animated values ─────────────────────────────────────── */
  const focusProgress = useSharedValue(0);
  const sendScale = useSharedValue(0);
  const sendOpacity = useSharedValue(0);
  const slashMenuProgress = useSharedValue(0);

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

  const slashMenuStyle = useAnimatedStyle(() => ({
    opacity: slashMenuProgress.value,
    transform: [{ translateY: (1 - slashMenuProgress.value) * -6 }],
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
  const slashSkillOptions = (() => {
    if (!slashTrigger) return [];
    const query = slashTrigger.query.toLowerCase().trim();
    const filtered = query
      ? skills.filter((skill) => skill.name.toLowerCase().includes(query))
      : skills;
    return filtered.slice(0, 8);
  })();
  const slashMenuOpen = !!slashTrigger && slashSkillOptions.length > 0;

  const updateSlashTrigger = useCallback(
    (nextValue: string, cursor: number) => {
      const safeCursor = Math.max(0, Math.min(cursor, nextValue.length));
      const textBefore = nextValue.slice(0, safeCursor);
      const slashMatch = textBefore.match(SLASH_TRIGGER_REGEX);
      if (!slashMatch) {
        setSlashTrigger(null);
        return;
      }
      const start = textBefore.lastIndexOf('/');
      if (start < 0) {
        setSlashTrigger(null);
        return;
      }
      setSlashTrigger({
        start,
        cursor: safeCursor,
        query: slashMatch[1] ?? '',
      });
    },
    [],
  );

  const insertSkillReference = useCallback(
    (skillName: string) => {
      if (!slashTrigger) return;
      const before = value.slice(0, slashTrigger.start);
      const after = value.slice(slashTrigger.cursor);
      const insertion = `$${skillName} `;
      const nextValue = `${before}${insertion}${after}`;
      const nextCursor = before.length + insertion.length;
      setValue(nextValue);
      setSelection({ start: nextCursor, end: nextCursor });
      setSlashTrigger(null);
      requestAnimationFrame(() => {
        inputRef.current?.focus();
      });
    },
    [slashTrigger, value],
  );

  const submit = useCallback(async () => {
    if (!canSend) return;
    const draft = trimmed;
    setValue('');
    setSlashTrigger(null);
    if (Platform.OS !== 'web') {
      Haptics.impactAsync(Haptics.ImpactFeedbackStyle.Light);
    }
    try {
      await onSend(draft);
    } catch {
      setValue(draft);
    }
  }, [canSend, trimmed, onSend]);

  useEffect(() => {
    const duration = reducedMotion ? 0 : 150;
    slashMenuProgress.value = withTiming(slashMenuOpen ? 1 : 0, { duration });
  }, [reducedMotion, slashMenuOpen, slashMenuProgress]);

  const safeBottomPadding = Math.max(bottomInset, spacing.sm);

  /* ── Derived palette values ──────────────────────────────── */
  const containerBg = palette.tabBar;
  const focusedBorderColor = palette.accent;
  const restBorderColor = palette.border;
  const pillBackground = palette.surface1;
  const disabledSendBackground = palette.surface2;

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
                  backgroundColor: pillBackground,
                  opacity: pressed ? 0.7 : disabled ? 0.45 : 1,
                },
              ]}>
              <Cpu size={11} color={palette.textSecondary} />
              <Text
                style={[styles.pillLabel, { color: palette.textSecondary }]}
                numberOfLines={1}>
                {modelLabel ?? 'Default'}
              </Text>
              <ChevronDown size={10} color={palette.textSecondary} style={{ opacity: 0.6 }} />
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
                    backgroundColor: pillBackground,
                    opacity: pressed ? 0.7 : disabled ? 0.45 : 1,
                  },
                ]}>
                <Activity size={11} color={palette.textSecondary} />
                <Text
                  style={[styles.pillLabel, { color: palette.textSecondary }]}
                  numberOfLines={1}>
                  {effortLabel}
                </Text>
                <ChevronDown size={10} color={palette.textSecondary} style={{ opacity: 0.6 }} />
              </Pressable>
            ) : null}
          </View>
        ) : null}

        {/* ── Slash skill quick menu ───────────────────────── */}
        {slashTrigger ? (
          <Animated.View
            style={[
              styles.slashMenu,
              slashMenuStyle,
              {
                backgroundColor: palette.surface0,
                borderColor: palette.border,
              },
            ]}>
            {slashSkillOptions.length > 0 ? (
              <ScrollView
                style={styles.slashScroll}
                keyboardShouldPersistTaps="handled"
                nestedScrollEnabled>
                {slashSkillOptions.map((skill) => (
                  <Pressable
                    key={skill.name}
                    accessibilityRole="button"
                    accessibilityLabel={`Insert skill ${skill.name}`}
                    onPress={() => insertSkillReference(skill.name)}
                    style={({ pressed }) => [
                      styles.slashItem,
                      { backgroundColor: pressed ? palette.surface1 : 'transparent' },
                    ]}>
                    <Sparkles size={14} color={palette.textSecondary} />
                    <View style={styles.slashItemText}>
                      <Text style={[styles.slashItemTitle, { color: palette.text }]}>/{skill.name}</Text>
                      {skill.description ? (
                        <Text style={[styles.slashItemBody, { color: palette.textSecondary }]} numberOfLines={1}>
                          {skill.description}
                        </Text>
                      ) : null}
                    </View>
                  </Pressable>
                ))}
              </ScrollView>
            ) : (
              <Text style={[styles.slashEmpty, { color: palette.textSecondary }]}>
                No matching skills.
              </Text>
            )}
          </Animated.View>
        ) : null}

        {/* ── Input area with embedded send button ────────── */}
        <View style={styles.inputRow}>
          <TextInput
            ref={inputRef}
            value={value}
            selection={selection}
            onChangeText={(next) => {
              setValue(next);
              const cursor = Math.min(selection.end, next.length);
              updateSlashTrigger(next, cursor);
            }}
            onSelectionChange={(event) => {
              const nextSelection = event.nativeEvent.selection;
              setSelection(nextSelection);
              updateSlashTrigger(value, nextSelection.start);
            }}
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
                backgroundColor: canSend ? palette.accent : disabledSendBackground,
              },
            ]}>
            <ArrowUp
              size={16}
              color={canSend ? palettes.light.surface0 : palette.textSecondary}
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
  slashMenu: {
    marginBottom: spacing.xs,
    borderWidth: 1,
    borderRadius: radius.md,
    maxHeight: 180,
    overflow: 'hidden',
  },
  slashScroll: {
    maxHeight: 180,
  },
  slashItem: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacing.xs,
    minHeight: 44,
    paddingHorizontal: spacing.sm,
    paddingVertical: spacing.xs,
  },
  slashItemText: {
    flex: 1,
    minWidth: 0,
  },
  slashItemTitle: {
    ...typography.label,
    fontWeight: '600',
  },
  slashItemBody: {
    ...typography.caption,
  },
  slashEmpty: {
    ...typography.caption,
    paddingHorizontal: spacing.sm,
    paddingVertical: spacing.sm,
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
