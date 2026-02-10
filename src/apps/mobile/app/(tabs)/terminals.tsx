import { Pressable, StyleSheet, Text, View } from 'react-native';

import { ScreenSurface } from '@/components/ui/ScreenSurface';
import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

export default function TerminalsTabScreen() {
  const { palette } = useAppTheme();

  return (
    <ScreenSurface>
      <View style={[styles.container, { backgroundColor: palette.background }]}> 
        <Text style={[styles.title, { color: palette.text }]}>Terminals</Text>
        <View style={[styles.card, { backgroundColor: palette.surface, borderColor: palette.border }]}>
          <Text style={[styles.cardTitle, { color: palette.text }]}>Renderer in progress</Text>
          <Text style={[styles.body, { color: palette.textSecondary }]}>
            Chat ships first. Terminal protocol hooks will be added in this phase, then full terminal rendering lands in
            the next milestone.
          </Text>
          <Pressable
            accessibilityRole="button"
            style={({ pressed }) => [
              styles.button,
              {
                backgroundColor: palette.surfaceAlt,
                borderColor: palette.border,
                opacity: pressed ? 0.86 : 1,
              },
            ]}>
            <Text style={[styles.buttonLabel, { color: palette.text }]}>View Milestone</Text>
          </Pressable>
        </View>
      </View>
    </ScreenSurface>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    paddingTop: spacing.xxl,
    paddingHorizontal: spacing.lg,
    gap: spacing.lg,
  },
  title: {
    ...typography.display,
  },
  card: {
    borderRadius: radius.lg,
    borderWidth: 1,
    padding: spacing.xl,
    gap: spacing.md,
  },
  cardTitle: {
    ...typography.title,
  },
  body: {
    ...typography.body,
    fontWeight: '400',
  },
  button: {
    marginTop: spacing.sm,
    minHeight: 44,
    borderRadius: radius.md,
    borderWidth: 1,
    alignItems: 'center',
    justifyContent: 'center',
  },
  buttonLabel: {
    ...typography.label,
  },
});
