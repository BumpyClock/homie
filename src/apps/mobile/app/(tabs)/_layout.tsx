import { Stack, usePathname, useRouter } from 'expo-router';
import { useEffect, useRef } from 'react';

import { MobileShellDataProvider } from '@/components/shell/MobileShellDataContext';
import { useMobileShellData } from '@/components/shell/MobileShellDataContext';

function TabsStartupRoute() {
  const { loadingTarget, hasTarget } = useMobileShellData();
  const router = useRouter();
  const pathname = usePathname();
  const routedRef = useRef(false);

  useEffect(() => {
    if (loadingTarget || routedRef.current) return;

    const isSettingsRoute = pathname === '/settings' || pathname.endsWith('/settings');
    const isChatRoute =
      pathname === '/' ||
      pathname === '/index' ||
      pathname === '/(tabs)' ||
      pathname.endsWith('/(tabs)');

    if (hasTarget && !isChatRoute) {
      router.replace('/(tabs)');
    } else if (!hasTarget && !isSettingsRoute) {
      router.replace('/(tabs)/settings');
    }

    routedRef.current = true;
  }, [hasTarget, loadingTarget, pathname, router]);

  return null;
}

export default function AppLayout() {
  return (
    <MobileShellDataProvider>
      <TabsStartupRoute />
      <Stack screenOptions={{ headerShown: false }}>
        <Stack.Screen name="index" />
        <Stack.Screen name="terminals" />
        <Stack.Screen name="settings" />
      </Stack>
    </MobileShellDataProvider>
  );
}
