import { useMemo } from 'react';
import { useColorScheme } from '@/components/useColorScheme';
import { palettes } from '@/theme/tokens';

export function useAppTheme() {
  const colorScheme = useColorScheme();

  return useMemo(() => {
    const mode = colorScheme === 'dark' ? 'dark' : 'light';
    return {
      mode,
      palette: palettes[mode],
    };
  }, [colorScheme]);
}
