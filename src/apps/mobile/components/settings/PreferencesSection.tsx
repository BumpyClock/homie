import { SlidersHorizontal } from 'lucide-react-native';
import { ScrollView, StyleSheet, Text, View } from 'react-native';

import { LabeledValueRow } from '@/components/ui/LabeledValueRow';
import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

interface PreferencesSectionProps {
  mode: string;
  selectedModelLabel: string;
  availableModelLabels: string[];
}

export function PreferencesSection({
  mode,
  selectedModelLabel,
  availableModelLabels,
}: PreferencesSectionProps) {
  const { palette } = useAppTheme();

  return (
    <ScrollView
      style={styles.scroll}
      contentContainerStyle={styles.scrollContent}
      showsVerticalScrollIndicator={false}
    >
      <View style={[styles.card, { backgroundColor: palette.surface0, borderColor: palette.border }]}>
        <View style={styles.cardHeader}>
          <SlidersHorizontal size={14} color={palette.accent} />
          <Text style={[styles.cardTitle, { color: palette.text }]}>Preferences</Text>
        </View>
        <LabeledValueRow label="Theme" value={mode} />
        <LabeledValueRow label="Selected model" value={selectedModelLabel} />
        <LabeledValueRow
          label="Available models"
          value={availableModelLabels.length > 0 ? `${availableModelLabels.length}` : 'Loading...'}
        />
        {availableModelLabels.length > 0 ? (
          <View style={[styles.modelList, { borderColor: palette.border, backgroundColor: palette.surface1 }]}>
            {availableModelLabels.map((label) => (
              <Text key={label} numberOfLines={1} style={[styles.modelListItem, { color: palette.textSecondary }]}>
                {`\u2022 ${label}`}
              </Text>
            ))}
          </View>
        ) : null}
        <LabeledValueRow label="Gateway path" value="/ws" mono last />
      </View>
    </ScrollView>
  );
}

const styles = StyleSheet.create({
  scroll: {
    flex: 1,
  },
  scrollContent: {
    gap: spacing.md,
    paddingBottom: spacing.xxxl,
  },
  card: {
    borderRadius: radius.lg,
    borderWidth: 1,
    gap: spacing.md,
    padding: spacing.lg,
  },
  cardHeader: {
    alignItems: 'center',
    flexDirection: 'row',
    gap: spacing.sm,
  },
  cardTitle: {
    ...typography.title,
    fontSize: 18,
  },
  modelList: {
    borderRadius: radius.md,
    borderWidth: 1,
    gap: spacing.xs,
    paddingHorizontal: spacing.md,
    paddingVertical: spacing.sm,
  },
  modelListItem: {
    ...typography.data,
    fontSize: 12,
  },
});
