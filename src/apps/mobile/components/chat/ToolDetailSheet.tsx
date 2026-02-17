// ABOUTME: Bottom sheet modal for displaying full tool call details.
// ABOUTME: Uses React Native Modal for simplicity; handles safe area insets, keyboard avoiding, and nested scrolling.

import { Globe, Search, Terminal, X } from 'lucide-react-native';
import { memo, useCallback, useMemo } from 'react';
import {
  KeyboardAvoidingView,
  Modal,
  Platform,
  Pressable,
  ScrollView,
  StyleSheet,
  Text,
  useWindowDimensions,
  View,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import type { ChatItem } from '@homie/shared';
import { friendlyToolLabelFromItem, normalizeChatToolName } from '@homie/shared';
import { ChatToolDetailCard } from './ChatToolDetailCard';
import { elevation, radius, spacing, touchTarget, type AppPalette, typography } from '@/theme/tokens';
import { motion, triggerMobileHaptic } from '@/theme/motion';

/* ── Icon helper ─────────────────────────────────────────── */

function isRecord(value: unknown): value is Record<string, unknown> {
  return Boolean(value) && typeof value === 'object' && !Array.isArray(value);
}

function getToolIcon(toolName: string | undefined, palette: AppPalette) {
  const size = 18;
  if (toolName === 'web_search') return <Search size={size} color={palette.accent} />;
  if (toolName === 'web_fetch' || toolName === 'browser') return <Globe size={size} color={palette.accent} />;
  return <Terminal size={size} color={palette.textSecondary} />;
}

/* ── Component ───────────────────────────────────────────── */

interface ToolDetailSheetProps {
  visible: boolean;
  item: ChatItem | null;
  palette: AppPalette;
  onClose: () => void;
}

function ToolDetailSheetInner({
  visible,
  item,
  palette,
  onClose,
}: ToolDetailSheetProps) {
  const insets = useSafeAreaInsets();
  const { height: windowHeight } = useWindowDimensions();

  // Calculate max sheet height (60% of screen, minus safe areas)
  const maxSheetHeight = Math.min(windowHeight * 0.6, windowHeight - insets.top - 80);
  // Reserve space for header (~56px) and bottom padding
  const maxContentHeight = maxSheetHeight - 80 - Math.max(insets.bottom, spacing.md);

  const handleClose = useCallback(() => {
    triggerMobileHaptic(motion.haptics.activityToggle);
    onClose();
  }, [onClose]);

  const toolName = useMemo(() => {
    if (!item) return undefined;
    const rawName = isRecord(item.raw) && typeof item.raw.tool === 'string' ? item.raw.tool : item.text;
    return normalizeChatToolName(rawName ?? undefined);
  }, [item]);

  const toolLabel = useMemo(() => {
    if (!item) return 'Tool Details';
    return friendlyToolLabelFromItem(item, 'Tool Details');
  }, [item]);

  const icon = getToolIcon(toolName, palette);

  if (!item) return null;

  return (
    <Modal
      visible={visible}
      transparent
      animationType="slide"
      onRequestClose={handleClose}
      statusBarTranslucent>
      {/* Backdrop */}
      <Pressable
        accessibilityRole="button"
        accessibilityLabel="Close tool details"
        style={[styles.backdrop, { backgroundColor: palette.overlay }]}
        onPress={handleClose}>
        <View />
      </Pressable>

      {/* Sheet */}
      <KeyboardAvoidingView
        behavior={Platform.OS === 'ios' ? 'padding' : 'height'}
        style={styles.keyboardAvoid}>
        <View
          style={[
            styles.sheet,
            elevation.sheet,
            {
              backgroundColor: palette.surface0,
              paddingBottom: Math.max(insets.bottom, spacing.md),
              maxHeight: maxSheetHeight,
            },
          ]}>
          {/* Handle bar */}
          <View style={styles.handle}>
            <View style={[styles.handleBar, { backgroundColor: palette.border }]} />
          </View>

          {/* Header */}
          <View style={styles.header}>
            <View style={styles.headerLeft}>
              {icon}
              <Text
                style={[styles.headerTitle, { color: palette.text }]}
                numberOfLines={1}>
                {toolLabel}
              </Text>
            </View>
            <Pressable
              accessibilityRole="button"
              accessibilityLabel="Close tool details"
              onPress={handleClose}
              hitSlop={12}
              style={({ pressed }) => [
                styles.closeButton,
                { opacity: pressed ? 0.7 : 1 },
              ]}>
              <X size={20} color={palette.textSecondary} />
            </Pressable>
          </View>

          {/* Content */}
          <ScrollView
            style={styles.content}
            contentContainerStyle={[
              styles.contentInner,
              { paddingBottom: spacing.md },
            ]}
            showsVerticalScrollIndicator={false}
            keyboardShouldPersistTaps="handled"
            nestedScrollEnabled>
            <ChatToolDetailCard
              item={item}
              palette={palette}
              nestedScrollEnabled
              maxHeight={maxContentHeight}
            />
          </ScrollView>
        </View>
      </KeyboardAvoidingView>
    </Modal>
  );
}

export const ToolDetailSheet = memo(ToolDetailSheetInner);

/* ── Styles ──────────────────────────────────────────────── */

const styles = StyleSheet.create({
  backdrop: {
    flex: 1,
  },
  keyboardAvoid: {
    position: 'absolute',
    bottom: 0,
    left: 0,
    right: 0,
  },
  sheet: {
    borderTopLeftRadius: radius.lg,
    borderTopRightRadius: radius.lg,
  },
  handle: {
    alignItems: 'center',
    paddingTop: spacing.sm,
    paddingBottom: spacing.xs,
  },
  handleBar: {
    width: 36,
    height: 4,
    borderRadius: 2,
  },
  header: {
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'space-between',
    paddingHorizontal: spacing.lg,
    paddingBottom: spacing.sm,
    gap: spacing.sm,
  },
  headerLeft: {
    flexDirection: 'row',
    alignItems: 'center',
    gap: spacing.sm,
    flex: 1,
  },
  headerTitle: {
    ...typography.title,
    flex: 1,
  },
  closeButton: {
    minWidth: touchTarget.min,
    minHeight: touchTarget.min,
    alignItems: 'center',
    justifyContent: 'center',
  },
  content: {
    flexGrow: 0,
  },
  contentInner: {
    paddingHorizontal: spacing.lg,
  },
});
