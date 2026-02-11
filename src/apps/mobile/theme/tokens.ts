import { StyleSheet } from 'react-native';

type AppPaletteBase = {
  background: string;
  surface0: string;
  surface1: string;
  surface2: string;
  surface3: string;
  text: string;
  textSecondary: string;
  textTertiary: string;
  accent: string;
  accentDim: string;
  success: string;
  successDim: string;
  warning: string;
  warningDim: string;
  danger: string;
  dangerDim: string;
  border: string;
  borderActive: string;
  overlay: string;
  tabBar: string;
};

export type AppPalette = AppPaletteBase & {
  /**
   * @deprecated Use surface0
   */
  readonly surface: string;
  /**
   * @deprecated Use surface1
   */
  readonly surfaceAlt: string;
};

function createPalette(palette: AppPaletteBase): AppPalette {
  return {
    ...palette,
    get surface() {
      return this.surface0;
    },
    get surfaceAlt() {
      return this.surface1;
    },
  };
}

export const palettes: Record<'light' | 'dark', AppPalette> = {
  light: createPalette({
    background: '#F5F7FA',
    surface0: '#FFFFFF',
    surface1: '#F0F2F6',
    surface2: '#E8ECF2',
    surface3: '#DDE2EA',
    text: '#0F1720',
    textSecondary: '#5B6878',
    textTertiary: '#97A3B3',
    accent: '#0A78E8',
    accentDim: 'rgba(10, 120, 232, 0.08)',
    success: '#1A8F5C',
    successDim: 'rgba(26, 143, 92, 0.08)',
    warning: '#B06A0A',
    warningDim: 'rgba(176, 106, 10, 0.08)',
    danger: '#C0364C',
    dangerDim: 'rgba(192, 54, 76, 0.08)',
    border: 'rgba(15, 23, 32, 0.08)',
    borderActive: 'rgba(15, 23, 32, 0.16)',
    overlay: 'rgba(0, 0, 0, 0.30)',
    tabBar: 'rgba(255, 255, 255, 0.92)',
  }),
  dark: createPalette({
    background: '#0B1018',
    surface0: '#111921',
    surface1: '#18222D',
    surface2: '#1F2B38',
    surface3: '#273545',
    text: '#E8EDF3',
    textSecondary: '#7B8A9C',
    textTertiary: '#4A5768',
    accent: '#4FA4FF',
    accentDim: 'rgba(79, 164, 255, 0.12)',
    success: '#43C38A',
    successDim: 'rgba(67, 195, 138, 0.12)',
    warning: '#F0B44D',
    warningDim: 'rgba(240, 180, 77, 0.12)',
    danger: '#F06A80',
    dangerDim: 'rgba(240, 106, 128, 0.12)',
    border: 'rgba(255, 255, 255, 0.06)',
    borderActive: 'rgba(255, 255, 255, 0.12)',
    overlay: 'rgba(0, 0, 0, 0.45)',
    tabBar: 'rgba(18, 28, 39, 0.90)',
  }),
};

export const spacing = {
  micro: 2,
  xs: 4,
  sm: 6,
  md: 8,
  lg: 12,
  xl: 16,
  xxl: 24,
  xxxl: 32,
} as const;

export const radius = {
  micro: 4,
  sm: 8,
  md: 12,
  lg: 16,
  pill: 999,
} as const;

export const typography = {
  display: {
    fontSize: 28,
    lineHeight: 34,
    fontWeight: '700',
    letterSpacing: -0.4,
  },
  heading: {
    fontSize: 22,
    lineHeight: 28,
    fontWeight: '600',
    letterSpacing: -0.3,
  },
  title: {
    fontSize: 17,
    lineHeight: 24,
    fontWeight: '600',
    letterSpacing: -0.2,
  },
  body: {
    fontSize: 15,
    lineHeight: 22,
    fontWeight: '400',
    letterSpacing: 0,
  },
  bodyMedium: {
    fontSize: 15,
    lineHeight: 22,
    fontWeight: '500',
    letterSpacing: 0,
  },
  caption: {
    fontSize: 13,
    lineHeight: 18,
    fontWeight: '500',
    letterSpacing: 0.1,
  },
  label: {
    fontSize: 12,
    lineHeight: 16,
    fontWeight: '600',
    letterSpacing: 0.3,
  },
  overline: {
    fontSize: 11,
    lineHeight: 14,
    fontWeight: '600',
    letterSpacing: 0.8,
  },
  mono: {
    fontSize: 13,
    lineHeight: 18,
    fontWeight: '400',
    letterSpacing: 0,
    fontFamily: 'SpaceMono',
  },
  monoSmall: {
    fontSize: 11,
    lineHeight: 16,
    fontWeight: '400',
    letterSpacing: 0,
    fontFamily: 'SpaceMono',
  },
  codeBlock: {
    fontSize: 13,
    lineHeight: 20,
    fontWeight: '400',
    letterSpacing: 0,
    fontFamily: 'SpaceMono',
  },
  /**
   * @deprecated Use mono
   */
  data: {
    fontSize: 13,
    lineHeight: 18,
    fontWeight: '400',
    letterSpacing: 0,
    fontFamily: 'SpaceMono',
  },
} as const;

export const iconSize = {
  xs: 12,
  sm: 14,
  md: 18,
  lg: 22,
  xl: 28,
  display: 48,
} as const;

export const borderWidth = {
  hairline: StyleSheet.hairlineWidth,
  thin: 1,
  medium: 2,
  thick: 3,
  accent: 4,
} as const;

export const opacity = {
  pressed: 0.7,
  disabled: 0.38,
  hover: 0.85,
  dimIcon: 0.5,
  backdrop: 0.45,
  backdropLight: 0.3,
  skeleton: 0.4,
} as const;

export const elevation = {
  none: {
    shadowColor: 'transparent',
    shadowOpacity: 0,
    shadowRadius: 0,
    shadowOffset: { width: 0, height: 0 },
    elevation: 0,
  },
  drawer: {
    shadowColor: '#000000',
    shadowOpacity: 0.2,
    shadowRadius: 24,
    shadowOffset: { width: 8, height: 0 },
    elevation: 8,
  },
  sheet: {
    shadowColor: '#000000',
    shadowOpacity: 0.15,
    shadowRadius: 16,
    shadowOffset: { width: 0, height: -4 },
    elevation: 6,
  },
  fab: {
    shadowColor: '#000000',
    shadowOpacity: 0.18,
    shadowRadius: 8,
    shadowOffset: { width: 0, height: 2 },
    elevation: 4,
  },
} as const;

export const zIndex = {
  base: 0,
  sticky: 10,
  fab: 20,
  drawer: 30,
  sheet: 40,
  toast: 50,
} as const;

export const touchTarget = {
  min: 44,
  comfortable: 48,
  compact: 36,
} as const;
