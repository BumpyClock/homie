export type AppPalette = {
  background: string;
  surface: string;
  surfaceAlt: string;
  text: string;
  textSecondary: string;
  accent: string;
  success: string;
  warning: string;
  danger: string;
  border: string;
  tabBar: string;
};

export const palettes: Record<'light' | 'dark', AppPalette> = {
  light: {
    background: '#F3F7FC',
    surface: '#FFFFFF',
    surfaceAlt: '#E9F0F8',
    text: '#101A27',
    textSecondary: '#5E6B7C',
    accent: '#0A84FF',
    success: '#22A06B',
    warning: '#C27A15',
    danger: '#CE4257',
    border: 'rgba(16, 26, 39, 0.10)',
    tabBar: 'rgba(255, 255, 255, 0.92)',
  },
  dark: {
    background: '#0D131B',
    surface: '#121C27',
    surfaceAlt: '#1A2735',
    text: '#E9EFF7',
    textSecondary: '#9BA8B9',
    accent: '#4FA4FF',
    success: '#43C38A',
    warning: '#F0B44D',
    danger: '#F06A80',
    border: 'rgba(233, 239, 247, 0.14)',
    tabBar: 'rgba(18, 28, 39, 0.90)',
  },
};

export const spacing = {
  xs: 4,
  sm: 8,
  md: 12,
  lg: 16,
  xl: 24,
  xxl: 32,
} as const;

export const radius = {
  sm: 6,
  md: 10,
  lg: 14,
  pill: 999,
} as const;

export const typography = {
  display: {
    fontSize: 28,
    lineHeight: 34,
    fontWeight: '700',
    letterSpacing: -0.4,
  },
  title: {
    fontSize: 20,
    lineHeight: 26,
    fontWeight: '600',
    letterSpacing: -0.2,
  },
  body: {
    fontSize: 15,
    lineHeight: 22,
    fontWeight: '500',
  },
  label: {
    fontSize: 13,
    lineHeight: 18,
    fontWeight: '600',
    letterSpacing: 0.2,
  },
  data: {
    fontSize: 13,
    lineHeight: 18,
    fontFamily: 'SpaceMono',
  },
} as const;
