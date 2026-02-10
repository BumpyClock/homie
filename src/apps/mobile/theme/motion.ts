import { Easing } from 'react-native-reanimated';

export const motion = {
  duration: {
    quick: 140,
    regular: 220,
  },
  easing: {
    enterExit: Easing.out(Easing.cubic),
    move: Easing.inOut(Easing.cubic),
  },
} as const;
