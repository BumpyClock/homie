/**
 * Minimal react-native mock for jsdom test environment.
 * Only stubs the APIs actually referenced by the code under test.
 */

export const Platform = { OS: 'ios', select: (o: Record<string, unknown>) => o.ios ?? o.default };
export const StyleSheet = { create: <T extends Record<string, unknown>>(s: T): T => s };
export const AppState = { currentState: 'active', addEventListener: () => ({ remove: () => {} }) };
export const Dimensions = {
  get: () => ({ width: 375, height: 812, scale: 2, fontScale: 1 }),
  addEventListener: () => ({ remove: () => {} }),
};
