import { Easing } from 'react-native-reanimated';

export const motion = {
  duration: {
    micro: 80,
    fast: 140,
    standard: 220,
    emphasis: 320,
    dramatic: 450,
    // Backwards compat aliases while migrating callers.
    quick: 140,
    regular: 220,
  },
  easing: {
    enter: Easing.out(Easing.cubic),
    exit: Easing.in(Easing.cubic),
    move: Easing.inOut(Easing.cubic),
    linear: Easing.linear,
    overshoot: Easing.out(Easing.back(1.7)),
    // Backwards compat alias while migrating callers.
    enterExit: Easing.out(Easing.cubic),
  },
  spring: {
    snappy: { damping: 20, stiffness: 300, mass: 0.5 },
    responsive: { damping: 18, stiffness: 220, mass: 0.6 },
    drawer: { damping: 22, stiffness: 180, mass: 0.8 },
    sheet: { damping: 24, stiffness: 200, mass: 1.0 },
    gentle: { damping: 26, stiffness: 120, mass: 1.0 },
  },
  stagger: {
    tight: 30,
    standard: 60,
    relaxed: 100,
  },
} as const;
