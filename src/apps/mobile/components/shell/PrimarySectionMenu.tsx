import FontAwesome from '@expo/vector-icons/FontAwesome';
import { useRouter } from 'expo-router';
import type { ComponentProps } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

export type MobileSection = 'chat' | 'terminals' | 'settings';
export type MobileSectionRoute = '/(tabs)' | '/(tabs)/terminals' | '/(tabs)/settings';

interface MenuItem {
  id: MobileSection;
  label: string;
  icon: ComponentProps<typeof FontAwesome>['name'];
  route: MobileSectionRoute;
}

const MENU_ITEMS: MenuItem[] = [
  { id: 'chat', label: 'Chat', icon: 'comments', route: '/(tabs)' },
  { id: 'terminals', label: 'Terminals', icon: 'terminal', route: '/(tabs)/terminals' },
  { id: 'settings', label: 'Settings', icon: 'sliders', route: '/(tabs)/settings' },
];

interface PrimarySectionMenuProps {
  activeSection: MobileSection;
  onNavigate?: () => void;
}

export const MOBILE_SECTION_TITLES: Record<MobileSection, string> = {
  chat: 'Chat',
  terminals: 'Terminals',
  settings: 'Settings',
};

export const MOBILE_SECTION_ROUTES: Record<MobileSection, MobileSectionRoute> = {
  chat: '/(tabs)',
  terminals: '/(tabs)/terminals',
  settings: '/(tabs)/settings',
};

export function PrimarySectionMenu({ activeSection, onNavigate }: PrimarySectionMenuProps) {
  const { palette } = useAppTheme();
  const router = useRouter();

  return (
    <View style={styles.container}>
      {MENU_ITEMS.map((item) => {
        const selected = activeSection === item.id;
        return (
          <Pressable
            key={item.id}
            accessibilityRole="button"
            accessibilityState={{ selected }}
            onPress={() => {
              if (!selected) {
                router.replace(item.route);
              }
              onNavigate?.();
            }}
            style={({ pressed }) => [
              styles.item,
              {
                backgroundColor: selected ? palette.surface1 : palette.surface0,
                borderColor: selected ? palette.accent : palette.border,
                opacity: pressed ? 0.86 : 1,
              },
            ]}>
            <FontAwesome
              name={item.icon}
              size={14}
              color={selected ? palette.accent : palette.textSecondary}
            />
            <Text style={[styles.label, { color: selected ? palette.text : palette.textSecondary }]}>
              {item.label}
            </Text>
          </Pressable>
        );
      })}
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    gap: spacing.xs,
    paddingHorizontal: spacing.md,
    paddingTop: spacing.md,
  },
  item: {
    alignItems: 'center',
    borderRadius: radius.md,
    borderWidth: 1,
    flexDirection: 'row',
    gap: spacing.sm,
    minHeight: 44,
    paddingHorizontal: spacing.sm,
  },
  label: {
    ...typography.label,
    fontSize: 13,
  },
});
