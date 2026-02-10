import { useEffect, useRef } from "react";
import { Check, Trash2, X } from "lucide-react";
import type { ServerHello } from "@homie/shared";
import type { Target } from "@/hooks/use-targets";

interface GatewayDetailsModalProps {
  target: Target;
  detailsName: string;
  detailsUrl: string;
  onNameChange: (value: string) => void;
  onUrlChange: (value: string) => void;
  onRemove: () => void;
  onSave: () => void;
  onClose: () => void;
  serverHello: ServerHello | null;
  showActiveGatewayInfo: boolean;
}

export function GatewayDetailsModal({
  target,
  detailsName,
  detailsUrl,
  onNameChange,
  onUrlChange,
  onRemove,
  onSave,
  onClose,
  serverHello,
  showActiveGatewayInfo,
}: GatewayDetailsModalProps) {
  const modalRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    modalRef.current?.focus();
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/40 p-4"
      onClick={onClose}
      role="presentation"
    >
      <div
        ref={modalRef}
        tabIndex={-1}
        role="dialog"
        aria-modal="true"
        aria-label="Gateway details"
        className="w-full max-w-[680px] max-h-[85vh] overflow-auto bg-popover border border-border rounded-lg shadow-lg outline-none homie-popover"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="p-4 border-b border-border flex items-center justify-between gap-3">
          <div>
            <div className="text-sm font-semibold">Gateway Details</div>
            <div className="text-xs text-muted-foreground">Edit name / URL, or remove this gateway.</div>
          </div>
          <button
            type="button"
            onClick={onClose}
            className="p-2 min-h-[44px] min-w-[44px] rounded-md text-muted-foreground hover:text-foreground hover:bg-muted/60 transition-colors"
            aria-label="Close"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-4 space-y-4">
          <div className="grid gap-3 sm:grid-cols-2">
            <div>
              <label className="block text-xs text-muted-foreground mb-1">Name</label>
              <input
                type="text"
                value={detailsName}
                onChange={(e) => onNameChange(e.target.value)}
                className="w-full bg-background border border-border rounded px-2 py-2 text-sm text-foreground focus:outline-none focus:border-primary"
              />
            </div>

            <div>
              <label className="block text-xs text-muted-foreground mb-1">WS URL</label>
              <input
                type="text"
                value={detailsUrl}
                onChange={(e) => onUrlChange(e.target.value)}
                disabled={target.type === "local"}
                className="w-full bg-background border border-border rounded px-2 py-2 text-sm text-foreground focus:outline-none focus:border-primary disabled:opacity-60"
              />
              {target.type === "local" && (
                <div className="text-[11px] text-muted-foreground mt-1">
                  Local gateway URL is derived automatically.
                </div>
              )}
            </div>
          </div>

          <div className="flex flex-wrap items-center justify-end gap-2">
            <button
              type="button"
              onClick={() => {
                const label = target.type === "local" ? "Hide local gateway?" : "Remove this gateway?";
                if (!confirm(label)) return;
                onRemove();
              }}
              className="flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md border border-border text-destructive hover:bg-destructive/10 transition-colors"
            >
              <Trash2 className="w-4 h-4" />
              {target.type === "local" ? "Hide Local" : "Remove"}
            </button>

            <button
              type="button"
              onClick={onSave}
              className="flex items-center gap-2 px-3 py-2 min-h-[44px] rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
            >
              <Check className="w-4 h-4" />
              Save
            </button>
          </div>

          <div className="border-t border-border pt-4">
            <div className="text-xs uppercase tracking-wide text-muted-foreground mb-2">Connection / Server</div>
            {showActiveGatewayInfo ? (
              <div className="text-sm">
                <div className="grid grid-cols-2 gap-2">
                  <span className="text-muted-foreground">ID:</span>
                  <span className="font-mono text-xs break-all">{serverHello?.server_id}</span>
                  <span className="text-muted-foreground">Protocol:</span>
                  <span>v{serverHello?.protocol_version}</span>
                  {serverHello?.identity && (
                    <>
                      <span className="text-muted-foreground">Identity:</span>
                      <span>{serverHello.identity}</span>
                    </>
                  )}
                </div>

                <div className="mt-3">
                  <div className="font-semibold mb-1">Services</div>
                  <ul className="list-disc list-inside text-muted-foreground">
                    {serverHello?.services.map((s, i) => (
                      <li key={i}>{s.service} (v{s.version})</li>
                    ))}
                  </ul>
                </div>
              </div>
            ) : (
              <div className="text-sm text-muted-foreground">
                Select this gateway to connect and load server details.
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
