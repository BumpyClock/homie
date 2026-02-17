// ABOUTME: Chat message composer with embedded send button and animated focus states.
// ABOUTME: Unified floating card design with model/effort pills above a multiline input, anchored above keyboard via KeyboardStickyView in parent.

import { Activity, ArrowUp, ChevronDown, Cpu, Shield, Sparkles, Square, Users, X } from 'lucide-react-native';
import type { ChatEffort, ChatPermissionMode, CollaborationModeOption, ModelOption, ReasoningEffortOption, SkillOption } from '@homie/shared';
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
  FadeIn,
  useAnimatedStyle,
  useSharedValue,
  withRepeat,
  withSequence,
  withSpring,
  withTiming,
} from 'react-native-reanimated';

import { CollaborationModePickerSheet } from '@/components/chat/CollaborationModePickerSheet';
import { EffortPickerSheet } from '@/components/chat/EffortPickerSheet';
import { ModelPickerSheet } from '@/components/chat/ModelPickerSheet';
import { PermissionPickerSheet } from '@/components/chat/PermissionPickerSheet';
import { useAppTheme } from '@/hooks/useAppTheme';
import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion } from '@/theme/motion';
import { palettes, radius, spacing, typography } from '@/theme/tokens';

const AnimatedPressable = Animated.createAnimatedComponent(Pressable);

/** Spring config for the send button scale animation */
const SEND_SPRING = { damping: 14, stiffness: 200, mass: 0.6 } as const;
const SLASH_TRIGGER_REGEX = /(?:^|\s)\/([\w-]*)$/;

interface ChatComposerProps {
  disabled?: boolean;
  sending?: boolean;
  isRunning?: boolean;
  bottomInset?: number;
  models?: ModelOption[];
  skills?: SkillOption[];
  collaborationModes?: CollaborationModeOption[];
  selectedModel?: string | null;
  selectedEffort?: ChatEffort;
  selectedPermission?: ChatPermissionMode;
  selectedCollaborationMode?: string | null;
  onSelectModel?: (modelId: string) => void;
  onSelectEffort?: (effort: ChatEffort) => void;
  onSelectPermission?: (permission: ChatPermissionMode) => void;
  onSelectCollaborationMode?: (modeId: string) => void;
  queuedMessage?: string | null;
  onClearQueue?: () => void;
  onSend: (message: string) => Promise<void>;
  onStop?: () => void;
}

export function ChatComposer({
  disabled = false,
  sending = false,
  isRunning = false,
  bottomInset = 0,
  models = [],
  skills = [],
  collaborationModes = [],
  selectedModel = null,
  selectedEffort = 'auto',
  selectedPermission = 'ask',
  selectedCollaborationMode = null,
  onSelectModel,
  onSelectEffort,
  onSelectPermission,
  queuedMessage = null,
  onClearQueue,
  onSelectCollaborationMode,
  onSend,
  onStop,
}: ChatComposerProps) {
  const { palette } = useAppTheme();
  const reducedMotion = useReducedMotion();
  const [value, setValue] = useState('');
  const [focused, setFocused] = useState(false);
  const [pickerVisible, setPickerVisible] = useState(false);
  const [effortPickerVisible, setEffortPickerVisible] = useState(false);
  const [permissionPickerVisible, setPermissionPickerVisible] = useState(false);
  const [collaborationPickerVisible, setCollaborationPickerVisible] = useState(false);
  const [selection, setSelection] = useState({ start: 0, end: 0 });
  const [slashTrigger, setSlashTrigger] = useState<{
    start: number;
    cursor: number;
    query: string;
  } | null>(null);
  const inputRef = useRef<TextInput>(null);

  const trimmed = value.trim();
  const canSend = !disabled && !sending && trimmed.length > 0;
  const showStop = isRunning && !sending;

  /* ── Animated values ─────────────────────────────────────── */
  const focusProgress = useSharedValue(0);
  const sendScale = useSharedValue(0);
  const sendOpacity = useSharedValue(0);
  const slashMenuProgress = useSharedValue(0);
  const borderPulse = useSharedValue(0);

  useEffect(() => {
    const dur = reducedMotion ? 0 : 180;
    focusProgress.value = withTiming(focused ? 1 : 0, { duration: dur });
  }, [focused, focusProgress, reducedMotion]);

  useEffect(() => {
    const active = canSend || showStop;
    if (active) {
      sendScale.value = reducedMotion ? 1 : withSpring(1, SEND_SPRING);
      sendOpacity.value = reducedMotion ? 1 : withTiming(1, { duration: 120 });
    } else {
      sendScale.value = reducedMotion ? 0.6 : withSpring(0.6, SEND_SPRING);
      sendOpacity.value = reducedMotion ? 0.35 : withTiming(0.35, { duration: 120 });
    }
  }, [canSend, showStop, sendScale, sendOpacity, reducedMotion]);

  useEffect(() => {
    if (sending && !reducedMotion) {
      borderPulse.value = withRepeat(
        withSequence(
          withTiming(1, { duration: 800 }),
          withTiming(0, { duration: 800 }),
        ),
        -1,
      );
    } else {
      borderPulse.value = withTiming(0, { duration: 200 });
    }
  }, [sending, borderPulse, reducedMotion]);

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

  /* ── Permission / collaboration state ──────────────────── */
  const permissionLabel = selectedPermission.charAt(0).toUpperCase() + selectedPermission.slice(1);
  const showPermissionPill = onSelectPermission != null;

  const activeCollaboration = selectedCollaborationMode
    ? collaborationModes.find((m) => m.id === selectedCollaborationMode || m.mode === selectedCollaborationMode) ?? null
    : null;
  const collaborationLabel = activeCollaboration?.label ?? null;
  const showCollaborationPill = collaborationModes.length > 0 && onSelectCollaborationMode;

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

  const containerBorderStyle = useAnimatedStyle(() => ({
    borderColor: focused
      ? focusedBorderColor
      : sending
        ? `rgba(10, 120, 232, ${0.15 + borderPulse.value * 0.25})`
        : restBorderColor,
  }));

  return (
    <View
      style={[
        styles.outerContainer,
        { paddingBottom: safeBottomPadding },
      ]}>
      <Animated.View
        style={[
          styles.container,
          {
            backgroundColor: containerBg,
            borderWidth: 1,
          },
          containerBorderStyle,
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
            {showPermissionPill ? (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel={`Permission: ${permissionLabel}. Tap to change.`}
                onPress={() => setPermissionPickerVisible(true)}
                disabled={disabled}
                style={({ pressed }) => [
                  styles.pill,
                  {
                    backgroundColor: pillBackground,
                    opacity: pressed ? 0.7 : disabled ? 0.45 : 1,
                  },
                ]}>
                <Shield size={11} color={palette.textSecondary} />
                <Text
                  style={[styles.pillLabel, { color: palette.textSecondary }]}
                  numberOfLines={1}>
                  {permissionLabel}
                </Text>
                <ChevronDown size={10} color={palette.textSecondary} style={{ opacity: 0.6 }} />
              </Pressable>
            ) : null}
            {showCollaborationPill ? (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel={`Collaboration: ${collaborationLabel ?? 'Default'}. Tap to change.`}
                onPress={() => setCollaborationPickerVisible(true)}
                disabled={disabled}
                style={({ pressed }) => [
                  styles.pill,
                  {
                    backgroundColor: pillBackground,
                    opacity: pressed ? 0.7 : disabled ? 0.45 : 1,
                  },
                ]}>
                <Users size={11} color={palette.textSecondary} />
                <Text
                  style={[styles.pillLabel, { color: palette.textSecondary }]}
                  numberOfLines={1}>
                  {collaborationLabel ?? 'Default'}
                </Text>
                <ChevronDown size={10} color={palette.textSecondary} style={{ opacity: 0.6 }} />
              </Pressable>
            ) : null}
          </View>
        ) : null}

        {/* ── Queued message indicator ─────────────────────── */}
        {queuedMessage ? (
          <Animated.View
            entering={reducedMotion ? undefined : FadeIn.duration(motion.duration.fast)}
            accessibilityRole="alert"
            accessibilityLabel="Message queued. Will send when agent turn completes. Tap X to cancel."
            style={[
              styles.queueBanner,
              { backgroundColor: palette.warningDim },
            ]}>
            <Text
              style={[styles.queueLabel, { color: palette.warning }]}
              numberOfLines={1}>
              Queued — will send when turn completes
            </Text>
            {onClearQueue ? (
              <Pressable
                accessibilityRole="button"
                accessibilityLabel="Cancel queued message"
                onPress={onClearQueue}
                hitSlop={8}
                style={styles.queueDismiss}>
                <X size={12} color={palette.warning} />
              </Pressable>
            ) : null}
          </Animated.View>
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
            accessibilityLabel={
              showStop ? 'Stop generation' : sending ? 'Sending message' : 'Send message'
            }
            onPress={() => {
              if (showStop) {
                if (Platform.OS !== 'web') {
                  Haptics.notificationAsync(Haptics.NotificationFeedbackType.Warning);
                }
                onStop?.();
              } else {
                void submit();
              }
            }}
            disabled={!canSend && !showStop}
            style={[
              styles.sendButton,
              sendButtonStyle,
              {
                backgroundColor: showStop
                  ? palette.danger
                  : canSend
                    ? palette.accent
                    : disabledSendBackground,
              },
            ]}>
            {showStop ? (
              <Square size={12} color={palettes.light.surface0} fill={palettes.light.surface0} />
            ) : (
              <ArrowUp
                size={16}
                color={canSend ? palettes.light.surface0 : palette.textSecondary}
                style={canSend ? undefined : { opacity: 0.5 }}
              />
            )}
          </AnimatedPressable>
        </View>
      </Animated.View>

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
      {showPermissionPill ? (
        <PermissionPickerSheet
          visible={permissionPickerVisible}
          selectedPermission={selectedPermission}
          onSelect={onSelectPermission}
          onClose={() => setPermissionPickerVisible(false)}
        />
      ) : null}
      {showCollaborationPill ? (
        <CollaborationModePickerSheet
          visible={collaborationPickerVisible}
          modes={collaborationModes}
          selectedModeId={selectedCollaborationMode}
          onSelect={onSelectCollaborationMode}
          onClose={() => setCollaborationPickerVisible(false)}
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
  queueBanner: {
    flexDirection: 'row',
    alignItems: 'center',
    borderRadius: radius.sm,
    paddingVertical: spacing.xs,
    paddingHorizontal: spacing.sm,
    marginHorizontal: spacing.xs,
    marginBottom: spacing.xs,
  },
  queueLabel: {
    ...typography.caption,
    flex: 1,
  },
  queueDismiss: {
    marginLeft: spacing.xs,
    padding: spacing.micro,
  },
});
