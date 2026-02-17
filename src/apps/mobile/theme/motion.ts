import * as Haptics from 'expo-haptics';
import { Easing } from 'react-native-reanimated';

type HapticFeedback =
  | { kind: 'selection' }
  | { kind: 'impact'; style: Haptics.ImpactFeedbackStyle }
  | { kind: 'notification'; style: Haptics.NotificationFeedbackType };

const duration = {
  micro: 80,
  fast: 140,
  standard: 220,
  emphasis: 320,
  dramatic: 450,
} as const;

const easing = {
  enter: Easing.out(Easing.cubic),
  exit: Easing.in(Easing.cubic),
  move: Easing.inOut(Easing.cubic),
  linear: Easing.linear,
  overshoot: Easing.out(Easing.back(1.7)),
} as const;

export const motion = {
  duration: {
    ...duration,
    listStagger: 40,
    // Backwards compat aliases while migrating callers.
    quick: duration.fast,
    regular: duration.standard,
  },
  easing: {
    ...easing,
    // Backwards compat alias while migrating callers.
    enterExit: easing.enter,
  },
  spring: {
    snappy: { damping: 20, stiffness: 300, mass: 0.5 },
    responsive: { damping: 18, stiffness: 220, mass: 0.6 },
    drawer: { damping: 22, stiffness: 180, mass: 0.8 },
    list: { damping: 20, stiffness: 250, mass: 0.7 },
    sheet: { damping: 24, stiffness: 200, mass: 1.0 },
    gentle: { damping: 26, stiffness: 120, mass: 1.0 },
  },
  stagger: {
    tight: 30,
    standard: 60,
    relaxed: 100,
  },
  haptics: {
    navSelect: { kind: 'selection' },
    drawerToggle: { kind: 'selection' },
    drawerSnap: { kind: 'impact', style: Haptics.ImpactFeedbackStyle.Light },
    activityToggle: { kind: 'selection' },
    activityDetail: { kind: 'impact', style: Haptics.ImpactFeedbackStyle.Light },
    settingsTabSwitch: { kind: 'selection' },
    providerConnect: { kind: 'impact', style: Haptics.ImpactFeedbackStyle.Light },
    providerAuthorized: { kind: 'notification', style: Haptics.NotificationFeedbackType.Success },
    providerDenied: { kind: 'notification', style: Haptics.NotificationFeedbackType.Warning },
  } as const satisfies Record<string, HapticFeedback>,
} as const;

export function triggerMobileHaptic(feedback: HapticFeedback): void {
  switch (feedback.kind) {
    case 'selection':
      void Haptics.selectionAsync();
      return;
    case 'impact':
      void Haptics.impactAsync(feedback.style);
      return;
    case 'notification':
      void Haptics.notificationAsync(feedback.style);
      return;
    default:
      return;
  }
}
