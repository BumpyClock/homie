import AsyncStorage from '@react-native-async-storage/async-storage';
import { useCallback, useEffect, useState } from 'react';

import { runtimeConfig } from '@/config/runtime';

const STORAGE_KEY = 'homie.mobile.gateway_target_url';

type SharedGatewayTargetState = {
  loaded: boolean;
  targetUrl: string | null;
  error: string | null;
};

type TargetListener = (state: SharedGatewayTargetState) => void;

let sharedState: SharedGatewayTargetState = {
  loaded: false,
  targetUrl: null,
  error: null,
};
let loadPromise: Promise<void> | null = null;
const listeners = new Set<TargetListener>();

function notifyTargetListeners() {
  for (const listener of listeners) {
    listener(sharedState);
  }
}

function setSharedTargetState(next: Partial<SharedGatewayTargetState>) {
  sharedState = {
    ...sharedState,
    ...next,
  };
  notifyTargetListeners();
}

function normalizeGatewayUrl(rawValue: string): string {
  const value = rawValue.trim();
  if (!value) {
    throw new Error('Enter a gateway URL');
  }

  let parsed: URL;
  try {
    parsed = new URL(value);
  } catch {
    throw new Error('Use a valid URL like wss://gateway.example.com/ws');
  }

  if (parsed.protocol !== 'ws:' && parsed.protocol !== 'wss:') {
    throw new Error('Gateway URL must use ws:// or wss://');
  }

  return parsed.toString();
}

async function ensureGatewayTargetLoaded() {
  if (sharedState.loaded) return;
  if (loadPromise) {
    await loadPromise;
    return;
  }

  loadPromise = (async () => {
    try {
      const stored = await AsyncStorage.getItem(STORAGE_KEY);
      const fallback = runtimeConfig.gatewayUrl.trim();
      const resolved = stored && stored.trim() ? stored.trim() : null;
      setSharedTargetState({
        loaded: true,
        targetUrl: resolved,
        error: null,
      });
      if (!resolved && fallback) {
        setSharedTargetState({
          error: `Tip: paste gateway URL from EXPO_PUBLIC_HOMIE_GATEWAY_URL (${fallback})`,
        });
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : 'Failed to load target';
      setSharedTargetState({
        loaded: true,
        targetUrl: null,
        error: message,
      });
    } finally {
      loadPromise = null;
    }
  })();

  await loadPromise;
}

async function persistGatewayTarget(targetUrl: string | null) {
  if (targetUrl) {
    await AsyncStorage.setItem(STORAGE_KEY, targetUrl);
    setSharedTargetState({
      targetUrl,
      error: null,
    });
    return;
  }

  await AsyncStorage.removeItem(STORAGE_KEY);
  setSharedTargetState({
    targetUrl: null,
    error: null,
  });
}

export interface UseGatewayTargetResult {
  loading: boolean;
  targetUrl: string | null;
  hasTarget: boolean;
  targetHint: string;
  error: string | null;
  saveTarget: (value: string) => Promise<void>;
  clearTarget: () => Promise<void>;
}

export function useGatewayTarget(): UseGatewayTargetResult {
  const [state, setState] = useState(sharedState);

  useEffect(() => {
    const listener: TargetListener = (nextState) => {
      setState(nextState);
    };
    listeners.add(listener);
    void ensureGatewayTargetLoaded();
    return () => {
      listeners.delete(listener);
    };
  }, []);

  const saveTarget = useCallback(async (value: string) => {
    const normalized = normalizeGatewayUrl(value);
    await persistGatewayTarget(normalized);
  }, []);

  const clearTarget = useCallback(async () => {
    await persistGatewayTarget(null);
  }, []);

  const targetHint = runtimeConfig.gatewayUrl.trim();

  return {
    loading: !state.loaded,
    targetUrl: state.targetUrl,
    hasTarget: Boolean(state.targetUrl),
    targetHint,
    error: state.error,
    saveTarget,
    clearTarget,
  };
}
