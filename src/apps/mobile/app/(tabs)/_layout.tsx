import { Stack } from 'expo-router';

import { MobileShellDataProvider } from '@/components/shell/MobileShellDataContext';

export default function AppLayout() {
  return (
    <MobileShellDataProvider>
      <Stack screenOptions={{ headerShown: false }}>
        <Stack.Screen name="index" />
        <Stack.Screen name="terminals" />
        <Stack.Screen name="settings" />
      </Stack>
    </MobileShellDataProvider>
  );
}
