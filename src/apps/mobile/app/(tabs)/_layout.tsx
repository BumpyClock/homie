import { Tabs, useRouter, useSegments } from 'expo-router';
import { MessageCircle, Settings2, TerminalSquare } from 'lucide-react-native';
import { useEffect, useRef } from 'react';
import { Platform } from 'react-native';

import { MobileShellDataProvider, useMobileShellData } from '@/components/shell/MobileShellDataContext';
import { useAppTheme } from '@/hooks/useAppTheme';
import { spacing, typography } from '@/theme/tokens';

function TabsStartupRoute() {
  const { loadingTarget, hasTarget } = useMobileShellData();
  const router = useRouter();
  const segments = useSegments();
  const routedRef = useRef(false);

  useEffect(() => {
    if (loadingTarget || routedRef.current) return;

    const leaf = segments[segments.length - 1] ?? '(tabs)';
    const isSettingsRoute = leaf === 'settings';
    const isChatRoute = leaf === '(tabs)';

    if (hasTarget && !isChatRoute) {
      router.replace('/(tabs)');
    } else if (!hasTarget && !isSettingsRoute) {
      router.replace('/(tabs)/settings');
    }

    routedRef.current = true;
  }, [hasTarget, loadingTarget, router, segments]);

  return null;
}

export default function AppLayout() {
  const { palette } = useAppTheme();

  return (
    <MobileShellDataProvider>
      <TabsStartupRoute />
      <Tabs
        initialRouteName="index"
        screenOptions={{
          headerShown: false,
          tabBarStyle: {
            backgroundColor: palette.tabBar,
            borderTopColor: palette.border,
            borderTopWidth: 1,
            height: Platform.select({ ios: 84, default: 68 }),
            paddingTop: spacing.xs,
            paddingBottom: Platform.select({ ios: spacing.lg, default: spacing.sm }),
          },
          tabBarLabelStyle: {
            ...typography.label,
            fontSize: 11,
          },
          tabBarActiveTintColor: palette.accent,
          tabBarInactiveTintColor: palette.textSecondary,
        }}>
        <Tabs.Screen
          name="index"
          options={{
            title: 'Chat',
            tabBarIcon: ({ color, size }) => <MessageCircle color={color} size={size} />,
          }}
        />
        <Tabs.Screen
          name="terminals"
          options={{
            title: 'Terminals',
            tabBarIcon: ({ color, size }) => <TerminalSquare color={color} size={size} />,
          }}
        />
        <Tabs.Screen
          name="settings"
          options={{
            title: 'Settings',
            tabBarIcon: ({ color, size }) => <Settings2 color={color} size={size} />,
          }}
        />
      </Tabs>
    </MobileShellDataProvider>
  );
}
