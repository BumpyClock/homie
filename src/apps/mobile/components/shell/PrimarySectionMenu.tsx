import FontAwesome from '@expo/vector-icons/FontAwesome';
import type { ComponentProps } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';

import { useAppTheme } from '@/hooks/useAppTheme';
import { radius, spacing, typography } from '@/theme/tokens';

export type MobileSection = 'chat' | 'terminals' | 'settings';

interface MenuItem {
  id: MobileSection;
  label: string;
  icon: ComponentProps<typeof FontAwesome>['name'];
}

const MENU_ITEMS: MenuItem[] = [
  { id: 'chat', label: 'Chat', icon: 'comments' },
  { id: 'terminals', label: 'Terminals', icon: 'terminal' },
  { id: 'settings', label: 'Settings', icon: 'sliders' },
];

interface PrimarySectionMenuProps {
  activeSection: MobileSection;
  onSelect: (section: MobileSection) => void;
}

export function PrimarySectionMenu({ activeSection, onSelect }: PrimarySectionMenuProps) {
  const { palette } = useAppTheme();

  return (
    <View style={styles.container}>
      {MENU_ITEMS.map((item) => {
        const selected = activeSection === item.id;
        return (
          <Pressable
            key={item.id}
            accessibilityRole="button"
            accessibilityState={{ selected }}
            onPress={() => onSelect(item.id)}
            style={({ pressed }) => [
              styles.item,
              {
                backgroundColor: selected ? palette.surfaceAlt : palette.surface,
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
