import { StatusDot } from "@/components/status-dot";
import type { ConnectionStatus } from "@/hooks/use-gateway";
import type { Target } from "@/hooks/use-targets";
import type { ServerHello } from "@homie/shared";

interface ConnectionSectionProps {
  status: ConnectionStatus;
  activeTarget: Target | null;
  serverHello: ServerHello | null;
}

export function ConnectionSection({ status, activeTarget, serverHello }: ConnectionSectionProps) {
  return (
    <div
      role="tabpanel"
      id="settings-panel-connection"
      aria-labelledby="settings-tab-connection"
      className="space-y-4"
    >
      <h2 className="text-sm font-semibold text-text-primary">Connection</h2>

      <div className="rounded-md border border-border bg-surface-0 p-4 space-y-3">
        <div className="flex items-center gap-3">
          <StatusDot status={status} className="h-3 w-3" />
          <div>
            <div className="text-sm font-medium text-text-primary capitalize">{status}</div>
            <div className="text-xs text-text-secondary">Gateway status</div>
          </div>
        </div>

        {activeTarget && (
          <div className="space-y-2 text-sm">
            <div className="flex items-baseline gap-2">
              <span className="text-text-secondary min-w-[60px]">Target:</span>
              <span className="text-text-primary font-medium">{activeTarget.name}</span>
            </div>
            <div className="flex items-baseline gap-2">
              <span className="text-text-secondary min-w-[60px]">URL:</span>
              <span className="text-text-primary font-mono text-xs break-all">{activeTarget.url}</span>
            </div>
            <div className="flex items-baseline gap-2">
              <span className="text-text-secondary min-w-[60px]">Type:</span>
              <span className="text-text-primary capitalize">{activeTarget.type}</span>
            </div>
          </div>
        )}

        {!activeTarget && (
          <div className="text-sm text-text-secondary">No target selected.</div>
        )}
      </div>

      {serverHello && (
        <div className="rounded-md border border-border bg-surface-0 p-4 space-y-3">
          <div className="text-xs uppercase tracking-wide text-text-secondary font-medium">Server Info</div>
          <div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
            <span className="text-text-secondary">ID:</span>
            <span className="font-mono text-xs text-text-primary break-all">{serverHello.server_id}</span>
            <span className="text-text-secondary">Protocol:</span>
            <span className="text-text-primary">v{serverHello.protocol_version}</span>
            {serverHello.identity && (
              <>
                <span className="text-text-secondary">Identity:</span>
                <span className="text-text-primary">{serverHello.identity}</span>
              </>
            )}
          </div>

          {serverHello.services.length > 0 && (
            <div>
              <div className="text-xs text-text-secondary mb-1">Services</div>
              <ul className="space-y-0.5">
                {serverHello.services.map((s, i) => (
                  <li key={i} className="text-sm text-text-primary">
                    {s.service} <span className="text-text-tertiary">v{s.version}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}
        </div>
      )}

      {status === "error" && (
        <div className="rounded-md border border-danger/30 bg-danger-dim p-3 text-sm text-danger">
          Connection error. Check that the gateway is running and the URL is correct.
        </div>
      )}
    </div>
  );
}
