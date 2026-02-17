import type { ServerHello } from "@homie/shared";

interface AboutSectionProps {
  serverHello: ServerHello | null;
}

export function AboutSection({ serverHello }: AboutSectionProps) {
  return (
    <div
      role="tabpanel"
      id="settings-panel-about"
      aria-labelledby="settings-tab-about"
      className="space-y-4"
    >
      <h2 className="text-sm font-semibold text-text-primary">About</h2>

      <div className="rounded-md border border-border bg-surface-0 p-4 space-y-3">
        <div className="text-xs uppercase tracking-wide text-text-secondary font-medium">Tips</div>
        <ul className="space-y-2 text-sm text-text-primary">
          <li className="flex gap-2">
            <span className="text-text-tertiary shrink-0">&bull;</span>
            <span>Use the target selector to switch between gateways.</span>
          </li>
          <li className="flex gap-2">
            <span className="text-text-tertiary shrink-0">&bull;</span>
            <span>Click a session card to attach and interact with it.</span>
          </li>
          <li className="flex gap-2">
            <span className="text-text-tertiary shrink-0">&bull;</span>
            <span>Previews refresh automatically based on the selected interval.</span>
          </li>
          <li className="flex gap-2">
            <span className="text-text-tertiary shrink-0">&bull;</span>
            <span>Use keyboard shortcut <kbd className="px-1 py-0.5 rounded bg-surface-1 text-xs font-mono">Esc</kbd> to close panels.</span>
          </li>
        </ul>
      </div>

      {serverHello && (
        <div className="rounded-md border border-border bg-surface-0 p-4 space-y-2">
          <div className="text-xs uppercase tracking-wide text-text-secondary font-medium">Gateway</div>
          <div className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-1 text-sm">
            <span className="text-text-secondary">Server ID:</span>
            <span className="font-mono text-xs text-text-primary break-all">{serverHello.server_id}</span>
            <span className="text-text-secondary">Protocol:</span>
            <span className="text-text-primary">v{serverHello.protocol_version}</span>
          </div>
        </div>
      )}

      <div className="text-xs text-text-tertiary">
        Homie Web &mdash; Gateway Console
      </div>
    </div>
  );
}
