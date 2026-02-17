import { useCallback, useEffect, useMemo, useState } from "react";
import { KeyRound } from "lucide-react";
import {
  createChatClient,
  useProviderAuth,
  type ChatAccountProviderStatus,
  type ChatDeviceCodeSession,
} from "@homie/shared";
import type { ConnectionStatus } from "@/hooks/use-gateway";
import { ProviderRow } from "./ProviderRow";

interface ProviderAccountsSectionProps {
  status: ConnectionStatus;
  call: (method: string, params?: unknown) => Promise<unknown>;
}

export function ProviderAccountsSection({ status, call }: ProviderAccountsSectionProps) {
  const [providers, setProviders] = useState<ChatAccountProviderStatus[]>([]);
  const [loading, setLoading] = useState(false);

  const chatClient = useMemo(() => createChatClient(call), [call]);

  const refreshProviders = useCallback(async () => {
    if (status !== "connected") return;
    setLoading(true);
    try {
      const result = await chatClient.listAccounts();
      setProviders(result);
    } catch {
      setProviders([]);
    } finally {
      setLoading(false);
    }
  }, [chatClient, status]);

  useEffect(() => {
    void refreshProviders();
  }, [refreshProviders]);

  const startLogin = useCallback(
    async (provider: string): Promise<ChatDeviceCodeSession> => {
      return chatClient.startAccountLogin({ provider });
    },
    [chatClient],
  );

  const pollLogin = useCallback(
    async (provider: string, session: ChatDeviceCodeSession) => {
      return chatClient.pollAccountLogin({ provider, session });
    },
    [chatClient],
  );

  const onAuthorized = useCallback(async () => {
    await refreshProviders();
  }, [refreshProviders]);

  const { authStates, connect, cancel } = useProviderAuth({
    startLogin,
    pollLogin,
    onAuthorized,
  });

  const enabledProviders = providers.filter((p) => p.enabled);

  return (
    <div
      role="tabpanel"
      id="settings-panel-providers"
      aria-labelledby="settings-tab-providers"
      className="space-y-4"
    >
      <div>
        <h2 className="text-sm font-semibold text-text-primary">Provider Accounts</h2>
        <p className="text-xs text-text-secondary mt-1">
          Connect your AI provider accounts to use their models.
        </p>
      </div>

      {loading && enabledProviders.length === 0 && (
        <div className="rounded-md border border-border bg-surface-0 p-6 flex items-center justify-center">
          <div className="flex items-center gap-2 text-sm text-text-secondary">
            <span className="homie-dots inline-flex gap-0.5" aria-hidden="true">
              <span className="inline-block w-1.5 h-1.5 rounded-full bg-text-tertiary" />
              <span className="inline-block w-1.5 h-1.5 rounded-full bg-text-tertiary" />
              <span className="inline-block w-1.5 h-1.5 rounded-full bg-text-tertiary" />
            </span>
            <span>Loading providers\u2026</span>
          </div>
        </div>
      )}

      {!loading && enabledProviders.length === 0 && (
        <div className="rounded-md border border-border bg-surface-0 p-6 text-center space-y-2">
          <div className="mx-auto w-10 h-10 rounded-full bg-surface-1 flex items-center justify-center">
            <KeyRound className="w-5 h-5 text-text-tertiary" aria-hidden="true" />
          </div>
          <div className="text-sm text-text-secondary">No providers enabled</div>
          <div className="text-xs text-text-tertiary">
            Enable providers in your gateway configuration to connect accounts here.
          </div>
        </div>
      )}

      {enabledProviders.length > 0 && (
        <div className="space-y-2">
          {enabledProviders.map((provider) => (
            <ProviderRow
              key={provider.id}
              provider={provider}
              authState={authStates[provider.id]}
              onConnect={connect}
              onCancel={cancel}
            />
          ))}
        </div>
      )}
    </div>
  );
}
