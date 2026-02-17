import { useRef, useEffect } from "react";
import { KeyRound } from "lucide-react";
import type { ChatAccountProviderStatus } from "@homie/shared";
import type { ProviderAuthState } from "@homie/shared";
import { modelProviderLabel } from "@homie/shared";
import { DeviceCodeInline } from "./DeviceCodeInline";

interface ProviderRowProps {
  provider: ChatAccountProviderStatus;
  authState: ProviderAuthState | undefined;
  onConnect: (providerId: string) => void;
  onCancel: (providerId: string) => void;
}

type EffectiveStatus = "idle" | "starting" | "polling" | "authorized" | "error";

function resolveStatus(
  provider: ChatAccountProviderStatus,
  authState: ProviderAuthState | undefined,
): EffectiveStatus {
  const hookStatus = authState?.status ?? "idle";
  // If the hook says authorized OR the provider was already logged in, show authorized
  if (hookStatus === "authorized" || (hookStatus === "idle" && provider.loggedIn)) {
    return "authorized";
  }
  if (hookStatus === "denied" || hookStatus === "expired" || hookStatus === "error") {
    return "error";
  }
  if (hookStatus === "starting") return "starting";
  if (hookStatus === "polling") return "polling";
  return "idle";
}

function formatExpiry(expiresAt: string | undefined): string | null {
  if (!expiresAt) return null;
  try {
    return new Date(expiresAt).toLocaleDateString(undefined, {
      year: "numeric",
      month: "short",
      day: "numeric",
    });
  } catch {
    return null;
  }
}

export function ProviderRow({ provider, authState, onConnect, onCancel }: ProviderRowProps) {
  const status = resolveStatus(provider, authState);
  const isExpanded = status === "polling";
  const liveRef = useRef<HTMLDivElement>(null);
  const prevStatusRef = useRef(status);

  const label = modelProviderLabel({ model: "", provider: provider.id });
  const expiry = formatExpiry(provider.expiresAt);
  const errorText = authState?.error;

  // Announce status changes to screen readers
  useEffect(() => {
    if (prevStatusRef.current !== status && liveRef.current) {
      if (status === "authorized") {
        liveRef.current.textContent = `${label} connected`;
      } else if (status === "error") {
        liveRef.current.textContent = `${label}: ${errorText ?? "Error"}`;
      } else if (status === "polling") {
        liveRef.current.textContent = `${label}: Waiting for authorization`;
      }
    }
    prevStatusRef.current = status;
  }, [status, label, errorText]);

  return (
    <div className="rounded-md border border-border bg-surface-0 transition-colors duration-[200ms]">
      {/* Main row */}
      <div className="flex items-center gap-3 px-4 py-3 min-h-[56px]">
        {/* Provider icon */}
        <div className="shrink-0 w-8 h-8 rounded-full bg-surface-1 flex items-center justify-center">
          <KeyRound className="w-4 h-4 text-text-secondary" aria-hidden="true" />
        </div>

        {/* Name + meta */}
        <div className="flex-1 min-w-0">
          <div className="text-sm font-medium text-text-primary truncate">{label}</div>
          {status === "authorized" && (
            <div className="text-xs text-text-secondary space-x-2">
              {provider.scopes && provider.scopes.length > 0 && (
                <span>Scopes: {provider.scopes.join(" \u00B7 ")}</span>
              )}
              {expiry && <span>Expires: {expiry}</span>}
            </div>
          )}
          {status === "idle" && (
            <div className="text-xs text-text-secondary">Not connected</div>
          )}
          {status === "starting" && (
            <div className="text-xs text-text-secondary">Starting device code flow\u2026</div>
          )}
          {status === "error" && (
            <div className="text-xs text-danger">{errorText ?? "Authentication failed."}</div>
          )}
        </div>

        {/* Status pill / action */}
        <div className="shrink-0 flex items-center">
          {status === "authorized" && (
            <span className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-full text-xs font-medium bg-success-dim text-success transition-colors duration-[200ms]">
              <span className="w-1.5 h-1.5 rounded-full bg-success" aria-hidden="true" />
              Connected
            </span>
          )}
          {status === "idle" && (
            <button
              type="button"
              onClick={() => onConnect(provider.id)}
              className="min-h-[44px] px-4 rounded-md text-sm font-medium bg-primary text-primary-foreground hover:bg-primary/90 transition-colors duration-[200ms]"
            >
              Connect
            </button>
          )}
          {status === "starting" && (
            <button
              type="button"
              disabled
              className="min-h-[44px] px-4 rounded-md text-sm font-medium bg-primary/60 text-primary-foreground cursor-not-allowed opacity-70"
            >
              Connecting\u2026
            </button>
          )}
          {status === "polling" && (
            <button
              type="button"
              onClick={() => onCancel(provider.id)}
              className="min-h-[44px] px-4 rounded-md text-sm font-medium border border-border text-text-secondary hover:text-text-primary hover:bg-surface-1 transition-colors duration-[200ms]"
            >
              Cancel
            </button>
          )}
          {status === "error" && (
            <button
              type="button"
              onClick={() => onConnect(provider.id)}
              className="min-h-[44px] px-4 rounded-md text-sm font-medium bg-primary text-primary-foreground hover:bg-primary/90 transition-colors duration-[200ms]"
            >
              Try Again
            </button>
          )}
        </div>
      </div>

      {/* Expandable device code area */}
      <div
        className="grid transition-[grid-template-rows] duration-[200ms] ease-[cubic-bezier(0.4,0,0.2,1)]"
        style={{ gridTemplateRows: isExpanded ? "1fr" : "0fr" }}
      >
        <div className="overflow-hidden">
          {isExpanded && authState?.session && (
            <div className="px-4 pb-4">
              <DeviceCodeInline
                verificationUrl={authState.session.verificationUrl}
                userCode={authState.session.userCode}
              />
            </div>
          )}
        </div>
      </div>

      {/* Accessibility live region */}
      <div ref={liveRef} aria-live="polite" className="sr-only" />
    </div>
  );
}
