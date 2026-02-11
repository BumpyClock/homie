import {
  createContext,
  useCallback,
  useContext,
  useMemo,
  useState,
  type PropsWithChildren,
} from 'react';

import { useGatewayChat, type UseGatewayChatResult } from '@/hooks/useGatewayChat';
import { useGatewayTarget } from '@/hooks/useGatewayTarget';

export interface MobileShellDataContextValue extends UseGatewayChatResult {
  loadingTarget: boolean;
  targetUrl: string | null;
  hasTarget: boolean;
  targetHint: string;
  targetError: string | null;
  savingTarget: boolean;
  saveGatewayTarget: (value: string) => Promise<void>;
  clearGatewayTarget: () => Promise<void>;
}

const MobileShellDataContext = createContext<MobileShellDataContextValue | null>(null);

export function MobileShellDataProvider({ children }: PropsWithChildren) {
  const [savingTarget, setSavingTarget] = useState(false);
  const {
    loading: loadingTarget,
    targetUrl,
    hasTarget,
    targetHint,
    error: targetError,
    saveTarget,
    clearTarget,
  } = useGatewayTarget();
  const gatewayState = useGatewayChat(targetUrl ?? '');

  const saveGatewayTarget = useCallback(async (value: string) => {
    setSavingTarget(true);
    try {
      await saveTarget(value);
    } finally {
      setSavingTarget(false);
    }
  }, [saveTarget]);

  const clearGatewayTarget = useCallback(async () => {
    setSavingTarget(true);
    try {
      await clearTarget();
    } finally {
      setSavingTarget(false);
    }
  }, [clearTarget]);

  const value = useMemo<MobileShellDataContextValue>(
    () => ({
      ...gatewayState,
      loadingTarget,
      targetUrl,
      hasTarget,
      targetHint,
      targetError,
      savingTarget,
      saveGatewayTarget,
      clearGatewayTarget,
    }),
    [
      gatewayState,
      loadingTarget,
      targetUrl,
      hasTarget,
      targetHint,
      targetError,
      savingTarget,
      saveGatewayTarget,
      clearGatewayTarget,
    ],
  );

  return (
    <MobileShellDataContext.Provider value={value}>
      {children}
    </MobileShellDataContext.Provider>
  );
}

export function useMobileShellData() {
  const context = useContext(MobileShellDataContext);
  if (!context) {
    throw new Error('useMobileShellData must be used inside MobileShellDataProvider');
  }
  return context;
}
