/**
 * In-memory mock of @react-native-async-storage/async-storage.
 * Provides the subset of the API used by the mobile app.
 */

const store = new Map<string, string>();

const AsyncStorage = {
  getItem: async (key: string): Promise<string | null> => store.get(key) ?? null,
  setItem: async (key: string, value: string): Promise<void> => {
    store.set(key, value);
  },
  removeItem: async (key: string): Promise<void> => {
    store.delete(key);
  },
  clear: async (): Promise<void> => {
    store.clear();
  },
  getAllKeys: async (): Promise<string[]> => [...store.keys()],
  multiGet: async (keys: string[]): Promise<[string, string | null][]> =>
    keys.map((k) => [k, store.get(k) ?? null]),
  multiSet: async (pairs: [string, string][]): Promise<void> => {
    for (const [k, v] of pairs) store.set(k, v);
  },
  multiRemove: async (keys: string[]): Promise<void> => {
    for (const k of keys) store.delete(k);
  },
};

export default AsyncStorage;

/** Helper — reset store between tests. */
export function resetAsyncStorage(): void {
  store.clear();
}
