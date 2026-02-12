// ABOUTME: Bottom-sheet modal that displays available AI models for selection.
// ABOUTME: Shows each model's display name and description, highlights the active choice, and dismisses on tap.

import { Check, X } from 'lucide-react-native';
import { modelProviderLabel, type ModelOption } from '@homie/shared';
import {
  Modal,
  Pressable,
  SectionList,
  StyleSheet,
  Text,
  View,
} from 'react-native';
import { useSafeAreaInsets } from 'react-native-safe-area-context';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface ModelPickerSheetProps {
  visible: boolean;
  models: ModelOption[];
  selectedModelId: string | null;
  onSelect: (modelId: string) => void;
  onClose: () => void;
}

interface ModelSection {
  title: string;
  data: ModelOption[];
}

export function ModelPickerSheet({
  visible,
  models,
  selectedModelId,
  onSelect,
  onClose,
}: ModelPickerSheetProps) {
  const { palette } = useAppTheme();
  const insets = useSafeAreaInsets();
  const sections = models.reduce<ModelSection[]>((acc, model) => {
    const title = modelProviderLabel({
      model: model.model,
      provider: (model as { provider?: string }).provider,
    });
    const existing = acc.find((section) => section.title === title);
    if (existing) {
      existing.data.push(model);
    } else {
      acc.push({ title, data: [model] });
    }
    return acc;
  }, []);

  const renderItem = ({ item }: { item: ModelOption }) => {
    const isActive = item.model === selectedModelId || item.id === selectedModelId;
    return (
      <Pressable
        accessibilityRole="button"
        accessibilityLabel={`Select model ${item.displayName}`}
        onPress={() => {
          onSelect(item.model || item.id);
          onClose();
        }}
        style={({ pressed }) => [
          styles.modelRow,
          {
            backgroundColor: isActive ? palette.surface1 : 'transparent',
            opacity: pressed ? 0.8 : 1,
          },
        ]}>
        <View style={styles.modelInfo}>
          <Text
            style={[styles.modelName, { color: palette.text }]}
            numberOfLines={1}>
            {item.displayName || item.model || item.id}
          </Text>
          {item.description ? (
            <Text
              style={[styles.modelDesc, { color: palette.textSecondary }]}
              numberOfLines={2}>
              {item.description}
            </Text>
          ) : null}
        </View>
        {isActive ? (
          <Check size={18} color={palette.accent} />
        ) : null}
      </Pressable>
    );
  };

  const renderSectionHeader = ({ section }: { section: ModelSection }) => (
    <View style={styles.sectionHeader}>
      <Text style={[styles.sectionHeaderText, { color: palette.textSecondary }]}>
        {section.title}
      </Text>
    </View>
  );

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
          <Text style={[styles.headerTitle, { color: palette.text }]}>Model</Text>
          <Pressable
            accessibilityRole="button"
            accessibilityLabel="Close model picker"
            onPress={onClose}
            hitSlop={12}>
            <X size={20} color={palette.textSecondary} />
          </Pressable>
        </View>
        <SectionList
          sections={sections}
          keyExtractor={(item) => item.id}
          renderItem={renderItem}
          renderSectionHeader={renderSectionHeader}
          style={styles.list}
          contentContainerStyle={styles.listContent}
          stickySectionHeadersEnabled
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
    paddingBottom: spacing.sm,
  },
  sectionHeader: {
    paddingHorizontal: spacing.sm,
    paddingTop: spacing.sm,
    paddingBottom: spacing.xs,
  },
  sectionHeaderText: {
    ...typography.caption,
    textTransform: 'uppercase',
    letterSpacing: 0.5,
    fontWeight: '600',
  },
  modelRow: {
    flexDirection: 'row',
    alignItems: 'center',
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.md,
    borderRadius: radius.md,
    minHeight: 52,
    gap: spacing.sm,
  },
  modelInfo: {
    flex: 1,
  },
  modelName: {
    ...typography.body,
  },
  modelDesc: {
    ...typography.label,
    fontWeight: '400',
    marginTop: 2,
  },
});
