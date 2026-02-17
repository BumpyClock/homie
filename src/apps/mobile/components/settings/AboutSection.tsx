import { ScrollView, StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

export function AboutSection() {
  const { palette } = useAppTheme();

  return (
    <ScrollView
      style={styles.scroll}
      contentContainerStyle={styles.scrollContent}
      showsVerticalScrollIndicator={false}
    >
      <View style={[styles.helpCard, { backgroundColor: palette.surface1, borderColor: palette.border }]}>
        <Text style={[styles.helpTitle, { color: palette.text }]}>Tips</Text>
        <Text style={[styles.helpItem, { color: palette.textSecondary }]}>
          - Prefer `wss://` for remote access.
        </Text>
        <Text style={[styles.helpItem, { color: palette.textSecondary }]}>
          - Include `/ws` at the end of your gateway URL.
        </Text>
        <Text style={[styles.helpItem, { color: palette.textSecondary }]}>
          - Update target here any time to switch machines.
        </Text>
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
  helpCard: {
    borderRadius: radius.md,
    borderWidth: 1,
    gap: spacing.xs,
    padding: spacing.md,
  },
  helpTitle: {
    ...typography.caption,
    textTransform: 'uppercase',
  },
  helpItem: {
    ...typography.caption,
    fontSize: 12,
    fontWeight: '500',
  },
});
