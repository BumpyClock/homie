import { useState, useCallback, useEffect, useRef } from "react";
import { Copy, Check, ExternalLink } from "lucide-react";

interface DeviceCodeInlineProps {
  verificationUrl: string;
  userCode: string;
  expiresAt?: string;
}

export function DeviceCodeInline({ verificationUrl, userCode, expiresAt }: DeviceCodeInlineProps) {
  const [copied, setCopied] = useState(false);
  const [remaining, setRemaining] = useState<string | null>(null);
  const timerRef = useRef<ReturnType<typeof setInterval>>();

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(userCode);
      setCopied(true);
      setTimeout(() => setCopied(false), 2000);
    } catch {
      // Fallback: select text for manual copy
    }
  }, [userCode]);

  useEffect(() => {
    if (!expiresAt) return;

    const update = () => {
      const diff = new Date(expiresAt).getTime() - Date.now();
      if (diff <= 0) {
        setRemaining("Expired");
        if (timerRef.current) clearInterval(timerRef.current);
        return;
      }
      const mins = Math.floor(diff / 60_000);
      const secs = Math.floor((diff % 60_000) / 1_000);
      setRemaining(`${mins}:${secs.toString().padStart(2, "0")}`);
    };

    update();
    timerRef.current = setInterval(update, 1_000);
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [expiresAt]);

  return (
    <div className="homie-fade-in rounded-md border border-border bg-surface-1 p-4 space-y-3">
      <div className="text-xs uppercase tracking-wide text-text-secondary font-medium">
        Verify your account
      </div>

      {/* Step 1: Open URL */}
      <div className="flex items-start gap-2 text-sm">
        <span className="text-text-tertiary shrink-0 font-medium">1.</span>
        <span className="text-text-secondary">
          Open{" "}
          <a
            href={verificationUrl.startsWith("http") ? verificationUrl : `https://${verificationUrl}`}
            target="_blank"
            rel="noopener noreferrer"
            className="inline-flex items-center gap-1 text-primary underline underline-offset-2 hover:text-primary/80 focus-visible:outline-2 focus-visible:outline-primary focus-visible:outline-offset-2 rounded-sm"
          >
            {verificationUrl.replace(/^https?:\/\//, "")}
            <ExternalLink className="w-3 h-3 shrink-0" aria-hidden="true" />
          </a>
        </span>
      </div>

      {/* Step 2: Enter code */}
      <div className="flex items-start gap-2 text-sm">
        <span className="text-text-tertiary shrink-0 font-medium">2.</span>
        <span className="text-text-secondary">Enter code:</span>
      </div>

      {/* Code display + copy */}
      <div className="flex items-center gap-3">
        <code
          aria-live="assertive"
          className="flex-1 text-lg font-mono font-semibold text-text-primary tracking-widest select-all bg-surface-0 rounded-md px-3 py-2 border border-border text-center"
        >
          {userCode}
        </code>
        <button
          type="button"
          onClick={handleCopy}
          aria-label="Copy verification code"
          className="min-h-[44px] min-w-[44px] flex items-center justify-center gap-1.5 px-3 rounded-md text-sm font-medium bg-surface-0 border border-border text-text-secondary hover:text-text-primary hover:bg-surface-1 transition-colors duration-[200ms]"
        >
          {copied ? (
            <>
              <Check className="w-4 h-4 text-success" aria-hidden="true" />
              <span className="text-success">Copied!</span>
            </>
          ) : (
            <>
              <Copy className="w-4 h-4" aria-hidden="true" />
              <span>Copy</span>
            </>
          )}
        </button>
      </div>

      {/* Waiting indicator */}
      <div className="flex items-center gap-2 text-sm text-text-secondary">
        <span className="homie-dots inline-flex gap-0.5" aria-hidden="true">
          <span className="inline-block w-1.5 h-1.5 rounded-full bg-text-tertiary" />
          <span className="inline-block w-1.5 h-1.5 rounded-full bg-text-tertiary" />
          <span className="inline-block w-1.5 h-1.5 rounded-full bg-text-tertiary" />
        </span>
        <span>Waiting for authorization{remaining ? ` \u2014 expires in ${remaining}` : "\u2026"}</span>
      </div>
    </div>
  );
}
