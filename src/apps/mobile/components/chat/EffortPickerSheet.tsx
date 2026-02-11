// ABOUTME: Bottom-sheet modal for selecting the reasoning effort level.
// ABOUTME: Shows effort options derived from the selected model's supported efforts, with a fallback set.

import { Feather } from '@expo/vector-icons';
import type { ChatEffort, ReasoningEffortOption } from '@homie/shared';
import {
  FlatList,
  Modal,
  Pressable,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface EffortItem {
  value: ChatEffort;
  label: string;
  description: string;
}

const FALLBACK_EFFORTS: EffortItem[] = [
  { value: 'low', label: 'Low', description: 'Faster, lighter reasoning.' },
  { value: 'medium', label: 'Medium', description: 'Balanced default.' },
  { value: 'high', label: 'High', description: 'Deeper reasoning.' },
];

interface EffortPickerSheetProps {
  visible: boolean;
  supportedEfforts: ReasoningEffortOption[];
  defaultEffort: string | null;
  selectedEffort: ChatEffort;
  onSelect: (effort: ChatEffort) => void;
  onClose: () => void;
}

export function EffortPickerSheet({
  visible,
  supportedEfforts,
  defaultEffort,
  selectedEffort,
  onSelect,
  onClose,
}: EffortPickerSheetProps) {
  const { palette } = useAppTheme();
  const insets = useSafeAreaInsets();

  const effortItems: EffortItem[] = (() => {
    const entries: EffortItem[] =
      supportedEfforts.length > 0
        ? supportedEfforts.map((e) => ({
            value: e.reasoningEffort as ChatEffort,
            label: e.reasoningEffort.charAt(0).toUpperCase() + e.reasoningEffort.slice(1),
            description: e.description,
          }))
        : FALLBACK_EFFORTS;

    const autoDescription = defaultEffort
      ? `Default: ${defaultEffort.charAt(0).toUpperCase() + defaultEffort.slice(1)}`
      : 'Model default effort.';

    return [
      { value: 'auto' as ChatEffort, label: 'Auto', description: autoDescription },
      ...entries,
    ];
  })();

  const renderItem = ({ item }: { item: EffortItem }) => {
    const isActive = item.value === selectedEffort;
    return (
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={`Select effort ${item.label}`}
        onPress={() => {
          onSelect(item.value);
          onClose();
        }}
        style={({ pressed }) => [
          styles.row,
          {
            backgroundColor: isActive ? palette.surfaceAlt : 'transparent',
            opacity: pressed ? 0.8 : 1,
          },
        ]}>
        <View style={styles.info}>
          <Text
            style={[styles.name, { color: palette.text }]}
            numberOfLines={1}>
            {item.label}
          </Text>
          {item.description ? (
            <Text
              style={[styles.desc, { color: palette.textSecondary }]}
              numberOfLines={2}>
              {item.description}
            </Text>
          ) : null}
        </View>
        {isActive ? (
          <Feather name="check" size={18} color={palette.accent} />
        ) : null}
      </Pressable>
    );
  };

  return (
    <Modal
      visible={visible}
      transparent
      animationType="slide"
      onRequestClose={onClose}>
      <Pressable style={styles.backdrop} onPress={onClose}>
        <View />
      </Pressable>
      <View
        style={[
          styles.sheet,
          {
            backgroundColor: palette.surface,
            paddingBottom: Math.max(insets.bottom, spacing.md),
          },
        ]}>
        <View style={styles.handle}>
          <View style={[styles.handleBar, { backgroundColor: palette.border }]} />
        </View>
        <View style={styles.header}>
          <Text style={[styles.headerTitle, { color: palette.text }]}>Reasoning Effort</Text>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Close effort picker"
            onPress={onClose}
            hitSlop={12}>
            <Feather name="x" size={20} color={palette.textSecondary} />
          </Pressable>
        </View>
        <FlatList
          data={effortItems}
          keyExtractor={(item) => item.value}
          renderItem={renderItem}
          style={styles.list}
          contentContainerStyle={styles.listContent}
        />
      </View>
    </Modal>
  );
}

const styles = StyleSheet.create({
  backdrop: {
    flex: 1,
    backgroundColor: 'rgba(0,0,0,0.35)',
  },
  sheet: {
    borderTopLeftRadius: radius.lg,
    borderTopRightRadius: radius.lg,
    maxHeight: '60%',
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
  },
  headerTitle: {
    ...typography.title,
  },
  list: {
    flexGrow: 0,
  },
  listContent: {
    paddingHorizontal: spacing.md,
  },
  row: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.md,
    borderRadius: radius.md,
    minHeight: 52,
    gap: spacing.sm,
  },
  info: {
    flex: 1,
  },
  name: {
    ...typography.body,
  },
  desc: {
    ...typography.label,
    fontWeight: '400',
    marginTop: 2,
  },
});
