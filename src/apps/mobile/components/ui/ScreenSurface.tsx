import { type PropsWithChildren, useEffect } from 'react';
import { StyleSheet } from 'react-native';
import Animated, { useAnimatedStyle, useSharedValue, withTiming } from 'react-native-reanimated';

import { useReducedMotion } from '@/hooks/useReducedMotion';
import { motion } from '@/theme/motion';

export function ScreenSurface({ children }: PropsWithChildren) {
  const reducedMotion = useReducedMotion();
  const opacity = useSharedValue(reducedMotion ? 1 : 0);
  const translateY = useSharedValue(reducedMotion ? 0 : 10);

  useEffect(() => {
    opacity.value = withTiming(1, {
      duration: reducedMotion ? 0 : motion.duration.standard,
      easing: motion.easing.enter,
    });
    translateY.value = withTiming(0, {
      duration: reducedMotion ? 0 : motion.duration.standard,
      easing: motion.easing.enter,
    });
  }, [opacity, reducedMotion, translateY]);

  const style = useAnimatedStyle(() => ({
    opacity: opacity.value,
    transform: [{ translateY: translateY.value }],
  }));

  return <Animated.View style={[styles.root, style]}>{children}</Animated.View>;
}

const styles = StyleSheet.create({
  root: {
    flex: 1,
  },
});
