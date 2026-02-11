import { Link, Stack } from 'expo-router';
import { Pressable, StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

export default function NotFoundScreen() {
  const { palette } = useAppTheme();

  return (
    <>
      <Stack.Screen options={{ title: 'Not found' }} />
      <View style={[styles.container, { backgroundColor: palette.background }]}> 
        <Text style={[styles.title, { color: palette.text }]}>This route does not exist</Text>
        <Link href="/" asChild>
          <Pressable style={[styles.link, { borderColor: palette.border, backgroundColor: palette.surface0 }]}> 
            <Text style={[styles.linkText, { color: palette.accent }]}>Go back to chat</Text>
          </Pressable>
        </Link>
      </View>
    </>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
    padding: spacing.xl,
    gap: spacing.lg,
  },
  title: {
    ...typography.title,
    textAlign: 'center',
  },
  link: {
    borderRadius: radius.md,
    borderWidth: 1,
    minHeight: 44,
    minWidth: 180,
    alignItems: 'center',
    justifyContent: 'center',
    paddingHorizontal: spacing.lg,
  },
  linkText: {
    ...typography.label,
  },
});
