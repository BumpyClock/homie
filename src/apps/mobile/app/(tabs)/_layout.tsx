import { Stack, useRouter, useSegments } from 'expo-router';
import { useEffect, useRef } from 'react';

import { MobileShellDataProvider, useMobileShellData } from '@/components/shell/MobileShellDataContext';

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
