// ABOUTME: Bottom-sheet modal for selecting the permission mode (Explore, Ask, Execute).
// ABOUTME: Controls how aggressively the agent can act without approval.

import { Check, X } from 'lucide-react-native';
import type { ChatPermissionMode } from '@homie/shared';
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

interface PermissionItem {
  value: ChatPermissionMode;
  label: string;
  description: string;
}

const PERMISSION_OPTIONS: PermissionItem[] = [
  { value: 'explore', label: 'Explore', description: 'Read-only. Agent asks before any changes.' },
  { value: 'ask', label: 'Ask', description: 'Agent asks before risky actions.' },
  { value: 'execute', label: 'Execute', description: 'Agent runs tools without asking.' },
];

interface PermissionPickerSheetProps {
  visible: boolean;
  selectedPermission: ChatPermissionMode;
  onSelect: (permission: ChatPermissionMode) => void;
  onClose: () => void;
}

export function PermissionPickerSheet({
  visible,
  selectedPermission,
  onSelect,
  onClose,
}: PermissionPickerSheetProps) {
  const { palette } = useAppTheme();
  const insets = useSafeAreaInsets();

  const renderItem = ({ item }: { item: PermissionItem }) => {
    const isActive = item.value === selectedPermission;
    return (
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={`Select permission ${item.label}`}
        onPress={() => {
          onSelect(item.value);
          onClose();
        }}
        style={({ pressed }) => [
          styles.row,
          {
            backgroundColor: isActive ? palette.surface1 : 'transparent',
            opacity: pressed ? 0.8 : 1,
          },
        ]}>
        <View style={styles.info}>
          <Text
            style={[styles.name, { color: palette.text }]}
            numberOfLines={1}>
            {item.label}
          </Text>
          <Text
            style={[styles.desc, { color: palette.textSecondary }]}
            numberOfLines={2}>
            {item.description}
          </Text>
        </View>
        {isActive ? (
          <Check size={18} color={palette.accent} />
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
      <Pressable style={[styles.backdrop, { backgroundColor: palette.overlay }]} onPress={onClose}>
        <View />
      </Pressable>
      <View
        style={[
          styles.sheet,
          {
            backgroundColor: palette.surface0,
            paddingBottom: Math.max(insets.bottom, spacing.md),
          },
        ]}>
        <View style={styles.handle}>
          <View style={[styles.handleBar, { backgroundColor: palette.border }]} />
        </View>
        <View style={styles.header}>
          <Text style={[styles.headerTitle, { color: palette.text }]}>Permission</Text>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Close permission picker"
            onPress={onClose}
            hitSlop={12}>
            <X size={20} color={palette.textSecondary} />
          </Pressable>
        </View>
        <FlatList
          data={PERMISSION_OPTIONS}
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
