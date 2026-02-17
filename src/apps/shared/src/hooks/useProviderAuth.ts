import { useState, useRef, useCallback, useEffect } from "react";
import type { ChatDeviceCodeSession, ChatDeviceCodePollResult } from "../chat-client.js";

export interface ProviderAuthState {
  status: "idle" | "starting" | "polling" | "authorized" | "denied" | "expired" | "error";
  session?: { verificationUrl: string; userCode: string };
  error?: string;
}

export interface UseProviderAuthOptions {
  startLogin: (provider: string) => Promise<ChatDeviceCodeSession>;
  pollLogin: (provider: string, session: ChatDeviceCodeSession) => Promise<ChatDeviceCodePollResult>;
  onAuthorized: () => Promise<void>;
}

interface CancelToken {
  cancelled: boolean;
}

const MAX_POLL_ITERATIONS = 120;
const MIN_INTERVAL_MS = 1_000;
const DEFAULT_INTERVAL_SECS = 5;
/** RFC 8628 §3.5: increase interval by 5s on slow_down */
const SLOW_DOWN_INCREASE_SECS = 5;

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

export function useProviderAuth(opts: UseProviderAuthOptions): {
  authStates: Record<string, ProviderAuthState>;
  connect: (providerId: string) => void;
  cancel: (providerId: string) => void;
} {
  const [authStates, setAuthStates] = useState<Record<string, ProviderAuthState>>({});
  const tokensRef = useRef<Map<string, CancelToken>>(new Map());
  const optsRef = useRef(opts);
  optsRef.current = opts;

  const setState = useCallback((providerId: string, state: ProviderAuthState) => {
    setAuthStates((prev) => ({ ...prev, [providerId]: state }));
  }, []);

  const cancel = useCallback(
    (providerId: string) => {
      const token = tokensRef.current.get(providerId);
      if (token) token.cancelled = true;
      tokensRef.current.delete(providerId);
      setState(providerId, { status: "idle" });
    },
    [setState],
  );

  const connect = useCallback(
    (providerId: string) => {
      // Cancel any in-flight flow for this provider
      const existing = tokensRef.current.get(providerId);
      if (existing) existing.cancelled = true;

      const token: CancelToken = { cancelled: false };
      tokensRef.current.set(providerId, token);

      setState(providerId, { status: "starting" });

      (async () => {
        try {
          const session = await optsRef.current.startLogin(providerId);
          if (token.cancelled) return;

          setState(providerId, {
            status: "polling",
            session: {
              verificationUrl: session.verificationUrl,
              userCode: session.userCode,
            },
          });

          let intervalSecs = Math.max(1, session.intervalSecs || DEFAULT_INTERVAL_SECS);

          for (let i = 0; i < MAX_POLL_ITERATIONS; i++) {
            await delay(Math.max(MIN_INTERVAL_MS, intervalSecs * 1_000));
            if (token.cancelled) return;

            const result = await optsRef.current.pollLogin(providerId, session);
            if (token.cancelled) return;

            switch (result.status) {
              case "authorized":
                setState(providerId, { status: "authorized" });
                tokensRef.current.delete(providerId);
                await optsRef.current.onAuthorized();
                return;

              case "slow_down":
                intervalSecs = Math.max(
                  intervalSecs + SLOW_DOWN_INCREASE_SECS,
                  result.intervalSecs ?? 0,
                );
                continue;

              case "pending":
                if (result.intervalSecs != null && result.intervalSecs > 0) {
                  intervalSecs = result.intervalSecs;
                }
                continue;

              case "denied":
                setState(providerId, { status: "denied", error: "Access denied." });
                tokensRef.current.delete(providerId);
                return;

              case "expired":
                setState(providerId, { status: "expired", error: "Device code expired." });
                tokensRef.current.delete(providerId);
                return;
            }
          }

          // Exceeded max poll iterations
          if (!token.cancelled) {
            setState(providerId, { status: "expired", error: "Authorization timed out." });
            tokensRef.current.delete(providerId);
          }
        } catch (err) {
          if (token.cancelled) return;
          const message = err instanceof Error ? err.message : "Login failed.";
          setState(providerId, { status: "error", error: message });
          tokensRef.current.delete(providerId);
        }
      })();
    },
    [setState],
  );

  // Cancel all in-flight flows on unmount
  useEffect(() => {
    const tokens = tokensRef.current;
    return () => {
      for (const token of tokens.values()) {
        token.cancelled = true;
      }
      tokens.clear();
    };
  }, []);

  return { authStates, connect, cancel };
}
