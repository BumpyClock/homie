import { AlertTriangle, ChevronRight } from "lucide-react";
import { AUTH_COPY } from "@homie/shared";

interface AuthRedirectBannerProps {
  /** Controls visibility/animation */
  visible: boolean;
  /** Primary message text */
  message: string;
  /** Button text (default: "Go to Settings") */
  actionLabel?: string;
  /** Navigation callback */
  onAction: () => void;
}

/**
 * Inline banner for chat surfaces that appears when a provider is unauthorized.
 * Redirects users to Settings for provider authentication.
 *
 * Accessibility:
 * - role="alert" + aria-live="polite" for screen reader announcement
 * - Keyboard navigable action button
 * - 44px minimum touch target
 * - No layout shift: fixed height container with CSS grid collapse
 */
export function AuthRedirectBanner({
  visible,
  message,
  actionLabel = AUTH_COPY.bannerActionWeb,
  onAction,
}: AuthRedirectBannerProps) {
  return (
    <div
      role="alert"
      aria-live="polite"
      aria-atomic="true"
      className="auth-redirect-banner"
      data-visible={visible}
      style={{
        display: "grid",
        gridTemplateRows: visible ? "1fr" : "0fr",
        transition: "grid-template-rows var(--duration-fast, 140ms) var(--ease-enter, cubic-bezier(0, 0, 0.2, 1))",
      }}
    >
      <div style={{ overflow: "hidden" }}>
        <div
          className="flex items-center gap-2 rounded-lg border px-3 py-2.5 mb-3"
          style={{
            background: "hsl(var(--warning-dim))",
            borderColor: "hsl(var(--warning) / 0.3)",
            opacity: visible ? 1 : 0,
            transition: "opacity var(--duration-fast, 140ms) var(--ease-enter)",
          }}
        >
          <AlertTriangle
            className="shrink-0"
            size={16}
            style={{ color: "hsl(var(--warning))" }}
            aria-hidden="true"
          />
          <span
            className="flex-1 text-sm font-medium"
            style={{ color: "hsl(var(--foreground))" }}
          >
            {message}
          </span>
          <button
            type="button"
            onClick={onAction}
            className="
              inline-flex items-center gap-1 px-2.5 py-1.5 min-h-[44px] rounded-md
              text-sm font-semibold
              transition-colors duration-[80ms]
              hover:bg-[hsl(var(--warning)/0.15)]
              focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-[hsl(var(--warning))] focus-visible:ring-offset-2
              motion-reduce:transition-none
            "
            style={{ color: "hsl(var(--warning))" }}
            aria-label={`${actionLabel} to sign in to provider`}
          >
            {actionLabel}
            <ChevronRight size={14} aria-hidden="true" />
          </button>
        </div>
      </div>
    </div>
  );
}
