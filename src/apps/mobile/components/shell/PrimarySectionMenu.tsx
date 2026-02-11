import FontAwesome from '@expo/vector-icons/FontAwesome';
import { useRouter } from 'expo-router';
import { memo, useCallback, type ComponentProps } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

export type MobileSection = 'chat' | 'terminals' | 'settings';
export type MobileSectionRoute = '/(tabs)' | '/(tabs)/terminals' | '/(tabs)/settings';

interface MenuItem {
  id: MobileSection;
  label: string;
  subtitle: string;
  icon: ComponentProps<typeof FontAwesome>['name'];
  route: MobileSectionRoute;
}

const MENU_ITEMS: MenuItem[] = [
  { id: 'chat', label: 'Chat', subtitle: 'Conversations', icon: 'comments', route: '/(tabs)' },
  { id: 'terminals', label: 'Terminals', subtitle: 'Running sessions', icon: 'terminal', route: '/(tabs)/terminals' },
  { id: 'settings', label: 'Settings', subtitle: 'Gateway and defaults', icon: 'sliders', route: '/(tabs)/settings' },
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
  const navigateTo = useCallback(
    (route: MobileSectionRoute, selected: boolean) => {
      if (!selected) {
        router.replace(route);
      }
      onNavigate?.();
    },
    [onNavigate, router],
  );

  return (
    <View style={styles.container}>
      {MENU_ITEMS.map((item) => {
        const selected = activeSection === item.id;
        return (
          <MenuItemRow
            key={item.id}
            item={item}
            selected={selected}
            palette={palette}
            onPress={() => {
              navigateTo(item.route, selected);
            }}
          />
        );
      })}
    </View>
  );
}

interface MenuItemRowProps {
  item: MenuItem;
  selected: boolean;
  palette: ReturnType<typeof useAppTheme>['palette'];
  onPress: () => void;
}

const MenuItemRow = memo(function MenuItemRow({ item, selected, palette, onPress }: MenuItemRowProps) {
  return (
    <Pressable
      accessibilityRole="button"
      accessibilityState={{ selected }}
      accessibilityLabel={`${item.label} section`}
      accessibilityHint={`Open ${item.label}`}
      onPress={onPress}
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
      <View style={styles.textColumn}>
        <Text style={[styles.label, { color: selected ? palette.text : palette.textSecondary }]}>
          {item.label}
        </Text>
        <Text style={[styles.subtitle, { color: palette.textSecondary }]}>
          {item.subtitle}
        </Text>
      </View>
      <FontAwesome
        name="angle-right"
        size={12}
        color={selected ? palette.accent : palette.textSecondary}
      />
    </Pressable>
  );
});

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
    minHeight: 56,
    paddingHorizontal: spacing.sm,
  },
  textColumn: {
    flex: 1,
    gap: 2,
  },
  label: {
    ...typography.label,
    fontSize: 13,
  },
  subtitle: {
    ...typography.data,
    fontSize: 11,
  },
});
