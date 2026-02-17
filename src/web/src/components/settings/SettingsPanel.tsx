import { useCallback, useEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import type { ConnectionStatus } from "@/hooks/use-gateway";
import type { Target } from "@/hooks/use-targets";
import type { ServerHello } from "@homie/shared";
import type { PreviewRefresh } from "@/lib/session-utils";
import { SettingsNav, type SettingsSection } from "./SettingsNav";
import { ConnectionSection } from "./ConnectionSection";
import { ProviderAccountsSection } from "./ProviderAccountsSection";
import { PreferencesSection } from "./PreferencesSection";
import { AboutSection } from "./AboutSection";

interface SettingsPanelProps {
  isOpen: boolean;
  onClose: () => void;
  status: ConnectionStatus;
  activeTarget: Target | null;
  serverHello: ServerHello | null;
  previewRefresh: PreviewRefresh;
  onPreviewRefresh: (value: PreviewRefresh) => void;
  call: (method: string, params?: unknown) => Promise<unknown>;
  /** Optional initial section to display when the panel opens */
  initialSection?: SettingsSection;
}

export function SettingsPanel({
  isOpen,
  onClose,
  status,
  activeTarget,
  serverHello,
  previewRefresh,
  onPreviewRefresh,
  call,
  initialSection,
}: SettingsPanelProps) {
  const [activeSection, setActiveSection] = useState<SettingsSection>(initialSection ?? "connection");

  // Sync activeSection when initialSection changes while opening
  useEffect(() => {
    if (isOpen && initialSection) {
      setActiveSection(initialSection);
    }
  }, [isOpen, initialSection]);
  const panelRef = useRef<HTMLDivElement>(null);
  const closeButtonRef = useRef<HTMLButtonElement>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);

  // Capture focus origin on open, restore on close
  useEffect(() => {
    if (isOpen) {
      previousFocusRef.current = document.activeElement as HTMLElement | null;
      // Focus the close button on next frame so the panel is rendered
      requestAnimationFrame(() => {
        closeButtonRef.current?.focus();
      });
    }
  }, [isOpen]);

  // Restore focus when closing
  const handleClose = useCallback(() => {
    onClose();
    requestAnimationFrame(() => {
      previousFocusRef.current?.focus();
    });
  }, [onClose]);

  // Escape to close
  useEffect(() => {
    if (!isOpen) return;

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        handleClose();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen, handleClose]);

  // Focus trap
  useEffect(() => {
    if (!isOpen) return;

    const panel = panelRef.current;
    if (!panel) return;

    const onKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;

      const focusable = panel.querySelectorAll<HTMLElement>(
        'button, [href], input, select, textarea, [tabindex]:not([tabindex="-1"])',
      );
      if (focusable.length === 0) return;

      const first = focusable[0];
      const last = focusable[focusable.length - 1];

      if (e.shiftKey && document.activeElement === first) {
        e.preventDefault();
        last.focus();
      } else if (!e.shiftKey && document.activeElement === last) {
        e.preventDefault();
        first.focus();
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [isOpen]);

  if (!isOpen) return null;

  const sectionContent = (() => {
    switch (activeSection) {
      case "connection":
        return <ConnectionSection status={status} activeTarget={activeTarget} serverHello={serverHello} />;
      case "providers":
        return <ProviderAccountsSection status={status} call={call} />;
      case "preferences":
        return <PreferencesSection previewRefresh={previewRefresh} onPreviewRefresh={onPreviewRefresh} />;
      case "about":
        return <AboutSection serverHello={serverHello} />;
    }
  })();

  return (
    <>
      {/* Overlay */}
      <div
        className="fixed inset-0 z-40 bg-overlay"
        onClick={handleClose}
        aria-hidden="true"
      />

      {/* Panel */}
      <div
        ref={panelRef}
        role="dialog"
        aria-modal="true"
        aria-label="Settings"
        className="
          fixed inset-y-0 right-0 z-50
          w-full max-w-[480px]
          bg-background border-l border-border
          flex flex-col
          homie-settings-enter
        "
      >
        {/* Sticky header */}
        <div className="flex items-center justify-between gap-3 px-4 py-3 border-b border-border bg-surface-0 shrink-0">
          <h1 className="text-base font-semibold text-text-primary">Settings</h1>
          <button
            ref={closeButtonRef}
            type="button"
            onClick={handleClose}
            className="p-2 min-h-[44px] min-w-[44px] rounded-md text-text-secondary hover:text-text-primary hover:bg-surface-1 transition-colors"
            aria-label="Close settings"
          >
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body: nav + content */}
        <div className="flex-1 min-h-0 flex flex-col sm:flex-row overflow-hidden">
          <div className="shrink-0 px-4 pt-3 sm:py-3 sm:px-0 sm:pl-3">
            <SettingsNav activeSection={activeSection} onSectionChange={setActiveSection} />
          </div>
          <div className="flex-1 min-h-0 overflow-y-auto px-4 py-4">
            {sectionContent}
          </div>
        </div>
      </div>
    </>
  );
}
